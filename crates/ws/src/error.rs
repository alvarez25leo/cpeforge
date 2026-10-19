//! Errores tipados, con presentación segura para registros.

use std::fmt;

use cpeforge_gre_api::ApiError;

/// Fallo de una operación `ws`.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WsError {
    /// ZIP inválido, inseguro o fuera de límites.
    Zip(&'static str),
    /// XML inválido, inseguro o fuera de límites.
    Xml(&'static str),
    /// SOAP remoto o envelope inválido.
    Soap(&'static str),
    /// Fault SOAP con código y texto originales. El texto no aparece en logs.
    SoapFault {
        /// `faultcode`.
        code: String,
        /// `faultstring`.
        message: String,
        /// `detail` serializado como JSON compacto, si el fault lo trae.
        /// Tampoco aparece en logs.
        detail: Option<String>,
    },
    /// Respuesta incompleta o inesperada.
    InvalidResponse(&'static str),
    /// XML bien formado que no puede leerse como el documento esperado
    /// (CDR vacío, versión UBL no admitida). `Display` muestra el mensaje
    /// completo, que forma parte del contrato.
    XmlReader(String),
    /// El servicio respondió sin los datos obligatorios de la operación
    /// (estado de ticket ausente, token vacío). `Display` muestra el mensaje
    /// completo, que forma parte del contrato.
    InvalidServiceResponse(&'static str),
    /// HTTP o API GRE falló. Detalle accesible en la causa original.
    Transport,
    /// Fallo tipado de `gre-api`, con estado, cabeceras y cuerpo accesibles
    /// mediante la causa. `Debug` no muestra secretos.
    GreApi(ApiError),
    /// Entrada que podría escapar del contenedor o filtrar credenciales.
    Security(&'static str),
}

impl fmt::Display for WsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zip(reason) => write!(f, "ZIP inválido: {reason}"),
            Self::Xml(reason) => write!(f, "XML inválido: {reason}"),
            Self::Soap(reason) => write!(f, "SOAP inválido: {reason}"),
            Self::SoapFault { code, .. } => write!(f, "fault SOAP ({code})"),
            Self::InvalidResponse(reason) => write!(f, "respuesta inválida: {reason}"),
            Self::XmlReader(message) => f.write_str(message),
            Self::InvalidServiceResponse(message) => f.write_str(message),
            Self::Transport => f.write_str("fallo de transporte o API"),
            Self::GreApi(error) => write!(f, "gre-api falló ({})", error.code()),
            Self::Security(reason) => write!(f, "entrada rechazada: {reason}"),
        }
    }
}

impl fmt::Debug for WsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SoapFault { code, .. } => f
                .debug_struct("SoapFault")
                .field("code", code)
                .field("message", &"<oculto>")
                .finish(),
            Self::GreApi(error) => f.debug_tuple("GreApi").field(error).finish(),
            _ => fmt::Display::fmt(self, f),
        }
    }
}

impl std::error::Error for WsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GreApi(error) => Some(error),
            _ => None,
        }
    }
}
