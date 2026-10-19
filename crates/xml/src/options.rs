//! Opciones de render: escape automático, modo estricto y reloj.

use jiff::Timestamp;

/// Escape automático de los valores impresos (`autoescape` de Twig).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Autoescape {
    /// Estrategia `html` (valor por defecto de Twig y de `TwigBuilder` sin
    /// opciones): `& < > " '` se escapan como `htmlspecialchars(ENT_QUOTES)`.
    /// Los valores con `|raw` (dentro de `CDATA`) no se escapan.
    #[default]
    Html,
    /// `autoescape => false`, como la referencia (`See` y `Api`). Los
    /// valores se insertan tal cual: un `&` o `<` produce XML mal formado,
    /// igual que en la referencia.
    Disabled,
}

/// Opciones de render.
///
/// `Default` reproduce `new InvoiceBuilder()` sin opciones: escape `html`,
/// `strict_variables` desactivado y reloj del sistema.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderOptions {
    /// Escape automático de los valores.
    pub autoescape: Autoescape,
    /// `strict_variables` de Twig: leer un atributo de un objeto nulo es un
    /// error en lugar de una cadena vacía.
    pub strict_variables: bool,
    /// Instante que se usa cuando una fecha impresa es nula. El filtro `date`
    /// de Twig imprime la hora actual en ese caso; `None` usa el reloj del
    /// sistema.
    pub now: Option<Timestamp>,
}

impl RenderOptions {
    /// Opciones de las fachadas `See` y `Api` del crate `cpeforge`: sin escape
    /// automático.
    #[must_use]
    pub fn cpeforge() -> Self {
        Self {
            autoescape: Autoescape::Disabled,
            ..Self::default()
        }
    }

    /// Opciones de las pruebas de referencia: `strict_variables` y sin escape.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            autoescape: Autoescape::Disabled,
            strict_variables: true,
            now: None,
        }
    }

    pub(crate) fn current_time(&self) -> Timestamp {
        self.now.unwrap_or_else(Timestamp::now)
    }
}
