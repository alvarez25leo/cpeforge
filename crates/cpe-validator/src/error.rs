//! Errores de infraestructura. Los errores de **reglas** son [`CpeError`] y
//! viajan en el `Ok` de la validación; este tipo indica que no se pudo
//! validar (nunca se convierte en «CPE válido»).

use std::fmt;
use std::path::PathBuf;

use crate::entity::CpeError;

/// Origen de una entrada rechazada.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputSource {
    /// XML a validar.
    Xml,
    /// Hoja principal.
    Stylesheet(PathBuf),
    /// Documento cargado por `xsl:include`, `xsl:import` o `document()`.
    Included(String),
}

/// Límite de recursos alcanzado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LimitKind {
    /// `pcre.backtrack_limit` o `pcre.recursion_limit` en `matches`.
    Regex,
    /// [`crate::Limits::xpath_operations`].
    XPathOperations,
    /// Profundidad de plantillas o variables de libxslt (`xsltMaxDepth`).
    TemplateDepth,
}

/// Operación de E/S rechazada por la política de hojas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccessKind {
    /// Lectura de archivo (`xsl:include`, `xsl:import`, `document()`).
    ReadFile,
    /// Escritura de archivo (`exsl:document`).
    WriteFile,
    /// Creación de directorio.
    CreateDirectory,
    /// Lectura por red.
    ReadNetwork,
    /// Escritura por red.
    WriteNetwork,
}

/// Error de infraestructura de la validación.
#[derive(Debug)]
#[non_exhaustive]
pub enum CpeValidatorError {
    /// XML vacío (la referencia: `ValueError` de `DOMDocument::loadXML()`).
    EmptyXml,
    /// XML mal formado. La referencia continúa con un documento sin raíz (tipo `null`,
    /// error `0305`); aquí se rechaza.
    MalformedXml {
        /// Diagnósticos de libxml2.
        diagnostics: Vec<String>,
    },
    /// El XML, la hoja o un documento incluido declara `DOCTYPE`.
    DoctypeForbidden {
        /// Dónde.
        source: InputSource,
    },
    /// Entrada mayor que el límite.
    TooLarge {
        /// Dónde.
        source: InputSource,
        /// Límite (bytes).
        limit: u64,
        /// Tamaño real (bytes).
        actual: u64,
    },
    /// `VoidedDocuments` sin `cbc:ID` (la referencia: `TypeError` de `strpos()`).
    VoidedWithoutId,
    /// No se pudo leer la hoja o el directorio de reglas.
    RuleIo {
        /// Ruta solicitada.
        path: PathBuf,
        /// Causa.
        source: std::io::Error,
    },
    /// La hoja está fuera del directorio de reglas autorizado.
    RuleOutsideRoot {
        /// Ruta canónica de la hoja.
        path: PathBuf,
        /// Directorio autorizado (canónico).
        root: PathBuf,
    },
    /// Hoja mal formada o con errores de compilación de libxslt.
    StylesheetInvalid {
        /// Hoja, si se cargó de un archivo.
        path: Option<PathBuf>,
        /// Diagnósticos de libxml2/libxslt.
        diagnostics: Vec<String>,
    },
    /// `XslValidator` sin hoja (la referencia: `InvalidArgumentException`).
    XslNotSet,
    /// Nombre de archivo con NUL (la referencia: `ValueError` de `setParameter()`).
    InvalidFilename,
    /// `php:function` falló como en la referencia (`Error`, `TypeError`,
    /// `ArgumentCountError`); el mensaje es el de la referencia.
    Extension {
        /// Clase de la excepción equivalente.
        class: &'static str,
        /// Mensaje de la referencia.
        message: String,
    },
    /// Patrón PCRE inválido en `matches` (la referencia: aviso y `false`).
    Regex {
        /// Patrón recibido por `matches`.
        pattern: String,
        /// Aviso que emitiría `preg_match`.
        message: String,
    },
    /// Límite de recursos alcanzado.
    ResourceLimit {
        /// Qué límite.
        kind: LimitKind,
    },
    /// La política de hojas rechazó una E/S (la referencia la permite o la ignora).
    AccessDenied {
        /// Operación.
        kind: AccessKind,
        /// Ruta o URL.
        resource: String,
        /// Motivo.
        reason: &'static str,
    },
    /// libxslt terminó con un error de ejecución (variable o función
    /// inexistente, nombre inválido…). La referencia devuelve los errores emitidos
    /// hasta ese punto como si la validación hubiera terminado.
    Transform {
        /// Errores de regla emitidos antes del fallo.
        partial: Vec<CpeError>,
        /// Mensajes de libxslt en orden.
        diagnostics: Vec<String>,
    },
    /// Falla interna del motor (memoria, inicialización).
    Engine(&'static str),
}

impl CpeValidatorError {
    /// Clase de la excepción que lanza la referencia en el mismo caso, si la lanza.
    /// `None` cuando la referencia devuelve un resultado (aunque sea vacío o parcial).
    #[must_use]
    pub fn error_class(&self) -> Option<&'static str> {
        Some(match self {
            Self::EmptyXml | Self::InvalidFilename => "ValueError",
            Self::VoidedWithoutId => "TypeError",
            Self::XslNotSet => "InvalidArgumentException",
            Self::Extension { class, .. } => class,
            Self::RuleIo { .. } | Self::StylesheetInvalid { .. } => "Error",
            _ => return None,
        })
    }
}

impl fmt::Display for CpeValidatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyXml => f.write_str("XML vacío"),
            Self::MalformedXml { diagnostics } => {
                write!(f, "XML mal formado: {}", diagnostics.join(" ").trim())
            }
            Self::DoctypeForbidden { source } => write!(f, "DOCTYPE no permitido en {source:?}"),
            Self::TooLarge {
                source,
                limit,
                actual,
            } => write!(f, "{source:?} supera el límite: {actual} > {limit} bytes"),
            Self::VoidedWithoutId => f.write_str("VoidedDocuments sin cbc:ID"),
            Self::RuleIo { path, source } => {
                write!(f, "no se pudo leer la regla {}: {source}", path.display())
            }
            Self::RuleOutsideRoot { path, root } => {
                write!(
                    f,
                    "la regla {} está fuera del directorio {}",
                    path.display(),
                    root.display()
                )
            }
            Self::StylesheetInvalid { path, diagnostics } => write!(
                f,
                "hoja XSL inválida{}: {}",
                path.as_ref()
                    .map(|p| format!(" {}", p.display()))
                    .unwrap_or_default(),
                diagnostics.join(" ").trim()
            ),
            // Mensaje idéntico al de la referencia.
            Self::XslNotSet => f.write_str("XslDocument no set"),
            Self::InvalidFilename => f.write_str("el nombre de archivo contiene NUL"),
            Self::Extension { message, .. } => f.write_str(message),
            Self::Regex { pattern, message } => {
                write!(f, "patrón inválido en matches ({pattern:?}): {message}")
            }
            Self::ResourceLimit { kind } => write!(f, "límite de recursos alcanzado: {kind:?}"),
            Self::AccessDenied {
                kind,
                resource,
                reason,
            } => write!(f, "acceso {kind:?} a {resource} rechazado: {reason}"),
            Self::Transform { diagnostics, .. } => {
                write!(f, "error de ejecución XSLT: {}", diagnostics.join(" | "))
            }
            Self::Engine(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CpeValidatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuleIo { source, .. } => Some(source),
            _ => None,
        }
    }
}
