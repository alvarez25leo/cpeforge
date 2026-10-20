//! Errores de las operaciones.
//!
//! Los nombres de clase (`CPEForge\Sunat\GRE\ApiException`, `TypeError`,
//! …) y los mensajes forman parte del contrato del crate: quien compare o
//! registre errores puede usarlos como datos.

use std::fmt;

use crate::model::{CpeError, CpeErrorValidation};
use crate::transport::TransportErrorKind;

/// Cabeceras de respuesta agrupadas por nombre, en el orden recibido. Los
/// nombres se comparan sin distinguir mayúsculas.
pub type Headers = Vec<(String, Vec<String>)>;

/// Cuerpo de error tipado ([`ApiError::response_object`]).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorObject {
    /// HTTP 500 de `CpeApi`.
    Server(CpeError),
    /// HTTP 422 de `CpeApi`.
    Validation(CpeErrorValidation),
}

/// Error de una operación.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApiError {
    /// Respuesta HTTP fuera de 2xx: un 4xx/5xx con el mensaje detallado
    /// (método, URI, estado y resumen del cuerpo) u otro estado con
    /// `Error connecting to the API`. Clase `ApiException`.
    Http {
        /// Código de estado.
        code: u16,
        /// Mensaje, con el prefijo `[estado] `.
        message: String,
        /// Cabeceras.
        headers: Headers,
        /// Cuerpo sin decodificar.
        body: Vec<u8>,
        /// Cuerpo tipado para 422/500 de `CpeApi`.
        object: Option<Box<ErrorObject>>,
    },
    /// Sin respuesta (conexión, tiempo límite, conexión cerrada): código 0,
    /// sin cabeceras ni cuerpo. Clase `ApiException`.
    Transport {
        /// Tipo de fallo.
        kind: TransportErrorKind,
        /// Mensaje, con el prefijo `[0] `.
        message: String,
    },
    /// Respuesta que no se puede leer como el modelo esperado: 2xx sin cuerpo
    /// JSON, valores imposibles de convertir, error al leer el cuerpo de un
    /// 422/500, o un fallo de red en las formas con errores genéricos.
    /// `class` es `TypeError`, `InvalidArgumentException`,
    /// `DateMalformedStringException` o `Error`.
    Decode {
        /// Clase del error.
        class: &'static str,
        /// Mensaje.
        message: String,
    },
    /// Solicitud rechazada antes de enviarla: un parámetro de ruta `.` o `..`
    /// (la URL se normalizaría y el token o las credenciales irían a otra
    /// ruta del host). Código 0, clase `InvalidArgumentException`.
    InvalidRequest {
        /// Mensaje.
        message: String,
    },
}

impl ApiError {
    /// Estado HTTP, o 0 sin respuesta.
    #[must_use]
    pub fn code(&self) -> u16 {
        match self {
            Self::Http { code, .. } => *code,
            Self::Transport { .. } | Self::Decode { .. } | Self::InvalidRequest { .. } => 0,
        }
    }

    /// Mensaje completo; puede incluir datos del cuerpo de la respuesta.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Http { message, .. }
            | Self::Transport { message, .. }
            | Self::Decode { message, .. }
            | Self::InvalidRequest { message } => message,
        }
    }

    /// Clase del error: `CPEForge\Sunat\GRE\ApiException` para
    /// [`Self::Http`] y [`Self::Transport`], `InvalidArgumentException` para
    /// [`Self::InvalidRequest`] o la `class` de [`Self::Decode`].
    #[must_use]
    pub fn error_class(&self) -> &'static str {
        match self {
            Self::Http { .. } | Self::Transport { .. } => "CPEForge\\Sunat\\GRE\\ApiException",
            Self::Decode { class, .. } => class,
            Self::InvalidRequest { .. } => "InvalidArgumentException",
        }
    }

    /// Cabeceras de la respuesta, si la hubo.
    #[must_use]
    pub fn response_headers(&self) -> Option<&Headers> {
        match self {
            Self::Http { headers, .. } => Some(headers),
            _ => None,
        }
    }

    /// Cuerpo de la respuesta, si la hubo.
    #[must_use]
    pub fn response_body(&self) -> Option<&[u8]> {
        match self {
            Self::Http { body, .. } => Some(body),
            _ => None,
        }
    }

    /// Cuerpo de error tipado (422/500 de `CpeApi` sin errores genéricos).
    #[must_use]
    pub fn response_object(&self) -> Option<&ErrorObject> {
        match self {
            Self::Http { object, .. } => object.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http { code, .. } => write!(
                f,
                "error HTTP GRE ({code}); consulte message() en un contexto seguro"
            ),
            Self::Transport { .. } => {
                f.write_str("error de transporte GRE; consulte message() en un contexto seguro")
            }
            Self::Decode { class, .. } => write!(
                f,
                "error de respuesta GRE ({class}); consulte message() en un contexto seguro"
            ),
            Self::InvalidRequest { .. } => f.write_str(
                "solicitud GRE rechazada antes de enviarla; consulte message() en un contexto seguro",
            ),
        }
    }
}

impl fmt::Debug for ApiError {
    /// Oculta el mensaje y el cuerpo, que pueden contener datos de la
    /// respuesta; muestra solo su tamaño.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http { code, body, .. } => f
                .debug_struct("Http")
                .field("code", code)
                .field("message", &"<oculto>")
                .field("object", &"<oculto>")
                .field("body", &format_args!("<{} bytes>", body.len()))
                .finish(),
            Self::Transport { kind, .. } => f
                .debug_struct("Transport")
                .field("kind", kind)
                .field("message", &"<oculto>")
                .finish(),
            Self::Decode { class, .. } => f
                .debug_struct("Decode")
                .field("class", class)
                .field("message", &"<oculto>")
                .finish(),
            Self::InvalidRequest { .. } => f
                .debug_struct("InvalidRequest")
                .field("message", &"<oculto>")
                .finish(),
        }
    }
}

impl std::error::Error for ApiError {}
