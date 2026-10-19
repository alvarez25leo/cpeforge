//! `CPEForge\Validator\Parser`: de mensaje de `xsl:message` a [`CpeError`].

use crate::entity::{CpeError, ErrorLevel};

/// Convierte un mensaje de la hoja en un error (`ResultParserInterface`).
///
/// La referencia acepta `?string` pero `ErrorResultParser` falla con `TypeError` ante
/// `null` (`explode()` en modo estricto); aquí la entrada siempre es texto.
pub trait ResultParser {
    /// `None` si el mensaje no tiene formato de error.
    fn parse(&self, raw: &str) -> Option<CpeError>;
}

/// `ErrorResultParser`: `nivel|código|mensaje[|ruta|valor]`.
///
/// Con menos de tres campos no hay error. El nivel es excepción solo si el
/// primer campo es exactamente `1`; cualquier otro valor es observación.
/// Ruta y valor se toman únicamente con cinco campos o más; el resto se
/// ignora. No se recortan espacios.
#[derive(Clone, Copy, Debug, Default)]
pub struct ErrorResultParser;

impl ResultParser for ErrorResultParser {
    fn parse(&self, raw: &str) -> Option<CpeError> {
        let parts: Vec<&str> = raw.split('|').collect();
        if parts.len() < 3 {
            return None;
        }
        let level = if parts[0] == ErrorLevel::Exception.code() {
            ErrorLevel::Exception
        } else {
            ErrorLevel::Observation
        };
        let mut error = CpeError {
            code: Some(parts[1].to_owned()),
            message: Some(parts[2].to_owned()),
            level: Some(level),
            ..CpeError::default()
        };
        if parts.len() >= 5 {
            error.node_path = Some(parts[3].to_owned());
            error.node_value = Some(parts[4].to_owned());
        }
        Some(error)
    }
}

impl<P: ResultParser + ?Sized> ResultParser for &P {
    fn parse(&self, raw: &str) -> Option<CpeError> {
        (**self).parse(raw)
    }
}
