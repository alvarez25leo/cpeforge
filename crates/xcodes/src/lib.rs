//! Códigos y mensajes del catálogo de errores de SUNAT.
//!
//! El catálogo es el de la hoja «CódigosRetorno» de las reglas de validación
//! de SUNAT en la versión [`CATALOG_VERSION`] (se regenera con
//! `scripts/update_xcodes.py`). Los códigos de la referencia conservan su
//! texto salvo los que SUNAT cambió.
//!
//! El catálogo se incrusta desde `CodeErrors.xml`, se comprueba antes del
//! primer uso y se parsea una sola vez. Los códigos son cadenas: `0100` no
//! equivale a `100`. Las consultas usan una clave literal, nunca XPath.
//!
//! ```
//! use cpeforge_xcodes::XmlErrorCodeProvider;
//! let codes = XmlErrorCodeProvider::new()?;
//! assert!(codes.get("0100").starts_with("El sistema"));
//! assert_eq!(codes.get("desconocido"), "");
//! # Ok::<(), cpeforge_xcodes::CatalogError>(())
//! ```

use cpeforge_core::ports::ErrorCodeProvider;
use roxmltree::Document;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::str;
use std::sync::OnceLock;

/// Versión de las reglas de validación de SUNAT de la que sale el catálogo
/// («Reglas de validación - actualizado al 26.08.2026»).
pub const CATALOG_VERSION: &str = "2026-08-26";
/// SHA-256 del `CodeErrors.xml` incrustado.
pub const XML_SHA256: &str = "108d64164c0a3c0eb70de700c3e1f14e9a8aa724ce06aac52020170114f43216";
/// Cantidad de códigos únicos del catálogo.
pub const ENTRY_COUNT: usize = 2_077;
const XML_BYTES: &[u8] = include_bytes!("data/CodeErrors.xml");
const XML_LENGTH: usize = 222_841;
static CATALOG: OnceLock<Result<Catalog, CatalogError>> = OnceLock::new();

/// Recurso incrustado roto o distinto del corte de la referencia fijado.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CatalogError {
    /// No hay BOM UTF-8 inicial.
    MissingBom,
    /// La longitud del recurso cambió.
    Length {
        /// Longitud encontrada en bytes.
        actual: usize,
    },
    /// El resumen del recurso cambió.
    Hash {
        /// Resumen SHA-256 encontrado en hexadecimal.
        actual: String,
    },
    /// Los bytes posteriores al BOM no son UTF-8.
    Utf8,
    /// El XML no es válido o declara un DTD.
    Xml(String),
    /// Cambió la estructura esperada del catálogo.
    Shape(&'static str),
    /// El mismo código aparece más de una vez.
    Duplicate(String),
    /// La cantidad de códigos cambió.
    Count {
        /// Cantidad de registros encontrada.
        actual: usize,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBom => write!(f, "CodeErrors.xml: falta el BOM UTF-8"),
            Self::Length { actual } => write!(f, "CodeErrors.xml: longitud inesperada {actual}"),
            Self::Hash { actual } => write!(f, "CodeErrors.xml: SHA-256 inesperado {actual}"),
            Self::Utf8 => write!(f, "CodeErrors.xml: UTF-8 inválido"),
            Self::Xml(reason) => write!(f, "CodeErrors.xml: XML inválido: {reason}"),
            Self::Shape(reason) => write!(f, "CodeErrors.xml: estructura inválida: {reason}"),
            Self::Duplicate(code) => write!(f, "CodeErrors.xml: código duplicado {code}"),
            Self::Count { actual } => write!(
                f,
                "CodeErrors.xml: {actual} códigos, se esperaban {ENTRY_COUNT}"
            ),
        }
    }
}

impl std::error::Error for CatalogError {}

struct Catalog {
    ordered: Vec<(String, String)>,
    positions: HashMap<String, usize>,
}

/// Proveedor inmutable equivalente a `XmlErrorCodeProvider` de la referencia.
///
/// `new` devuelve un error si los bytes incrustados no coinciden con el XML
/// fijado. La primera instancia parsea el recurso; las siguientes reutilizan
/// la tabla comprobada.
#[derive(Clone, Copy)]
pub struct XmlErrorCodeProvider {
    catalog: &'static Catalog,
}

impl fmt::Debug for XmlErrorCodeProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XmlErrorCodeProvider")
            .finish_non_exhaustive()
    }
}

impl XmlErrorCodeProvider {
    /// Comprueba y carga el catálogo una sola vez.
    ///
    /// # Errors
    ///
    /// El recurso incluido cambió, está mal formado o viola sus invariantes.
    pub fn new() -> Result<Self, CatalogError> {
        let catalog = CATALOG
            .get_or_init(load_catalog)
            .as_ref()
            .map_err(Clone::clone)?;
        Ok(Self { catalog })
    }

    /// Consulta literal por código. Un código desconocido produce `""`, igual
    /// que `getValue()` para una búsqueda sin resultados en la referencia fijado.
    pub fn get(&self, code: &str) -> &str {
        self.catalog
            .positions
            .get(code)
            .map(|position| self.catalog.ordered[*position].1.as_str())
            .unwrap_or("")
    }

