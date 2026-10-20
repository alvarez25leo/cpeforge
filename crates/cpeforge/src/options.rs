//! Actualización parcial de las opciones de render de `xml`.

use cpeforge_xml::{Autoescape, RenderOptions};
use jiff::Timestamp;

/// Campos de `setBuilderOptions` soportados por los builders Rust. `None`
/// deja el valor previo.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct BuilderOptionsPatch {
    /// Estrategia de `autoescape`.
    pub autoescape: Option<Autoescape>,
    /// Modo estricto (`strict_variables`).
    pub strict_variables: Option<bool>,
    /// Reloj explícito del render; `Some(None)` vuelve al reloj del sistema.
    pub now: Option<Option<Timestamp>>,
}

impl BuilderOptionsPatch {
    pub(crate) fn apply(self, target: &mut RenderOptions) {
        if let Some(value) = self.autoescape {
            target.autoescape = value;
        }
        if let Some(value) = self.strict_variables {
            target.strict_variables = value;
        }
        if let Some(value) = self.now {
            target.now = value;
        }
    }
}
