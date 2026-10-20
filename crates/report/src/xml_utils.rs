//! Lectura del `DigestValue` de un XML firmado.
//!
//! Busca `ext:UBLExtensions/ext:UBLExtension` bajo el elemento raíz, recorre
//! las extensiones **desde la última** y devuelve el texto del primer
//! `ext:ExtensionContent/ds:Signature/ds:SignedInfo/ds:Reference/ds:DigestValue`.
//! Sin extensiones o sin `DigestValue` devuelve una cadena vacía.
//!
//! Leer el resumen **no** verifica la firma: para eso está
//! `cpeforge-xmldsig`.
//!
//! # Entradas rechazadas
//!
//! Un XML mal formado, una DTD, un archivo ilegible o un documento que supera
//! los límites producen [`XmlUtilsError`] en lugar de un resumen vacío. La
//! DTD se rechaza para no expandir entidades.

use std::borrow::Cow;
use std::fmt;
use std::fs;
use std::io::Read as _;
use std::path::Path;

/// Espacio de nombres `ext` de UBL.
pub const EXT_NAMESPACE: &str =
    "urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2";
/// Espacio de nombres `ds` de XML-DSig.
pub const DS_NAMESPACE: &str = "http://www.w3.org/2000/09/xmldsig#";

/// Fallo al leer el XML.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlUtilsError {
    /// Contenido o ruta vacíos.
    Empty,
    /// XML mal formado.
    Malformed(String),
    /// El documento declara una DTD (rechazada por seguridad).
    Dtd,
    /// Codificación distinta de UTF-8 o ISO-8859-1 con bytes no ASCII.
    UnsupportedEncoding(String),
    /// Supera el tamaño, la profundidad, los atributos por elemento o los
    /// espacios de nombres máximos.
    Limit(String),
    /// No se pudo leer el archivo.
    Io(String),
}

impl fmt::Display for XmlUtilsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("XML vacío"),
            Self::Malformed(m) => write!(f, "XML mal formado: {m}"),
            Self::Dtd => f.write_str("DTD no permitida"),
            Self::UnsupportedEncoding(e) => write!(f, "codificación no admitida: {e}"),
            Self::Limit(m) => write!(f, "límite superado: {m}"),
            Self::Io(m) => write!(f, "no se pudo leer el archivo: {m}"),
        }
    }
}

impl std::error::Error for XmlUtilsError {}

/// Extractor del resumen de la firma con límites configurables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XmlUtils {
    /// Tamaño máximo del XML en bytes.
    pub max_bytes: usize,
    /// Profundidad máxima de elementos (256 por defecto).
    pub max_depth: usize,
}

impl Default for XmlUtils {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_depth: 256,
        }
    }
}

/// Codificación declarada en `<?xml … encoding="…"?>`, en minúsculas.
fn declared_encoding(bytes: &[u8]) -> Option<String> {
    let head = bytes.get(..bytes.len().min(256))?;
    let head = String::from_utf8_lossy(head);
    let decl = head.strip_prefix("<?xml")?;
    let decl = &decl[..decl.find("?>")?];
    let pos = decl.find("encoding")?;
    let rest = decl[pos + "encoding".len()..]
        .trim_start()
        .strip_prefix('=')?
        .trim_start();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let value = &rest[1..];
    Some(value[..value.find(quote)?].to_ascii_lowercase())
}

/// Texto del documento: UTF-8 se usa sin copiar; ISO-8859-1 se convierte.
fn decode(bytes: &[u8]) -> Result<Cow<'_, str>, XmlUtilsError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(Cow::Borrowed(text));
    }
    match declared_encoding(bytes).as_deref() {
        Some("iso-8859-1" | "latin1" | "iso_8859-1" | "l1") => {
            Ok(Cow::Owned(bytes.iter().map(|b| char::from(*b)).collect()))
        }
        Some(other) => Err(XmlUtilsError::UnsupportedEncoding(other.to_owned())),
        None => Err(XmlUtilsError::UnsupportedEncoding(
            "UTF-8 inválido".to_owned(),
        )),
    }
}