    /// Recorre código y mensaje en el orden original del XML, sin convertir
    /// claves numéricas ni perder ceros iniciales.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &str)> + '_ {
        self.catalog
            .ordered
            .iter()
            .map(|(code, message)| (code.as_str(), message.as_str()))
    }
}

impl ErrorCodeProvider for XmlErrorCodeProvider {
    type Error = CatalogError;

    fn all(&self) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        Ok(Some(self.catalog.ordered.clone()))
    }

    fn value(&self, code: Option<&str>) -> Result<Option<String>, Self::Error> {
        Ok(Some(self.get(code.unwrap_or("")).to_owned()))
    }
}

fn load_catalog() -> Result<Catalog, CatalogError> {
    checked_catalog(XML_BYTES)
}

fn checked_catalog(bytes: &[u8]) -> Result<Catalog, CatalogError> {
    if !bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(CatalogError::MissingBom);
    }
    if bytes.len() != XML_LENGTH {
        return Err(CatalogError::Length {
            actual: bytes.len(),
        });
    }
    let mut actual = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut actual, "{byte:02x}")
            .map_err(|_| CatalogError::Shape("no se pudo codificar SHA-256"))?;
    }
    if actual != XML_SHA256 {
        return Err(CatalogError::Hash { actual });
    }
    parse_catalog(bytes)
}

fn parse_catalog(bytes: &[u8]) -> Result<Catalog, CatalogError> {
    let text = str::from_utf8(
        bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .ok_or(CatalogError::MissingBom)?,
    )
    .map_err(|_| CatalogError::Utf8)?;
    let xml = Document::parse(text).map_err(|error| CatalogError::Xml(error.to_string()))?;
    let root = xml.root_element();
    if root.tag_name().name() != "errors" || root.attributes().len() != 0 {
        return Err(CatalogError::Shape("raíz distinta de <errors>"));
    }
    let mut ordered = Vec::with_capacity(ENTRY_COUNT);
    let mut positions = HashMap::with_capacity(ENTRY_COUNT);
    for node in root.children() {
        if !node.is_element() {
            if node.is_text() && node.text().is_some_and(|text| text.trim().is_empty()) {
                continue;
            }
            return Err(CatalogError::Shape("contenido fuera de <error>"));
        }
        if node.tag_name().name() != "error" || node.attributes().len() != 1 {
            return Err(CatalogError::Shape("elemento distinto de <error code=...>"));
        }
        let code = node
            .attribute("code")
            .ok_or(CatalogError::Shape("falta code"))?;
        if code.is_empty() || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(CatalogError::Shape("código no numérico"));
        }
        if node.children().any(|child| !child.is_text()) {
            return Err(CatalogError::Shape("contenido anidado en <error>"));
        }
        let message: String = node.children().filter_map(|child| child.text()).collect();
        if message.is_empty() {
            return Err(CatalogError::Shape("mensaje vacío"));
        }
        if positions.insert(code.to_owned(), ordered.len()).is_some() {
            return Err(CatalogError::Duplicate(code.to_owned()));
        }
        ordered.push((code.to_owned(), message));
    }
    if ordered.len() != ENTRY_COUNT {
        return Err(CatalogError::Count {
            actual: ordered.len(),
        });
    }
    Ok(Catalog { ordered, positions })
}

#[cfg(test)]
mod tests {
    use super::{CatalogError, XML_BYTES, XmlErrorCodeProvider, checked_catalog, parse_catalog};

    #[test]
    fn rejects_modified_resources() {
        let mut modified = XML_BYTES.to_vec();
        modified[100] ^= 1;
        assert!(matches!(
            checked_catalog(&modified),
            Err(CatalogError::Hash { .. })
        ));
        assert!(matches!(
            checked_catalog(&modified[..100]),
            Err(CatalogError::Length { .. })
        ));
        assert_eq!(
            parse_catalog(b"<errors/>").err(),
            Some(CatalogError::MissingBom)
        );
        let dtd =
            b"\xef\xbb\xbf<!DOCTYPE errors [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><errors/>";
        assert!(matches!(parse_catalog(dtd), Err(CatalogError::Xml(_))));
        let duplicate =
            b"\xef\xbb\xbf<errors><error code='1'>a</error><error code='1'>b</error></errors>";
        assert_eq!(
            parse_catalog(duplicate).err(),
            Some(CatalogError::Duplicate("1".into()))
        );
        let invalid = b"\xef\xbb\xbf<errors><error code='1'>\xff</error></errors>";
        assert_eq!(parse_catalog(invalid).err(), Some(CatalogError::Utf8));
    }

    #[test]
    fn literal_lookup() -> Result<(), CatalogError> {
        let provider = XmlErrorCodeProvider::new()?;
        assert_eq!(provider.iter().len(), super::ENTRY_COUNT);
        assert!(!provider.get("0100").is_empty());
        assert_eq!(provider.get("' or @code='0100"), "");
        assert_eq!(provider.get("000100"), "");
        Ok(())
    }
}
