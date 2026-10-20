//! Validación de comprobantes SUNAT (CPE) con hojas XSL.
//!
//! El crate **no distribuye** las reglas SUNAT: [`CpeValidator`] las
//! busca en un directorio local con las diez rutas de [`XslPathResolver`]
//! (`2.X/ValidaExprRegFactura-2.0.1.xsl`, …). El resultado depende, por
//! tanto, de la versión de las hojas que provea el consumidor; este crate no
//! certifica reglas que no suministra.
//!
//! Flujo: [`XmlTypeResolver`] detecta el tipo por la raíz del XML,
//! [`XslPathResolver`] ubica la hoja, [`XslValidator`] la ejecuta con
//! libxslt (XSLT 1.0 + EXSLT, el motor de `ext-xsl` de la referencia) pasando
//! `nombreArchivoEnviado`, y [`ErrorResultParser`] convierte cada
//! `xsl:message` `nivel|código|mensaje[|ruta|valor]` en un [`CpeError`]. Las
//! hojas pueden llamar a
//! `php:function('CPEForge\Validator\Xml\XsltFunctions::matches', patrón, valor)`
//! con semántica PCRE de la referencia.
//!
//! ```no_run
//! use cpeforge_cpe_validator::CpeValidator;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let validator = CpeValidator::new("/ruta/a/reglas-sunat");
//! let xml = std::fs::read("20000000001-01-F001-1.xml")?;
//! for error in validator.validate_xml("20000000001-01-F001-1.xml", &xml)? {
//!     println!("{:?} {:?}: {:?}", error.level, error.code, error.message);
//! }
//! # Ok(()) }
//! ```
//!
//! # Perfil seguro
//!
//! XML y XSL se tratan como entradas no confiables: se rechaza todo
//! `DOCTYPE`, no hay acceso de red ni escritura, `xsl:include`,
//! `xsl:import` y `document()` solo leen dentro del directorio autorizado
//! (con enlaces simbólicos resueltos), y hay [`Limits`] de tamaño, de
//! operaciones XPath y de PCRE. Los fallos de infraestructura son
//! [`CpeValidatorError`], nunca una lista vacía.
//!
//! # Requisitos
//!
//! libxml2, libxslt y libexslt del sistema (Linux: `libxslt1-dev`; macOS:
//! las del SDK). Windows no está soportado.

mod entity;
mod error;
mod ffi;
mod options;
mod parser;
mod resolver;
mod stylesheet;
mod validator;
mod xml;
mod xslt_ext;

pub use entity::{CpeError, DocumentType, ErrorLevel};
pub use error::{AccessKind, CpeValidatorError, InputSource, LimitKind};
pub use options::Limits;
pub use parser::{ErrorResultParser, ResultParser};
pub use resolver::{RuleResolver, TypeResolver, XmlTypeResolver, XslPathResolver};
pub use stylesheet::Stylesheet;
pub use validator::{CpeValidator, StylesheetValidator, Validator, XslValidator};
pub use xml::XmlDocument;
pub use xslt_ext::MATCHES_HANDLER;

/// Versiones de las bibliotecas nativas en tiempo de ejecución.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct EngineInfo {
    /// `xmlParserVersion` (p. ej. `20913`).
    pub libxml2: String,
    /// `xsltEngineVersion` (p. ej. `10135`).
    pub libxslt: String,
    /// Si [`Limits::xpath_operations`] se aplica con esta libxml2.
    pub xpath_operation_limit: bool,
}

/// Versiones de libxml2 y libxslt enlazadas.
#[must_use]
pub fn engine_info() -> EngineInfo {
    let (libxml2, libxslt) = ffi::runtime_versions();
    EngineInfo {
        libxml2,
        libxslt,
        xpath_operation_limit: ffi::op_limit_supported(),
    }
}