impl XmlUtils {
    /// `DigestValue` del XML en `xml` (UTF-8, o ISO-8859-1 declarado).
    ///
    /// # Errors
    ///
    /// Ver [`XmlUtilsError`].
    pub fn hash_sign(&self, xml: &[u8]) -> Result<String, XmlUtilsError> {
        if xml.is_empty() {
            return Err(XmlUtilsError::Empty);
        }
        if xml.len() > self.max_bytes {
            return Err(XmlUtilsError::Limit(format!(
                "{} bytes (máximo {})",
                xml.len(),
                self.max_bytes
            )));
        }
        let text = decode(xml)?;
        if cpeforge_xml_limits::exceeds_depth(&text, self.max_depth) {
            return Err(XmlUtilsError::Limit(format!(
                "profundidad mayor que {}",
                self.max_depth
            )));
        }
        // `roxmltree` busca atributos duplicados en tiempo cuadrático y copia
        // el ámbito de espacios de nombres en cada elemento que declara uno.
        cpeforge_xml_limits::check_tree_markup(
            &text,
            cpeforge_xml_limits::DEFAULT_MAX_ATTRIBUTES,
            cpeforge_xml_limits::DEFAULT_MAX_NAMESPACE_SCOPE,
        )
        .map_err(|e| XmlUtilsError::Limit(e.to_string()))?;
        let options = roxmltree::ParsingOptions {
            allow_dtd: false,
            ..roxmltree::ParsingOptions::default()
        };
        let doc = roxmltree::Document::parse_with_options(&text, options).map_err(|e| match e {
            roxmltree::Error::DtdDetected => XmlUtilsError::Dtd,
            other => XmlUtilsError::Malformed(other.to_string()),
        })?;
        Ok(Self::hash_sign_from_document(&doc))
    }

    /// `DigestValue` del XML guardado en `path`; lee como mucho `max_bytes`.
    ///
    /// # Errors
    ///
    /// [`XmlUtilsError::Io`] si no se puede leer y los de
    /// [`XmlUtils::hash_sign`].
    pub fn hash_sign_from_file(&self, path: impl AsRef<Path>) -> Result<String, XmlUtilsError> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err(XmlUtilsError::Empty);
        }
        let file = fs::File::open(path).map_err(|e| XmlUtilsError::Io(e.to_string()))?;
        let limit = u64::try_from(self.max_bytes).unwrap_or(u64::MAX);
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| XmlUtilsError::Io(e.to_string()))?;
        self.hash_sign(&bytes)
    }

    /// `DigestValue` de un documento ya analizado; cadena vacía si no hay
    /// extensión con firma.
    #[must_use]
    pub fn hash_sign_from_document(doc: &roxmltree::Document<'_>) -> String {
        let root = doc.root_element();
        let extensions: Vec<roxmltree::Node<'_, '_>> =
            children(root, EXT_NAMESPACE, "UBLExtensions")
                .flat_map(|exts| children(exts, EXT_NAMESPACE, "UBLExtension"))
                .collect();
        for extension in extensions.iter().rev() {
            let digest = children(*extension, EXT_NAMESPACE, "ExtensionContent")
                .flat_map(|n| children(n, DS_NAMESPACE, "Signature"))
                .flat_map(|n| children(n, DS_NAMESPACE, "SignedInfo"))
                .flat_map(|n| children(n, DS_NAMESPACE, "Reference"))
                .flat_map(|n| children(n, DS_NAMESPACE, "DigestValue"))
                .next();
            if let Some(node) = digest {
                // Valor del elemento: todo su texto descendiente, CDATA incluido.
                return node
                    .descendants()
                    .filter(roxmltree::Node::is_text)
                    .filter_map(|n| n.text())
                    .collect();
            }
        }
        String::new()
    }
}

fn children<'a, 'input: 'a>(
    node: roxmltree::Node<'a, 'input>,
    namespace: &'static str,
    name: &'static str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    node.children().filter(move |child| {
        child.is_element()
            && child.tag_name().name() == name
            && child.tag_name().namespace() == Some(namespace)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_last_extension_digest() {
        let xml = format!(
            r#"<Invoice xmlns:ext="{EXT_NAMESPACE}" xmlns:ds="{DS_NAMESPACE}"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><ds:Signature><ds:SignedInfo><ds:Reference><ds:DigestValue>AAA</ds:DigestValue></ds:Reference></ds:SignedInfo></ds:Signature></ext:ExtensionContent></ext:UBLExtension><ext:UBLExtension><ext:ExtensionContent><ds:Signature><ds:SignedInfo><ds:Reference><ds:DigestValue>BBB</ds:DigestValue></ds:Reference><ds:Reference><ds:DigestValue>CCC</ds:DigestValue></ds:Reference></ds:SignedInfo></ds:Signature></ext:ExtensionContent></ext:UBLExtension></ext:UBLExtensions></Invoice>"#
        );
        assert_eq!(
            XmlUtils::default().hash_sign(xml.as_bytes()).unwrap(),
            "BBB"
        );
        assert_eq!(
            XmlUtils::default().hash_sign(b"<h1>EMPTY</h1>").unwrap(),
            ""
        );
        assert_eq!(
            XmlUtils::default().hash_sign(b""),
            Err(XmlUtilsError::Empty)
        );
        assert!(matches!(
            XmlUtils::default().hash_sign(b"<a>"),
            Err(XmlUtilsError::Malformed(_))
        ));
        assert_eq!(
            XmlUtils::default().hash_sign(b"<!DOCTYPE a [<!ENTITY e 'x'>]><a>&e;</a>"),
            Err(XmlUtilsError::Dtd)
        );
    }
}
