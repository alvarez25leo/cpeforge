//! Parsers por tipo de comprobante.
//!
//! Cada parser implementa [`DocumentParser`] con entrada `[u8]` (el XML) y
//! ofrece además `parse_document` para un [`XmlDocument`] ya analizado (el
//! documento ya analizado). No guardan estado entre llamadas.

mod despatch;
mod invoice;
mod note;
mod perception;
mod receipt;
mod retention;
mod summary;
mod voided;

pub use despatch::DespatchParser;
pub use invoice::InvoiceParser;
pub use note::NoteParser;
pub use perception::PerceptionParser;
pub use receipt::ReceiptParser;
pub use retention::RetentionParser;
pub use summary::SummaryParser;
pub use voided::{VoidedDocument, VoidedParser};

use cpeforge_core::model::company::Address;

use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::Lookup;

/// Implementa constructor, `parse_document` y [`DocumentParser`] para un parser.
macro_rules! parser {
    ($name:ident, $output:ty, $model:literal) => {
        #[doc = concat!("Lee un XML UBL y devuelve el modelo `", $model, "`.")]
        ///
        /// Ver [`ParseOptions`](crate::ParseOptions) para la zona horaria, el
        /// reloj y los límites.
        #[derive(Debug, Clone, Default)]
        pub struct $name {
            options: $crate::ParseOptions,
        }

        impl $name {
            /// Parser con [`ParseOptions::default`](crate::ParseOptions::default).
            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }

            /// Parser con opciones propias.
            #[must_use]
            pub fn with_options(options: $crate::ParseOptions) -> Self {
                Self { options }
            }

            /// Opciones del parser.
            #[must_use]
            pub fn options(&self) -> &$crate::ParseOptions {
                &self.options
            }

            /// Interpreta un documento ya analizado.
            ///
            /// # Errors
            ///
            /// [`XmlParserError`](crate::XmlParserError) si falta un nodo
            /// obligatorio o un valor no se puede convertir.
            pub fn parse_document(
                &self,
                document: &$crate::XmlDocument,
            ) -> Result<$output, $crate::XmlParserError> {
                let xml = $crate::reader::Lookup { document };
                parse(&xml, &self.options.date_context())
            }
        }

        impl cpeforge_core::ports::DocumentParser for $name {
            type Input = [u8];
            type Output = $output;
            type Error = $crate::XmlParserError;

            /// Analiza el XML con los límites configurados y lo interpreta.
            /// Nunca devuelve `Ok(None)`.
            fn parse(&self, value: &[u8]) -> Result<Option<$output>, $crate::XmlParserError> {
                let document =
                    $crate::XmlDocument::parse_with_limits(value, self.options.limits())?;
                self.parse_document(&document).map(Some)
            }
        }
    };
}

pub(crate) use parser;

/// Dirección completa (calle, ciudad, provincia, distrito y ubigeo) si existe
/// el nodo `query` bajo `node`; la usan notas, retenciones y percepciones.
fn full_address<'a>(
    xml: &Lookup<'a>,
    query: &str,
    node: Option<Node<'a>>,
) -> Result<Option<Address>, XmlParserError> {
    let Some(address) = xml.node(query, node)? else {
        return Ok(None);
    };
    let at = Some(address);
    Ok(Some(Address {
        direccion: Some(xml.value("cbc:StreetName", at)?),
        departamento: Some(xml.value("cbc:CityName", at)?),
        provincia: Some(xml.value("cbc:CountrySubentity", at)?),
        distrito: Some(xml.value("cbc:District", at)?),
        ubigueo: Some(xml.value("cbc:ID", at)?),
        ..Address::default()
    }))
}
