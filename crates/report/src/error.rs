//! Errores tipados del crate.

use std::fmt;

use cpeforge_core::CoreError;

use crate::qr::QrError;

/// Causa de un fallo durante el render.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderCause {
    /// Variable, clave o atributo inexistente con `strict_variables`, también
    /// al leer un atributo de un valor nulo.
    Undefined,
    /// Argumento de tipo incorrecto para un filtro o función, por ejemplo un
    /// importe nulo en `n_format` o un logo nulo en `image_b64`.
    TypeError {
        /// Identificador estable de la operación que rechaza el argumento
        /// (`number_format`, `substr`, `date`, …).
        function: &'static str,
    },
    /// Getter de un modelo que falla por un dato ausente, como `name` sin
    /// emisor o la fecha de emisión nula al construir el QR.
    Document(CoreError),
    /// El objeto no tiene el getter que la función necesita, por ejemplo
    /// `qrCode` con una guía de remisión.
    UndefinedMethod {
        /// Clase del modelo.
        class: String,
        /// Método ausente.
        method: &'static str,
    },
    /// Fallo del generador QR (contenido vacío o demasiado grande).
    Qr(QrError),
    /// Valor sin representación de texto (un modelo o una fecha impresos
    /// directamente).
    NotPrintable,
    /// Límite de recursos superado (tamaño del logo o combustible del motor).
    Limit(String),
    /// Error del motor de plantillas sin equivalente más específico.
    Engine,
}

impl fmt::Display for RenderCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Undefined => f.write_str("variable o atributo inexistente"),
            Self::TypeError { function } => write!(f, "TypeError en {function}()"),
            Self::Document(err) => write!(f, "{err}"),
            Self::UndefinedMethod { class, method } => {
                write!(f, "método inexistente {class}::{method}()")
            }
            Self::Qr(err) => write!(f, "{err}"),
            Self::NotPrintable => f.write_str("valor no convertible en texto"),
            Self::Limit(what) => write!(f, "límite superado: {what}"),
            Self::Engine => f.write_str("error del motor de plantillas"),
        }
    }
}

/// Error de [`crate::HtmlReport`] y [`crate::DefaultTemplateResolver`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportError {
    /// La plantilla no existe en la carpeta del usuario ni entre las
    /// integradas.
    TemplateNotFound {
        /// Nombre pedido.
        name: String,
    },
    /// Nombre de plantilla rechazado antes de buscarlo: sale de las raíces
    /// autorizadas (`..`), es absoluto o contiene NUL.
    InvalidTemplateName {
        /// Nombre pedido.
        name: String,
        /// Motivo del rechazo.
        reason: &'static str,
    },
    /// No se pudo leer una plantilla de la carpeta del usuario.
    TemplateIo {
        /// Nombre de la plantilla.
        name: String,
        /// Mensaje del sistema operativo o del límite aplicado.
        message: String,
    },
    /// Error de sintaxis de la plantilla, incluidos filtros, funciones o tests
    /// desconocidos.
    Syntax {
        /// Plantilla donde se detectó.
        template: Option<String>,
        /// Línea, si el motor la conoce.
        line: Option<usize>,
        /// Descripción del motor.
        message: String,
    },
    /// Error durante el render.
    Render {
        /// Plantilla donde ocurrió.
        template: Option<String>,
        /// Línea, si el motor la conoce.
        line: Option<usize>,
        /// Causa tipada.
        cause: RenderCause,
        /// Descripción completa del motor.
        message: String,
    },
    /// [`crate::DefaultTemplateResolver`] no tiene plantilla para el tipo de
    /// documento.
    InvalidDocument {
        /// Identificador de clase del modelo (`CPEForge\Model\Sale\Receipt`).
        class: String,
    },
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemplateNotFound { name } => write!(f, "Unable to find template \"{name}\""),
            Self::InvalidTemplateName { name, reason } => {
                write!(f, "nombre de plantilla inválido \"{name}\": {reason}")
            }
            Self::TemplateIo { name, message } => {
                write!(f, "no se pudo leer la plantilla \"{name}\": {message}")
            }
            Self::Syntax {
                template, message, ..
            } => write!(
                f,
                "error de sintaxis en {}: {message}",
                template.as_deref().unwrap_or("?")
            ),
            Self::Render {
                template, cause, ..
            } => write!(
                f,
                "error al representar {}: {cause}",
                template.as_deref().unwrap_or("?")
            ),
            Self::InvalidDocument { class } => write!(f, "Not found template for {class}"),
        }
    }
}

impl std::error::Error for ReportError {}

/// Error propio que los filtros adjuntan a `minijinja::Error` para conservar
/// la causa tipada a través del motor.
#[derive(Debug)]
pub(crate) struct CauseError(pub(crate) RenderCause);

impl fmt::Display for CauseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for CauseError {}

/// Fallo del cargador de plantillas transportado dentro de `minijinja::Error`.
#[derive(Debug)]
pub(crate) struct LoaderFailure(pub(crate) ReportError);

impl fmt::Display for LoaderFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for LoaderFailure {}

/// `minijinja::Error` con una causa tipada.
pub(crate) fn engine_error(cause: RenderCause) -> minijinja::Error {
    minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, cause.to_string())
        .with_source(CauseError(cause))
}

/// Primer texto entre comillas dobles.
fn quoted(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let len = text[start..].find('"')?;
    Some(text[start..start + len].to_owned())
}

/// Traduce un error del motor a [`ReportError`].
pub(crate) fn from_engine(err: &minijinja::Error) -> ReportError {
    use minijinja::ErrorKind;
    let template = err.name().map(str::to_owned);
    let line = err.line();
    let message = format!("{err:#}");
    let mut source: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(err);
    while let Some(current) = source {
        if let Some(found) = current.downcast_ref::<LoaderFailure>() {
            return found.0.clone();
        }
        source = current.source();
    }
    match err.kind() {
        ErrorKind::TemplateNotFound => ReportError::TemplateNotFound {
            name: quoted(err.detail().unwrap_or_default())
                .or(template)
                .unwrap_or_default(),
        },
        // Un filtro, función o test desconocido es un error de la plantilla,
        // no de los datos: se informa como sintaxis.
        ErrorKind::SyntaxError
        | ErrorKind::BadEscape
        | ErrorKind::UnknownFunction
        | ErrorKind::UnknownFilter
        | ErrorKind::UnknownTest => ReportError::Syntax {
            template,
            line,
            message,
        },
        kind => {
            let mut cause = None;
            let mut source: Option<&(dyn std::error::Error + 'static)> =
                std::error::Error::source(err);
            while let Some(current) = source {
                if let Some(found) = current.downcast_ref::<LoaderFailure>() {
                    return found.0.clone();
                }
                if let Some(found) = current.downcast_ref::<CauseError>() {
                    cause = Some(found.0.clone());
                    break;
                }
                if let Some(inner) = current.downcast_ref::<minijinja::Error>() {
                    if inner.kind() == ErrorKind::TemplateNotFound {
                        return ReportError::TemplateNotFound {
                            name: inner.name().map(str::to_owned).unwrap_or_default(),
                        };
                    }
                }
                source = current.source();
            }
            let cause = cause.unwrap_or(match kind {
                ErrorKind::UndefinedError => RenderCause::Undefined,
                ErrorKind::OutOfFuel => RenderCause::Limit("combustible del motor".to_owned()),
                _ => RenderCause::Engine,
            });
            ReportError::Render {
                template,
                line,
                cause,
                message,
            }
        }
    }
}
