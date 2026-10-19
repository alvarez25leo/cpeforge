use std::fmt;

/// Fallos tipados de la orquestación de `cpeforge`.
pub enum CpeForgeError {
    /// Nombre u otros datos del documento.
    Document(cpeforge_core::CoreError),
    /// Builder XML.
    Xml(cpeforge_xml::XmlError),
    /// Firma XML.
    Signature(cpeforge_xmldsig::Error),
    /// SOAP, ZIP, CDR o GRE.
    Service(cpeforge_ws::WsError),
    /// Falta una dependencia configurada en `FeFactory`.
    Missing(&'static str),
    /// Tipo de documento no admitido por el resolver tipado.
    UnsupportedDocument,
}

impl fmt::Debug for CpeForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(_) => f.write_str("CpeForgeError::Document(<oculto>)"),
            Self::Xml(_) => f.write_str("CpeForgeError::Xml(<oculto>)"),
            Self::Signature(_) => f.write_str("CpeForgeError::Signature(<oculto>)"),
            Self::Service(_) => f.write_str("CpeForgeError::Service(<oculto>)"),
            Self::Missing(name) => f.debug_tuple("CpeForgeError::Missing").field(name).finish(),
            Self::UnsupportedDocument => f.write_str("CpeForgeError::UnsupportedDocument"),
        }
    }
}

impl fmt::Display for CpeForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(_) => f.write_str("documento inválido"),
            Self::Xml(_) => f.write_str("no se pudo generar XML"),
            Self::Signature(_) => f.write_str("no se pudo firmar XML"),
            Self::Service(_) => f.write_str("falló el servicio"),
            Self::Missing(name) => write!(f, "falta {name}"),
            Self::UnsupportedDocument => f.write_str("tipo de documento no admitido"),
        }
    }
}

impl std::error::Error for CpeForgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Xml(error) => Some(error),
            Self::Signature(error) => Some(error),
            Self::Service(error) => Some(error),
            Self::Missing(_) | Self::UnsupportedDocument => None,
        }
    }
}

impl From<cpeforge_core::CoreError> for CpeForgeError {
    fn from(value: cpeforge_core::CoreError) -> Self {
        Self::Document(value)
    }
}
impl From<cpeforge_xml::XmlError> for CpeForgeError {
    fn from(value: cpeforge_xml::XmlError) -> Self {
        Self::Xml(value)
    }
}
impl From<cpeforge_xmldsig::Error> for CpeForgeError {
    fn from(value: cpeforge_xmldsig::Error) -> Self {
        Self::Signature(value)
    }
}
impl From<cpeforge_ws::WsError> for CpeForgeError {
    fn from(value: cpeforge_ws::WsError) -> Self {
        Self::Service(value)
    }
}
