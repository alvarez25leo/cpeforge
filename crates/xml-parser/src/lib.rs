//! Lectura de comprobantes electrónicos SUNAT (XML UBL) a los modelos de
//! [`cpeforge_core`], equivalente a la referencia.
//!
//! Hay un parser por familia: [`InvoiceParser`], [`NoteParser`],
//! [`DespatchParser`], [`ReceiptParser`], [`SummaryParser`],
//! [`VoidedParser`], [`RetentionParser`] y [`PerceptionParser`]. Todos
//! implementan [`DocumentParser`](cpeforge_core::ports::DocumentParser)
//! con entrada `[u8]`. Las rutas XML, los valores por defecto, las
//! conversiones de importes y fechas y los errores forman parte del
//! contrato del crate. [`XmlReader`] ofrece las mismas consultas
//! XPath para leer otros campos.
//!
//! # Perfil seguro
//!
//! El XML se lee con [`XmlDocument`], que rechaza el XML mal formado, las DTD con declaraciones (sin expansión de
//! entidades ni acceso a recursos externos) y las entradas que superan los
//! [`Limits`] de tamaño, profundidad y número de nodos. Nunca accede a la red
//! ni al sistema de archivos.
//!
//! # Ejemplo
//!
//! ```
//! use cpeforge_core::ports::DocumentParser;
//! use cpeforge_xml_parser::VoidedParser;
//!
//! let xml = r#"<VoidedDocuments
//!   xmlns="urn:sunat:names:specification:ubl:peru:schema:xsd:VoidedDocuments-1"
//!   xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"
//!   xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2">
//!   <cbc:ID>RA-20240601-7</cbc:ID>
//!   <cbc:ReferenceDate>2024-05-30</cbc:ReferenceDate>
//!   <cbc:IssueDate>2024-06-01</cbc:IssueDate>
//! </VoidedDocuments>"#;
//! let document = VoidedParser::new().parse(xml.as_bytes())?.ok_or("sin documento")?;
//! assert_eq!(document.class_name(), "Voided");
//! assert_eq!(document.voided().correlativo.as_deref(), Some("7"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod convert;
mod dom;
mod encoding;
mod error;
mod options;
mod parser;
mod reader;
mod xpath;

pub use dom::{Limits, Node, XmlDocument};
pub use error::XmlParserError;
pub use options::ParseOptions;
pub use parser::{
    DespatchParser, InvoiceParser, NoteParser, PerceptionParser, ReceiptParser, RetentionParser,
    SummaryParser, VoidedDocument, VoidedParser,
};
pub use reader::XmlReader;
