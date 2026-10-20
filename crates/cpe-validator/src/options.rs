//! Límites del perfil seguro.

/// Límites aplicados a XML, hojas y ejecución.
///
/// Los valores por defecto superan con margen un CPE y un juego de reglas
/// SUNAT reales; la referencia no tiene límites de tamaño ni de operaciones XPath.
/// Los de PCRE son los de `pcre.backtrack_limit` y `pcre.recursion_limit`
/// de la referencia, y la profundidad de plantillas es la de libxslt
/// (`xsltMaxDepth`, 3000), igual que en la referencia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// Tamaño máximo del XML a validar (bytes).
    pub max_xml_bytes: u64,
    /// Tamaño máximo de cada hoja, include, import o `document()` (bytes).
    pub max_stylesheet_bytes: u64,
    /// Operaciones XPath por transformación (`opLimit` de libxml2). `0`
    /// desactiva el límite. Si la libxml2 del sistema no lo admite, la
    /// transformación se ejecuta sin él (véase [`crate::engine_info`]).
    pub xpath_operations: u64,
    /// `pcre.backtrack_limit`.
    pub pcre_backtrack_limit: u32,
    /// `pcre.recursion_limit`.
    pub pcre_recursion_limit: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_xml_bytes: 16 * 1024 * 1024,
            max_stylesheet_bytes: 8 * 1024 * 1024,
            xpath_operations: 200_000_000,
            pcre_backtrack_limit: 1_000_000,
            pcre_recursion_limit: 100_000,
        }
    }
}

impl Limits {
    /// Límites por defecto.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cambia el tamaño máximo del XML.
    #[must_use]
    pub const fn with_max_xml_bytes(mut self, bytes: u64) -> Self {
        self.max_xml_bytes = bytes;
        self
    }

    /// Cambia el tamaño máximo de las hojas.
    #[must_use]
    pub const fn with_max_stylesheet_bytes(mut self, bytes: u64) -> Self {
        self.max_stylesheet_bytes = bytes;
        self
    }

    /// Cambia el límite de operaciones XPath (`0` lo desactiva).
    #[must_use]
    pub const fn with_xpath_operations(mut self, operations: u64) -> Self {
        self.xpath_operations = operations;
        self
    }
}
