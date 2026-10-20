//! Documento XML analizado con libxml2 (el `DOMDocument` de la referencia).

use cpeforge_xml_limits::{DEFAULT_MAX_ATTRIBUTES, MarkupRejection, check_markup};

use crate::error::{CpeValidatorError, InputSource};
use crate::ffi::{self, Parsed};
use crate::options::Limits;

/// XML ya analizado, reutilizable en varias validaciones.
///
/// Perfil seguro: sin red, sin entidades externas ni sustitución de
/// entidades, y se rechaza cualquier `DOCTYPE`. El documento no se modifica
/// al validar (libxslt trabaja sobre una copia).
pub struct XmlDocument {
    pub(crate) doc: ffi::Doc,
}

impl std::fmt::Debug for XmlDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XmlDocument")
            .field("root", &self.root_name())
            .finish()
    }
}

impl XmlDocument {
    /// Analiza `xml` con los límites por defecto.
    ///
    /// # Errors
    ///
    /// [`CpeValidatorError::EmptyXml`], [`CpeValidatorError::TooLarge`],
    /// [`CpeValidatorError::DoctypeForbidden`] o
    /// [`CpeValidatorError::MalformedXml`].
    pub fn parse(xml: impl AsRef<[u8]>) -> Result<Self, CpeValidatorError> {
        Self::parse_with_limits(xml, &Limits::default())
    }

    /// Analiza `xml` con `limits`.
    ///
    /// # Errors
    ///
    /// Igual que [`XmlDocument::parse`].
    pub fn parse_with_limits(
        xml: impl AsRef<[u8]>,
        limits: &Limits,
    ) -> Result<Self, CpeValidatorError> {
        let bytes = xml.as_ref();
        if bytes.is_empty() {
            return Err(CpeValidatorError::EmptyXml);
        }
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual > limits.max_xml_bytes {
            return Err(CpeValidatorError::TooLarge {
                source: InputSource::Xml,
                limit: limits.max_xml_bytes,
                actual,
            });
        }
        match check_markup(bytes, DEFAULT_MAX_ATTRIBUTES) {
            Ok(()) => {}
            Err(MarkupRejection::Declaration) => {
                return Err(CpeValidatorError::DoctypeForbidden {
                    source: InputSource::Xml,
                });
            }
            Err(rejection) => {
                return Err(CpeValidatorError::MalformedXml {
                    diagnostics: vec![rejection.to_string()],
                });
            }
        }
        match ffi::parse_xml(bytes).map_err(CpeValidatorError::Engine)? {
            Parsed::Doc(doc) => Ok(Self { doc }),
            Parsed::Dtd => Err(CpeValidatorError::DoctypeForbidden {
                source: InputSource::Xml,
            }),
            Parsed::Malformed(diagnostics) => Err(CpeValidatorError::MalformedXml { diagnostics }),
        }
    }

    /// `localName` del elemento raíz como lo expone la referencia (con un prefijo sin
    /// declarar incluye `prefijo:`).
    #[must_use]
    pub fn root_name(&self) -> Option<String> {
        self.doc.root_name()
    }
}
