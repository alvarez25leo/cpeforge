//! Valida documentos UBL 2.0/2.1 con libxml2 XSD 1.0 y un conjunto de
//! esquemas local verificado contra la referencia.
//!
//! Los XSD se proporcionan como directorio externo hasta cerrar su auditoría
//! de redistribución. Cada validación comprueba las huellas de todos los
//! archivos que alcanza su esquema antes de que libxml2 abra el esquema.
//! No se aceptan DTD, red, rutas fuera de ese directorio ni XML excesivo.

mod ffi;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

use cpeforge_xml_limits::{DEFAULT_MAX_ATTRIBUTES, MarkupRejection, check_markup};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const MANIFEST: &str = include_str!("../schema-manifest.json");
const MAX_XML_BYTES: usize = 10 * 1024 * 1024;
const MAX_NODES: usize = 100_000;
const MAX_DEPTH: usize = 128;

#[derive(Debug, Deserialize)]
struct Manifest {
    xsd: Vec<SchemaEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct SchemaEntry {
    path: String,
    sha256: String,
    references: Vec<SchemaReference>,
}

#[derive(Clone, Debug, Deserialize)]
struct SchemaReference {
    location: Option<String>,
}

/// Error libxml observable de `SchemaValidator`; `file` es siempre `None`
/// porque el mapper no asigna ese campo.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct XmlError {
    /// Severidad libxml: warning 1, error 2, fatal 3.
    pub level: i32,
    /// Código numérico libxml.
    pub code: i32,
    /// Columna informada por libxml.
    pub column: i32,
    /// Texto de libxml, incluido el salto final si existe.
    pub message: String,
    /// La referencia deja este campo sin asignar.
    pub file: Option<String>,
    /// Línea informada por libxml.
    pub line: i32,
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Code: {}, Line: {}, Column: {}, Message: {}",
            self.code, self.line, self.column, self.message
        )
    }
}

/// Fallos de infraestructura o de la política de seguridad; separados de los
/// errores de validación XSD, que producen [`Validation::valid`] `false`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InfrastructureError {
    /// El directorio de XSD no existe o no es legible.
    SchemaDirectory,
    /// Ruta derivada del XML fuera de los 16 esquemas incluidos.
    UnsafePath,
    /// Falta un import/include requerido para compilar el esquema.
    MissingImport(String),
    /// Archivo XSD modificado respecto al commit fijado.
    SchemaHash(String),
    /// El esquema tiene un enlace simbólico o sale del directorio autorizado.
    SchemaEscape(String),
    /// Tamaño, profundidad o número de nodos fuera del límite.
    XmlLimit,
    /// DTD o entidad declarada; la referencia original la acepta según libxml.
    DtdForbidden,
    /// libxml no pudo analizar el XML precomprobado.
    XmlParse,
    /// La referencia lanza `ValueError` al pasar una cadena vacía a `loadXML`.
    EmptyInput,
    /// libxml no pudo compilar el esquema.
    SchemaCompile(Vec<XmlError>),
    /// Error interno de validación de libxml.
    SchemaValidate(Vec<XmlError>),
    /// Error del manifiesto incluido en el crate.
    Manifest,
    /// Fallo al crear una copia privada de los esquemas verificados.
    ResourceIo,
}

impl fmt::Display for InfrastructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaDirectory => write!(f, "directorio de esquemas no disponible"),
            Self::UnsafePath => write!(f, "ruta de esquema no autorizada"),
            Self::MissingImport(p) => write!(f, "import XSD ausente: {p}"),
            Self::SchemaHash(p) => write!(f, "hash XSD distinto: {p}"),
            Self::SchemaEscape(p) => write!(f, "XSD fuera del directorio autorizado: {p}"),
            Self::XmlLimit => write!(f, "límite XML excedido"),
            Self::DtdForbidden => write!(f, "DTD prohibido"),
            Self::XmlParse => write!(f, "libxml no pudo analizar XML"),
            Self::EmptyInput => write!(
                f,
                "DOMDocument::loadXML(): Argument #1 ($source) must not be empty"
            ),
            Self::SchemaCompile(_) => write!(f, "libxml no pudo compilar XSD"),
            Self::SchemaValidate(_) => write!(f, "error interno de validación XSD"),
            Self::Manifest => write!(f, "manifiesto XSD inválido"),
            Self::ResourceIo => write!(f, "no se pudo preparar una copia privada de los XSD"),
        }
    }
}

impl std::error::Error for InfrastructureError {}

/// Resultado tipado de una llamada a `isValid`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Validation {
    /// Equivalente al booleano.
    pub valid: bool,
    /// Versión resuelta del XML o fijada por la configuración.
    pub version: String,
    /// Ruta relativa elegida; `None` para XML sin raíz.
    pub schema: Option<String>,
    /// Mensaje equivalente a `UblValidator::getError()`.
    pub error: String,
    /// Errores ordenados de `SchemaValidator::getErrors()`.
    pub errors: Vec<XmlError>,
}

/// Resolutor de versión y validador con estado de último resultado.
pub struct UblValidator {
    base: PathBuf,
    display_base: PathBuf,
    manifest: HashMap<String, SchemaEntry>,
    version: Option<String>,
    last: Option<Validation>,
}

