//! Hojas XSLT 1.0 compiladas con libxslt.

use std::path::{Path, PathBuf};

use crate::error::{AccessKind, CpeValidatorError, InputSource};
use crate::ffi::{self, Compiled, Denial, IoOperation, ReadPolicy, Session};
use crate::options::Limits;

/// Hoja compilada y su política de lectura.
///
/// Las lecturas de `xsl:include`, `xsl:import` y `document()` solo se
/// permiten dentro del directorio autorizado (resolviendo enlaces
/// simbólicos) y hasta [`Limits::max_stylesheet_bytes`]. La red, la
/// escritura (`exsl:document`) y cualquier `DOCTYPE` se rechazan. Están
/// disponibles EXSLT (como en la referencia) y solo `php:function`/`php:functionString`
/// con `XsltFunctions::matches`.
pub struct Stylesheet {
    style: ffi::Style,
    path: Option<PathBuf>,
    root: Option<PathBuf>,
    max_file_bytes: u64,
}

impl std::fmt::Debug for Stylesheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stylesheet")
            .field("path", &self.path)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

fn canonical(path: &Path) -> Result<PathBuf, CpeValidatorError> {
    std::fs::canonicalize(path).map_err(|source| CpeValidatorError::RuleIo {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn access_kind(operation: IoOperation) -> AccessKind {
    match operation {
        IoOperation::ReadFile => AccessKind::ReadFile,
        IoOperation::WriteFile => AccessKind::WriteFile,
        IoOperation::CreateDirectory => AccessKind::CreateDirectory,
        IoOperation::ReadNetwork => AccessKind::ReadNetwork,
        IoOperation::WriteNetwork => AccessKind::WriteNetwork,
    }
}

pub(crate) fn denial_error(denial: &Denial) -> CpeValidatorError {
    CpeValidatorError::AccessDenied {
        kind: access_kind(denial.operation),
        resource: denial.resource.clone(),
        reason: denial.reason,
    }
}

impl Stylesheet {
    /// Carga y compila `path`. `rules_root` es el directorio autorizado; con
    /// `None` se autoriza el directorio de la hoja.
    ///
    /// # Errors
    ///
    /// [`CpeValidatorError::RuleIo`], [`CpeValidatorError::RuleOutsideRoot`],
    /// [`CpeValidatorError::TooLarge`], [`CpeValidatorError::DoctypeForbidden`],
    /// [`CpeValidatorError::AccessDenied`] o
    /// [`CpeValidatorError::StylesheetInvalid`].
    pub fn from_file(
        path: impl AsRef<Path>,
        rules_root: Option<&Path>,
        limits: &Limits,
    ) -> Result<Self, CpeValidatorError> {
        let requested = path.as_ref();
        let path = canonical(requested)?;
        let root = match rules_root {
            Some(root) => canonical(root)?,
            None => path
                .parent()
                .map_or_else(|| path.clone(), Path::to_path_buf),
        };
        if !path.starts_with(&root) {
            return Err(CpeValidatorError::RuleOutsideRoot { path, root });
        }
        let meta = std::fs::metadata(&path).map_err(|source| CpeValidatorError::RuleIo {
            path: path.clone(),
            source,
        })?;
        if meta.len() > limits.max_stylesheet_bytes {
            return Err(CpeValidatorError::TooLarge {
                source: InputSource::Stylesheet(path),
                limit: limits.max_stylesheet_bytes,
                actual: meta.len(),
            });
        }
        let bytes = std::fs::read(&path).map_err(|source| CpeValidatorError::RuleIo {
            path: path.clone(),
            source,
        })?;
        Self::compile(&bytes, path, Some(root), limits)
    }

    /// Compila una hoja en memoria. Sin `rules_root` no puede leer ningún
    /// archivo; con él, las rutas relativas se resuelven desde ese
    /// directorio.
    ///
    /// # Errors
    ///
    /// Igual que [`Stylesheet::from_file`].
    pub fn from_bytes(
        xsl: impl AsRef<[u8]>,
        rules_root: Option<&Path>,
        limits: &Limits,
    ) -> Result<Self, CpeValidatorError> {
        let bytes = xsl.as_ref();
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let (root, base) = match rules_root {
            Some(root) => {
                let root = canonical(root)?;
                let base = root.join("inline.xsl");
                (Some(root), base)
            }
            None => (None, PathBuf::from("/inline.xsl")),
        };
        if actual > limits.max_stylesheet_bytes {
            return Err(CpeValidatorError::TooLarge {
                source: InputSource::Stylesheet(base),
                limit: limits.max_stylesheet_bytes,
                actual,
            });
        }
        let mut sheet = Self::compile(bytes, base, root, limits)?;
        sheet.path = None;
        Ok(sheet)
    }

    fn compile(
        bytes: &[u8],
        path: PathBuf,
        root: Option<PathBuf>,
        limits: &Limits,
    ) -> Result<Self, CpeValidatorError> {
        let policy = ReadPolicy {
            root: root.clone(),
            max_file_bytes: limits.max_stylesheet_bytes,
        };
        match ffi::compile_stylesheet(bytes, &path, &policy).map_err(CpeValidatorError::Engine)? {
            Compiled::Style(style) => Ok(Self {
                style,
                path: Some(path),
                root,
                max_file_bytes: limits.max_stylesheet_bytes,
            }),
            Compiled::Dtd => Err(CpeValidatorError::DoctypeForbidden {
                source: InputSource::Stylesheet(path),
            }),
            Compiled::Malformed(session) | Compiled::Failed(session) => {
                Err(compile_error(session, path))
            }
        }
    }

    /// Ruta canónica de la hoja (si se cargó de un archivo).
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Directorio autorizado (canónico).
    #[must_use]
    pub fn rules_root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub(crate) fn style(&self) -> &ffi::Style {
        &self.style
    }

    pub(crate) fn read_policy(&self) -> ReadPolicy {
        ReadPolicy {
            root: self.root.clone(),
            max_file_bytes: self.max_file_bytes,
        }
    }
}

fn compile_error(session: Session, path: PathBuf) -> CpeValidatorError {
    if let Some(denial) = session.denials.first() {
        return denial_error(denial);
    }
    if let Some(uri) = session.rejected_dtd.first() {
        return CpeValidatorError::DoctypeForbidden {
            source: InputSource::Included(uri.clone()),
        };
    }
    CpeValidatorError::StylesheetInvalid {
        path: Some(path),
        diagnostics: session.messages,
    }
}
