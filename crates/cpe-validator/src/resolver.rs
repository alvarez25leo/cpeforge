//! `CPEForge\Validator\Resolver`: tipo de documento y ruta de la regla.

use std::ffi::OsString;
use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};

use crate::entity::DocumentType;
use crate::error::CpeValidatorError;
use crate::options::Limits;
use crate::xml::XmlDocument;

/// Detecta el tipo de un XML (`TypeResolverInterface`).
pub trait TypeResolver {
    /// `Ok(None)` si el tipo no es reconocido.
    ///
    /// # Errors
    ///
    /// Cuando la referencia lanza (p. ej. [`CpeValidatorError::VoidedWithoutId`]).
    fn resolve(&self, document: &XmlDocument) -> Result<Option<DocumentType>, CpeValidatorError>;
}

/// `XmlTypeResolver`: decide por el nombre local de la raíz, no por su
/// espacio de nombres.
///
/// - `Invoice`: `03` si el primer `cbc:InvoiceTypeCode` hijo de la raíz vale
///   exactamente `03` (texto de todos sus descendientes); `01` en otro caso.
/// - `VoidedDocuments`: `RR` si el primer `cbc:ID` hijo empieza por `RR`
///   (mayúsculas); `RA` si no. Sin `cbc:ID` falla como la referencia.
/// - `CreditNote` `07`, `DebitNote` `08`, `SummaryDocuments` `RC`,
///   `DespatchAdvice` `09`, `Retention` `20`, `Perception` `40`.
///
/// El prefijo `cbc` se resuelve con las declaraciones visibles en la raíz
/// (como `DOMXPath::query` con `registerNodeNS`), sea cual sea su URI; si la
/// raíz no declara `cbc`, la consulta falla y se trata como ausente.
#[derive(Clone, Copy, Debug, Default)]
pub struct XmlTypeResolver;

impl XmlTypeResolver {
    /// `getTypeFromXml`: analiza `xml` y detecta el tipo.
    ///
    /// # Errors
    ///
    /// Errores de [`XmlDocument::parse`] (la referencia devuelve `null` para XML mal
    /// formado y `ValueError` para XML vacío) y los de [`TypeResolver::resolve`].
    pub fn resolve_xml(
        &self,
        xml: impl AsRef<[u8]>,
    ) -> Result<Option<DocumentType>, CpeValidatorError> {
        self.resolve(&XmlDocument::parse_with_limits(xml, &Limits::default())?)
    }
}

impl TypeResolver for XmlTypeResolver {
    fn resolve(&self, document: &XmlDocument) -> Result<Option<DocumentType>, CpeValidatorError> {
        let Some(name) = document.root_name() else {
            return Ok(None);
        };
        Ok(Some(match name.as_str() {
            "Invoice" => match document.doc.root_text("cbc:InvoiceTypeCode").as_deref() {
                Some("03") => DocumentType::Boleta,
                _ => DocumentType::Factura,
            },
            "CreditNote" => DocumentType::NotaCredito,
            "DebitNote" => DocumentType::NotaDebito,
            "SummaryDocuments" => DocumentType::ResumenDiario,
            "VoidedDocuments" => match document.doc.root_text("cbc:ID") {
                Some(id) if id.starts_with(DocumentType::ResumenReversion.code()) => {
                    DocumentType::ResumenReversion
                }
                Some(_) => DocumentType::ComunicacionBaja,
                None => return Err(CpeValidatorError::VoidedWithoutId),
            },
            "DespatchAdvice" => DocumentType::GuiaRemision,
            "Retention" => DocumentType::Retencion,
            "Perception" => DocumentType::Percepcion,
            _ => return Ok(None),
        }))
    }
}

/// Ubica la hoja de un tipo (`RuleResolverInterface`).
pub trait RuleResolver {
    /// Ruta de la hoja, o `None` si el tipo no tiene regla.
    fn rule_path(&self, document_type: DocumentType) -> Option<PathBuf>;

    /// Directorio autorizado para la hoja y sus `xsl:include`,
    /// `xsl:import` y `document()`. Con `None` se usa el directorio de la
    /// propia hoja.
    fn rules_root(&self) -> Option<&Path> {
        None
    }
}

/// `XslPathResolver`: `basePath + DIRECTORY_SEPARATOR + ruta fija` para los
/// diez tipos. No comprueba existencia, versión ni hash de la hoja: el
/// resultado depende de las reglas que provea el consumidor (el paquete de referencia
/// no distribuye las reglas SUNAT).
#[derive(Clone, Debug)]
pub struct XslPathResolver {
    base: PathBuf,
    root: PathBuf,
}

impl XslPathResolver {
    /// Resolutor sobre `base`, que también es el directorio autorizado.
    #[must_use]
    pub fn new(base: impl Into<PathBuf>) -> Self {
        let base = base.into();
        Self {
            root: base.clone(),
            base,
        }
    }

    /// Autoriza un directorio más amplio que `base` (p. ej. reglas que
    /// incluyen `../commons/...`).
    #[must_use]
    pub fn with_rules_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = root.into();
        self
    }

    /// Ruta relativa de la hoja de cada tipo (mapa de la referencia).
    #[must_use]
    pub const fn relative_path(document_type: DocumentType) -> &'static str {
        match document_type {
            DocumentType::Factura => "2.X/ValidaExprRegFactura-2.0.1.xsl",
            DocumentType::Boleta => "2.X/ValidaExprRegBoleta-2.0.1.xsl",
            DocumentType::NotaCredito => "2.X/ValidaExprRegNC-2.0.1.xsl",
            DocumentType::NotaDebito => "2.X/ValidaExprRegND-2.0.1.xsl",
            DocumentType::ResumenDiario => "1.X/ValidaExprRegSummary-1.1.0.xsl",
            DocumentType::ComunicacionBaja => "1.X/ValidaExprRegSummaryVoided-1.0.1.xsl",
            DocumentType::GuiaRemision => "2.X/ValidaExprRegGreRemitente-2.0.1.xsl",
            DocumentType::Retencion => "1.X/ValidaExprRegRetencion-1.0.3.xsl",
            DocumentType::Percepcion => "1.X/ValidaExprRegPercepcion-1.0.1.xsl",
            DocumentType::ResumenReversion => "1.X/ValidaExprRegOtrosVoided-1.0.1.xsl",
        }
    }

    /// `getPath` con un código de texto: `None` si no es uno de los diez.
    /// La ruta se concatena como en la referencia (sin normalizar `//`).
    #[must_use]
    pub fn path_for_code(&self, code: &str) -> Option<PathBuf> {
        DocumentType::from_code(code).and_then(|t| self.rule_path(t))
    }
}

impl RuleResolver for XslPathResolver {
    fn rule_path(&self, document_type: DocumentType) -> Option<PathBuf> {
        let mut path = OsString::from(self.base.as_os_str());
        path.push(MAIN_SEPARATOR_STR);
        path.push(Self::relative_path(document_type));
        Some(PathBuf::from(path))
    }

    fn rules_root(&self) -> Option<&Path> {
        Some(&self.root)
    }
}
