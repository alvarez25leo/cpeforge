//! Lector XPath de alto nivel y las consultas que usan los parsers.

use crate::dom::{Limits, Node, XmlDocument};
use crate::error::XmlParserError;
use crate::xpath::{Eval, Path};

/// Lector de un documento con consultas XPath.
///
/// Las consultas admiten el subconjunto de XPath de los parsers (pasos por
/// nombre, `//`, `*` y un atributo final; ver [`XmlReader::nodes`]). Los
/// prefijos se resuelven con los espacios de nombres en ámbito del nodo de
/// contexto, y sin contexto se consulta desde el elemento raíz.
///
/// ```
/// use cpeforge_xml_parser::XmlReader;
///
/// let xml = r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
///   xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
///   <cbc:ID>F001-1</cbc:ID></Invoice>"#;
/// let reader = XmlReader::load_xml(xml.as_bytes())?;
/// assert_eq!(reader.value("cbc:ID", None)?, "F001-1");
/// assert_eq!(reader.value("cbc:Nada", None)?, "");
/// assert_eq!(reader.value_or("cbc:Nada", None, None)?, None);
/// # Ok::<(), cpeforge_xml_parser::XmlParserError>(())
/// ```
#[derive(Debug, Clone)]
pub struct XmlReader {
    document: XmlDocument,
}

impl XmlReader {
    /// Analiza el XML con [`Limits::default`].
    ///
    /// # Errors
    ///
    /// Los de [`XmlDocument::parse`]: un XML inválido se rechaza, no se
    /// sustituye por un documento vacío.
    pub fn load_xml(bytes: &[u8]) -> Result<Self, XmlParserError> {
        Ok(Self::from_document(XmlDocument::parse(bytes)?))
    }

    /// Analiza el XML con límites propios.
    ///
    /// # Errors
    ///
    /// Los de [`XmlDocument::parse_with_limits`].
    pub fn load_xml_with_limits(bytes: &[u8], limits: &Limits) -> Result<Self, XmlParserError> {
        Ok(Self::from_document(XmlDocument::parse_with_limits(
            bytes, limits,
        )?))
    }

    /// Lector sobre un documento ya analizado.
    #[must_use]
    pub fn from_document(document: XmlDocument) -> Self {
        Self { document }
    }

    /// Documento cargado.
    #[must_use]
    pub fn document(&self) -> &XmlDocument {
        &self.document
    }

    /// Valor del primer nodo de la consulta o `""`.
    ///
    /// # Errors
    ///
    /// [`XmlParserError::Query`] si la consulta está fuera del subconjunto o
    /// el contexto pertenece a otro documento.
    pub fn value(&self, query: &str, context: Option<Node<'_>>) -> Result<String, XmlParserError> {
        Ok(self.value_or(query, context, Some(""))?.unwrap_or_default())
    }

    /// Valor del primer nodo de la consulta o `default`. Un prefijo no
    /// definido también devuelve `default`.
    ///
    /// # Errors
    ///
    /// Los de [`Self::value`].
    pub fn value_or(
        &self,
        query: &str,
        context: Option<Node<'_>>,
        default: Option<&str>,
    ) -> Result<Option<String>, XmlParserError> {
        Ok(match self.nodes(query, context)? {
            Some(nodes) if !nodes.is_empty() => nodes.first().map(Node::value),
            _ => default.map(str::to_owned),
        })
    }

    /// Primer nodo de la consulta si es un elemento.
    ///
    /// # Errors
    ///
    /// Los de [`Self::value`].
    pub fn node<'a>(
        &'a self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<Option<Node<'a>>, XmlParserError> {
        Ok(self
            .nodes(query, context)?
            .and_then(|nodes| nodes.first().copied())
            .filter(Node::is_element))
    }

    /// Nodos de la consulta en orden de documento. `None` si un prefijo de la
    /// consulta no está definido en el contexto (no es lo mismo que una lista
    /// vacía).
    ///
    /// Sintaxis admitida: `/a:b/c:d` (desde el documento), `a:b/c:d` (desde
    /// el contexto), `//a:b` y `a//b` (descendientes), `*` y `@atributo` como
    /// último paso. Funciones, predicados, ejes explícitos y uniones se
    /// rechazan con [`XmlParserError::Query`] (XPath 1.0 completo no está
    /// implementado).
    ///
    /// # Errors
    ///
    /// Los de [`Self::value`].
    pub fn nodes<'a>(
        &'a self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<Option<Vec<Node<'a>>>, XmlParserError> {
        evaluate(&self.document, query, context, &[])
    }
}

/// Compila y evalúa una consulta; `Ok(None)` si un prefijo no está definido.
pub(crate) fn evaluate<'a>(
    document: &'a XmlDocument,
    query: &str,
    context: Option<Node<'a>>,
    registered: &[(&str, &str)],
) -> Result<Option<Vec<Node<'a>>>, XmlParserError> {
    let error = |message: String| XmlParserError::Query {
        query: query.to_owned(),
        message,
    };
    let path = Path::compile(query).map_err(error)?;
    let context = match context {
        Some(node) if !std::ptr::eq(node.document(), document) => {
            return Err(error(
                "el nodo de contexto pertenece a otro documento".into(),
            ));
        }
        Some(node) => node,
        None => document.root(),
    };
    Ok(match path.evaluate(document, context, registered) {
        Eval::Nodes(nodes) => Some(nodes),
        Eval::Undefined(_) => None,
    })
}

/// Consultas de los parsers sobre un documento: valor con cadena vacía por
/// defecto, nodo opcional y lista de nodos.
pub(crate) struct Lookup<'a> {
    pub(crate) document: &'a XmlDocument,
}

impl<'a> Lookup<'a> {
    pub(crate) fn root(&self) -> Node<'a> {
        self.document.root()
    }

    fn query(
        &self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<Option<Vec<Node<'a>>>, XmlParserError> {
        evaluate(self.document, query, context, &[])
    }

    /// Valor del primer nodo o cadena vacía.
    pub(crate) fn value(
        &self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<String, XmlParserError> {
        self.value_def(query, context, "")
    }

    /// Valor del primer nodo o `default`.
    pub(crate) fn value_def(
        &self,
        query: &str,
        context: Option<Node<'a>>,
        default: &str,
    ) -> Result<String, XmlParserError> {
        Ok(match self.query(query, context)? {
            Some(nodes) if !nodes.is_empty() => nodes.first().map(Node::value).unwrap_or_default(),
            _ => default.to_owned(),
        })
    }

    /// Primer nodo si es un elemento.
    pub(crate) fn node(
        &self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<Option<Node<'a>>, XmlParserError> {
        Ok(self
            .query(query, context)?
            .and_then(|nodes| nodes.first().copied())
            .filter(Node::is_element))
    }

    /// Nodos de la consulta; si un prefijo no está definido, ninguno.
    pub(crate) fn nodes(
        &self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<Vec<Node<'a>>, XmlParserError> {
        Ok(self.query(query, context)?.unwrap_or_default())
    }
}

/// Valor de un nodo opcional: `None` si falta.
pub(crate) fn node_value(node: Option<Node<'_>>) -> Option<String> {
    node.map(|n| n.value())
}

/// Atributo de un nodo obligatorio: `Error` de ejecución si falta el nodo.
pub(crate) fn node_attribute(node: Option<Node<'_>>, name: &str) -> Result<String, XmlParserError> {
    node.map(|n| n.attribute(name))
        .ok_or_else(|| XmlParserError::call_on_null("getAttribute"))
}