impl fmt::Debug for UblValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UblValidator")
            .field("schemas", &self.manifest.len())
            .finish_non_exhaustive()
    }
}

/// Acceso directo al motor XSD para uno de los 45 recursos aprobados.
/// A diferencia de la referencia, un import ausente o alterado es un error de
/// infraestructura antes de ejecutar libxml.
pub struct SchemaValidator {
    owner: UblValidator,
    errors: Vec<XmlError>,
}

impl fmt::Debug for SchemaValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SchemaValidator")
            .field("errors", &self.errors.len())
            .finish_non_exhaustive()
    }
}

impl SchemaValidator {
    /// Crea un validador directo sobre el directorio XSD fijado.
    pub fn new(base: impl AsRef<Path>) -> Result<Self, InfrastructureError> {
        Ok(Self {
            owner: UblValidator::new(base)?,
            errors: Vec::new(),
        })
    }

    /// Valida XML frente a una ruta relativa del manifiesto, por ejemplo
    /// `2.1/maindoc/UBL-Invoice-2.1.xsd`.
    pub fn validate(&mut self, xml: &[u8], relative: &str) -> Result<bool, InfrastructureError> {
        self.errors.clear();
        preflight_document(xml)?;
        let snapshot = self.owner.snapshot(relative)?;
        let path = snapshot.path().join(relative);
        let path = path.to_str().ok_or(InfrastructureError::UnsafePath)?;
        let (valid, errors) = ffi::validate(xml, path)?;
        self.errors = errors;
        Ok(valid)
    }

    /// Errores ordenados de la última validación.
    pub fn get_errors(&self) -> &[XmlError] {
        &self.errors
    }
}

impl UblValidator {
    /// Crea una instancia sobre el directorio `src/xsd` del paquete de referencia fijado.
    /// Los XSD permanecen externos mientras se resuelve su atribución.
    pub fn new(base: impl AsRef<Path>) -> Result<Self, InfrastructureError> {
        let display_base = base.as_ref().to_path_buf();
        let base = fs::canonicalize(base).map_err(|_| InfrastructureError::SchemaDirectory)?;
        if !base.is_dir() {
            return Err(InfrastructureError::SchemaDirectory);
        }
        let manifest: Manifest =
            serde_json::from_str(MANIFEST).map_err(|_| InfrastructureError::Manifest)?;
        if manifest.xsd.len() != 45 {
            return Err(InfrastructureError::Manifest);
        }
        let manifest = manifest
            .xsd
            .into_iter()
            .map(|x| (x.path.clone(), x))
            .collect();
        Ok(Self {
            base,
            display_base,
            manifest,
            version: None,
            last: None,
        })
    }

    /// Fija la versión de esquema; `None` limpia la versión guardada. Como el
    /// resolutor, una versión detectada no vacía se conserva entre llamadas.
    pub fn set_version(&mut self, version: Option<&str>) -> Result<(), InfrastructureError> {
        if version.is_some_and(|v| v != "2.0" && v != "2.1") {
            return Err(InfrastructureError::UnsafePath);
        }
        self.version = version.map(str::to_owned);
        Ok(())
    }

    /// Devuelve el último mensaje, o `None` antes de validar.
    pub fn get_error(&self) -> Option<&str> {
        self.last.as_ref().map(|v| v.error.as_str())
    }

    /// Devuelve el último resultado íntegro.
    pub fn last_result(&self) -> Option<&Validation> {
        self.last.as_ref()
    }

    /// Valida XML contra su esquema UBL 2.0/2.1. libxml2 interpreta la
    /// codificación declarada, igual que en la referencia.
    pub fn validate(&mut self, xml: &[u8]) -> Result<Validation, InfrastructureError> {
        match preflight_document(xml) {
            Ok(()) => {}
            Err(InfrastructureError::XmlParse) => {
                let result = Validation {
                    valid: false,
                    version: String::new(),
                    schema: None,
                    error: "Invalid XML Document".into(),
                    errors: Vec::new(),
                };
                self.last = Some(result.clone());
                return Ok(result);
            }
            Err(error) => return Err(error),
        }
        let Some((name, detected_version)) = ffi::probe(xml)? else {
            let result = Validation {
                valid: false,
                version: String::new(),
                schema: None,
                error: "Invalid XML Document".into(),
                errors: Vec::new(),
            };
            self.last = Some(result.clone());
            return Ok(result);
        };
        let version = self.version.clone().unwrap_or(detected_version);
        let relative = resolve_schema_path(&name, &version)?;
        if !version.is_empty() {
            self.version = Some(version.clone());
        }
        if version.is_empty() {
            let result = Validation {
                valid: false,
                version,
                schema: Some(relative),
                error: format!(
                    "XSD Path: :'{}//maindoc/UBL-{name}-.xsd' not found",
                    self.display_base.display()
                ),
                errors: Vec::new(),
            };
            self.last = Some(result.clone());
            return Ok(result);
        }
        let path = self.base.join(&relative);
        if !path.is_file() {
            let display_path = self.display_base.join(&relative);
            let result = Validation {
                valid: false,
                version,
                schema: Some(relative),
                error: format!("XSD Path: :'{}' not found", display_path.display()),
                errors: Vec::new(),
            };
            self.last = Some(result.clone());
            return Ok(result);
        }
        let snapshot = self.snapshot(&relative)?;
        let verified_path = snapshot.path().join(&relative);
        let path_text = verified_path
            .to_str()
            .ok_or(InfrastructureError::UnsafePath)?;
        let (valid, errors) = ffi::validate(xml, path_text)?;
        let error = if valid {
            String::new()
        } else {
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        };
        let result = Validation {
            valid,
            version,
            schema: Some(relative),
            error,
            errors,
        };
        self.last = Some(result.clone());
        Ok(result)
    }

