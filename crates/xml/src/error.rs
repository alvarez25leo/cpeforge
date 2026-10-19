use std::fmt;

use cpeforge_core::CoreError;

/// Error al generar el XML de un documento.
///
/// Cada variante corresponde a un fallo observable de la referencia (plantilla
/// inexistente o error durante el render).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlError {
    /// No hay plantilla para la versión o el tipo pedido. El nombre se
    /// construye como en la referencia (`invoice` + `ublVersion` + `.xml.twig`).
    TemplateNotFound {
        /// Nombre de plantilla que se habría buscado.
        template: String,
    },
    /// Con `strict_variables`, se leyó un atributo de un objeto nulo
    /// (`Impossible to access an attribute ("…") on a null variable`).
    NullAttribute {
        /// Plantilla referencia equivalente.
        template: &'static str,
        /// Atributo leído.
        attribute: &'static str,
    },
    /// Un método del modelo falló durante el render (por ejemplo
    /// `getXmlId()` sin fecha o `getNro()` nulo); Twig lo envuelve en
    /// `RuntimeError`.
    Document {
        /// Plantilla referencia equivalente.
        template: &'static str,
        /// Error del modelo.
        source: CoreError,
    },
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemplateNotFound { template } => {
                write!(f, "Unable to find template \"{template}\"")
            }
            Self::NullAttribute {
                template,
                attribute,
            } => write!(
                f,
                "Impossible to access an attribute (\"{attribute}\") on a null variable in \"{template}\""
            ),
            Self::Document { template, source } => {
                write!(
                    f,
                    "An exception has been thrown during the rendering of a template (\"{source}\") in \"{template}\""
                )
            }
        }
    }
}

impl std::error::Error for XmlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document { source, .. } => Some(source),
            _ => None,
        }
    }
}
