//! Errores del lector y de los parsers.

use std::fmt;

/// Error al leer un XML o al convertirlo en documento.
///
/// Las variantes de entrada ([`Self::Empty`] a [`Self::Malformed`]) rechazan
/// el XML inválido en lugar de continuar con un documento vacío. Las variantes
/// [`Self::Runtime`], [`Self::Date`] y [`Self::Number`] corresponden a los fallos
/// de la implementación de referencia durante la lectura; [`Self::error_class`]
/// y [`Self::error_message`] devuelven la clase y el mensaje de esa excepción,
/// que forman parte del contrato de compatibilidad (véase «Origen,
/// interoperabilidad y compatibilidad» en el README del repositorio).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlParserError {
    /// Entrada vacía (clase de referencia `ValueError`).
    Empty,
    /// La entrada supera [`Limits::max_bytes`](crate::Limits::max_bytes).
    TooLarge {
        /// Tamaño de la entrada en bytes.
        size: usize,
        /// Límite configurado.
        limit: usize,
    },
    /// El anidamiento de elementos supera [`Limits::max_depth`](crate::Limits::max_depth).
    TooDeep {
        /// Límite configurado.
        limit: usize,
    },
    /// El número de nodos supera [`Limits::max_nodes`](crate::Limits::max_nodes).
    TooManyNodes {
        /// Límite configurado.
        limit: u32,
    },
    /// Un elemento supera el máximo de atributos (incluidas las declaraciones
    /// `xmlns`).
    TooManyAttributes {
        /// Límite aplicado.
        limit: usize,
    },
    /// Las declaraciones de espacios de nombres en ámbito superan el coste
    /// máximo (ver `cpeforge_xml_limits::check_tree_markup`).
    TooManyNamespaces {
        /// Límite aplicado.
        limit: usize,
    },
    /// DTD con declaraciones (entidades internas o externas): se rechaza.
    Dtd,
    /// Codificación declarada no admitida o bytes inválidos para ella.
    Encoding {
        /// Nombre declarado (`UTF-8` si no hay declaración).
        name: String,
    },
    /// XML mal formado.
    Malformed {
        /// Descripción del error del analizador.
        message: String,
    },
    /// Error de ejecución de la implementación de referencia en el mismo punto
    /// de la lectura, por ejemplo `Call to a member function getAttribute() on
    /// null` cuando falta un nodo obligatorio.
    Runtime {
        /// Clase de la excepción (`Error`, `TypeError`).
        class: &'static str,
        /// Mensaje sin ruta de archivo ni línea.
        message: String,
    },
    /// Fecha que `new DateTime()` rechaza o que este crate no interpreta.
    Date {
        /// Texto de la fecha.
        text: String,
        /// `true` si es una fecha válida en un formato fuera del subconjunto
        /// admitido (relativo, nombres de mes…), que se rechaza de forma
        /// explícita.
        unsupported: bool,
    },
    /// Importe o cantidad cuyo valor no cabe en `Decimal`/`i64`.
    Number {
        /// Texto numérico.
        text: String,
    },
    /// Consulta de [`XmlReader`](crate::XmlReader) fuera del subconjunto de
    /// XPath admitido.
    Query {
        /// Consulta recibida.
        query: String,
        /// Motivo.
        message: String,
    },
}

impl XmlParserError {
    /// Clase de la excepción equivalente de la implementación de referencia,
    /// si esta falla en el mismo caso.
    #[must_use]
    pub fn error_class(&self) -> Option<&str> {
        match self {
            Self::Empty => Some("ValueError"),
            Self::Runtime { class, .. } => Some(class),
            Self::Date {
                unsupported: false, ..
            } => Some("DateMalformedStringException"),
            _ => None,
        }
    }

    /// Mensaje de esa excepción cuando es determinista.
    ///
    /// Para fechas inválidas solo se reproduce el prefijo
    /// `Failed to parse time string (…)`, sin la posición del error.
    #[must_use]
    pub fn error_message(&self) -> Option<String> {
        match self {
            Self::Empty => {
                Some("DOMDocument::loadXML(): Argument #1 ($source) must not be empty".into())
            }
            Self::Runtime { message, .. } => Some(message.clone()),
            Self::Date {
                text,
                unsupported: false,
            } => Some(format!("Failed to parse time string ({text})")),
            _ => None,
        }
    }

    /// Error de ejecución con clase y mensaje de referencia.
    pub(crate) fn runtime(class: &'static str, message: impl Into<String>) -> Self {
        Self::Runtime {
            class,
            message: message.into(),
        }
    }

    /// `Call to a member function {method}() on null`.
    pub(crate) fn call_on_null(method: &str) -> Self {
        Self::runtime(
            "Error",
            format!("Call to a member function {method}() on null"),
        )
    }
}

impl fmt::Display for XmlParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("XML vacío"),
            Self::TooLarge { size, limit } => {
                write!(f, "XML de {size} bytes supera el límite de {limit}")
            }
            Self::TooDeep { limit } => write!(f, "XML con más de {limit} niveles de anidamiento"),
            Self::TooManyNodes { limit } => write!(f, "XML con más de {limit} nodos"),
            Self::TooManyAttributes { limit } => {
                write!(f, "XML con un elemento de más de {limit} atributos")
            }
            Self::TooManyNamespaces { limit } => {
                write!(
                    f,
                    "XML con espacios de nombres en ámbito de coste mayor que {limit}"
                )
            }
            Self::Dtd => f.write_str("DTD con declaraciones no admitida"),
            Self::Encoding { name } => {
                write!(f, "codificación no admitida o bytes inválidos: {name}")
            }
            Self::Malformed { message } => write!(f, "XML mal formado: {message}"),
            Self::Runtime { class, message } => write!(f, "{class}: {message}"),
            Self::Date {
                text,
                unsupported: true,
            } => write!(f, "formato de fecha no admitido: {text:?}"),
            Self::Date { text, .. } => write!(f, "fecha inválida: {text:?}"),
            Self::Number { text } => write!(f, "número fuera de rango: {text:?}"),
            Self::Query { query, message } => {
                write!(f, "consulta {query:?} no admitida: {message}")
            }
        }
    }
}

impl std::error::Error for XmlParserError {}