    fn snapshot(&self, root: &str) -> Result<tempfile::TempDir, InfrastructureError> {
        let snapshot = tempfile::Builder::new()
            .prefix("cpeforge-ubl-")
            .tempdir()
            .map_err(|_| InfrastructureError::ResourceIo)?;
        let mut seen = HashSet::new();
        let mut pending = vec![root.to_owned()];
        while let Some(relative) = pending.pop() {
            if !seen.insert(relative.clone()) {
                continue;
            }
            let entry = self
                .manifest
                .get(&relative)
                .ok_or(InfrastructureError::UnsafePath)?;
            let path = self.base.join(&relative);
            let canonical = fs::canonicalize(&path)
                .map_err(|_| InfrastructureError::MissingImport(relative.clone()))?;
            if !canonical.starts_with(&self.base)
                || fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err(InfrastructureError::SchemaEscape(relative));
            }
            let bytes = fs::read(&canonical)
                .map_err(|_| InfrastructureError::MissingImport(relative.clone()))?;
            let hash =
                Sha256::digest(&bytes)
                    .iter()
                    .fold(String::with_capacity(64), |mut hex, b| {
                        let _ = write!(hex, "{b:02x}");
                        hex
                    });
            if hash != entry.sha256 {
                return Err(InfrastructureError::SchemaHash(relative));
            }
            let destination = snapshot.path().join(&relative);
            let parent = destination
                .parent()
                .ok_or(InfrastructureError::ResourceIo)?;
            fs::create_dir_all(parent).map_err(|_| InfrastructureError::ResourceIo)?;
            fs::write(&destination, &bytes).map_err(|_| InfrastructureError::ResourceIo)?;
            for reference in &entry.references {
                let Some(location) = &reference.location else {
                    continue;
                };
                let child = normalize_reference(&relative, location)
                    .ok_or(InfrastructureError::UnsafePath)?;
                if !self.manifest.contains_key(&child) {
                    return Err(InfrastructureError::MissingImport(child));
                }
                pending.push(child);
            }
        }
        Ok(snapshot)
    }
}

fn preflight_document(xml: &[u8]) -> Result<(), InfrastructureError> {
    if xml.is_empty() {
        return Err(InfrastructureError::EmptyInput);
    }
    if xml.len() > MAX_XML_BYTES {
        return Err(InfrastructureError::XmlLimit);
    }
    match check_markup(xml, DEFAULT_MAX_ATTRIBUTES) {
        Ok(()) => {}
        Err(MarkupRejection::Declaration) => return Err(InfrastructureError::DtdForbidden),
        Err(MarkupRejection::TooManyAttributes) => return Err(InfrastructureError::XmlLimit),
        Err(_) => return Err(InfrastructureError::XmlParse),
    }
    ffi::preflight(xml, MAX_NODES, MAX_DEPTH)
}

/// Compone la ruta relativa de un esquema como `UblPathResolver::getPath`,
/// sin verificar que exista. Rechaza segmentos que podrían salir de la raíz.
pub fn resolve_schema_path(name: &str, version: &str) -> Result<String, InfrastructureError> {
    if !safe_segment(name) || (!version.is_empty() && !safe_segment(version)) {
        return Err(InfrastructureError::UnsafePath);
    }
    Ok(format!("{version}/maindoc/UBL-{name}-{version}.xsd"))
}

fn safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

fn normalize_reference(parent: &str, location: &str) -> Option<String> {
    if location.contains("://") || location.contains('\\') {
        return None;
    }
    let mut parts: Vec<&str> = parent.split('/').collect();
    parts.pop()?;
    for component in Path::new(location).components() {
        match component {
            Component::Normal(segment) => parts.push(segment.to_str()?),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    let result = parts.join("/");
    (result.starts_with("2.0/") || result.starts_with("2.1/")).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_do_not_escape() {
        assert_eq!(
            normalize_reference(
                "2.0/maindoc/UBL-Invoice-2.0.xsd",
                "../common/UBL-CommonBasicComponents-2.0.xsd"
            ),
            Some("2.0/common/UBL-CommonBasicComponents-2.0.xsd".into())
        );
        assert_eq!(
            normalize_reference("2.0/common/a.xsd", "../../../escape.xsd"),
            None
        );
        assert_eq!(
            normalize_reference("2.0/common/a.xsd", "https://example.org/x.xsd"),
            None
        );
    }
}
