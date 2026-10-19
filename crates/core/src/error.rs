use std::fmt;

/// Errores de las operaciones derivadas de los modelos.
///
/// No valida al construir: falla al ejecutar el método que
/// necesita un dato ausente (por ejemplo `getName()` sin `company` lanza
/// `Error: Call to a member function getRuc() on null`). Este crate devuelve
/// el mismo fallo como un valor tipado.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoreError {
    /// Un campo necesario para la operación es `None`.
    MissingField {
        /// Modelo que contiene el campo, con el nombre de clase.
        model: &'static str,
        /// Nombre de la propiedad ausente.
        field: &'static str,
    },
    /// No se pudo resolver la zona horaria de Perú en la base tz incluida.
    TimeZone {
        /// Descripción del fallo devuelta por `jiff`.
        message: String,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { model, field } => write!(f, "{model}: falta el campo `{field}`"),
            Self::TimeZone { message } => {
                write!(f, "zona horaria de Perú no disponible: {message}")
            }
        }
    }
}

impl std::error::Error for CoreError {}

impl CoreError {
    pub(crate) const fn missing(model: &'static str, field: &'static str) -> Self {
        Self::MissingField { model, field }
    }
}
