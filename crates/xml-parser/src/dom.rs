//! Documento XML de solo lectura con perfil seguro.
//!
//! Se rechaza el XML mal formado, cualquier DTD con declaraciones (no hay
//! expansión de entidades ni acceso a recursos externos) y las entradas que
//! superan los [`Limits`]. El árbol se analiza con `roxmltree` y se copia a
//! una estructura propia que conserva los nombres calificados tal como
//! aparecen en el texto y los espacios de nombres en ámbito de cada elemento,
//! con los que se resuelven los prefijos de las consultas.

use std::collections::HashMap;
use std::fmt;

use cpeforge_xml_limits::{
    DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE, MarkupRejection, check_tree_markup,
    exceeds_depth,
};

use crate::encoding::decode_input;
use crate::error::XmlParserError;

/// Límites de lectura.
///
/// Los valores por defecto admiten cualquier comprobante SUNAT real y cortan
/// entradas hostiles antes de construir el árbol completo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// Tamaño máximo de la entrada en bytes. Por defecto 10 MiB.
    pub max_bytes: usize,
    /// Profundidad máxima de elementos (el raíz es 1). Por defecto 64: un
    /// comprobante UBL firmado no pasa de unas 15. `roxmltree` desciende de
    /// forma recursiva (en compilaciones debug, unos 16 KiB de pila por
    /// nivel), así que el tope protege la pila; subirlo exige hilos con pila
    /// suficiente.
    pub max_depth: usize,
    /// Número máximo de nodos (elementos, textos, comentarios…). Por defecto
    /// 1 000 000.
    pub max_nodes: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024,
            max_depth: 64,
            max_nodes: 1_000_000,
        }
    }
}

/// Índice en `XmlDocument::names`: los nombres y espacios de nombres se
/// guardan una sola vez por documento.
type Name = u32;

#[derive(Debug, Clone)]
struct Attr {
    qname: Name,
    local: Name,
    namespace: Option<Name>,
    value: Box<str>,
}

#[derive(Debug, Clone)]
enum Kind {
    Element {
        qname: Name,
        local: Name,
        namespace: Option<Name>,
        attrs: Vec<Attr>,
        /// Índice en `XmlDocument::scopes`.
        scope: usize,
        /// Último descendiente (inclusive) en orden de documento.
        last_descendant: usize,
    },
    Text(Box<str>),
}

#[derive(Debug, Clone)]
struct NodeData {
    kind: Kind,
    parent: Option<usize>,
    /// Hijos (elementos y textos) en `XmlDocument::children[start..end]`.
    children: (usize, usize),
}

/// Prefijos con nombre y URI en ámbito, internados en `XmlDocument::names`
/// (las consultas solo resuelven prefijos con nombre). Ocho bytes por
/// entrada: un documento que redeclara prefijos en miles de elementos no
/// multiplica cadenas.
type Scope = Vec<(Name, Name)>;

/// Documento XML analizado, que los parsers aceptan en lugar de bytes
/// (`parse_document`) para no analizar dos veces el mismo XML.
///
/// Siempre tiene elemento raíz: un XML sin raíz es mal formado y se rechaza.
#[derive(Clone)]
pub struct XmlDocument {
    /// Elementos y textos en orden de documento; el raíz es el 0.
    nodes: Vec<NodeData>,
    scopes: Vec<Scope>,
    /// Nombres calificados, locales y URI de espacios de nombres distintos.
    names: Vec<Box<str>>,
    /// Hijos de todos los nodos, contiguos por padre y en orden de documento.
    children: Vec<usize>,
}

impl fmt::Debug for XmlDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XmlDocument")
            .field("root", &self.root().name())
            .field("nodes", &self.nodes.len())
            .finish()
    }
}

impl XmlDocument {
    /// Analiza bytes con los [`Limits`] por defecto.
    ///
    /// # Errors
    ///
    /// [`XmlParserError`] de entrada: vacía, demasiado grande o profunda, con
    /// DTD, con codificación no admitida o mal formada.
    pub fn parse(bytes: &[u8]) -> Result<Self, XmlParserError> {
        Self::parse_with_limits(bytes, &Limits::default())
    }

    /// Analiza bytes con límites propios.
    ///
    /// # Errors
    ///
    /// Los mismos que [`Self::parse`].
    pub fn parse_with_limits(bytes: &[u8], limits: &Limits) -> Result<Self, XmlParserError> {
        if bytes.is_empty() {
            return Err(XmlParserError::Empty);
        }
        if bytes.len() > limits.max_bytes {
            return Err(XmlParserError::TooLarge {
                size: bytes.len(),
                limit: limits.max_bytes,
            });
        }
        let text = decode_input(bytes)?;
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
        // Antes de analizar: `roxmltree` desciende recursivamente por cada
        // elemento anidado.
        check_depth(text, limits.max_depth)?;
        check_tree(text)?;
        let options = roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: limits.max_nodes,
            ..roxmltree::ParsingOptions::default()
        };
        let tree =
            roxmltree::Document::parse_with_options(text, options).map_err(|err| match err {
                roxmltree::Error::DtdDetected => XmlParserError::Dtd,
                roxmltree::Error::NodesLimitReached => XmlParserError::TooManyNodes {
                    limit: limits.max_nodes,
                },
                other => XmlParserError::Malformed {
                    message: other.to_string(),
                },
            })?;
        Self::from_tree(&tree, text, limits.max_depth)
    }

    /// Copia el árbol de roxmltree a la estructura propia en una pasada.
    ///
    /// Cada nodo copiado solo asigna su texto o los valores de sus atributos:
    /// los nombres se internan en `names`, los hijos de todos los nodos
    /// comparten un único vector y el ámbito de espacios de nombres se
    /// comparte con el padre cuando no cambia.
    fn from_tree<'t>(
        tree: &'t roxmltree::Document<'t>,
        text: &'t str,
        max_depth: usize,
    ) -> Result<Self, XmlParserError> {
        let total = tree.descendants().count();
        let mut doc = Self {
            nodes: Vec::with_capacity(total),
            scopes: Vec::new(),
            names: Vec::new(),
            children: Vec::new(),
        };
        let mut interner: HashMap<&'t str, Name> = HashMap::new();
        // Índice propio y profundidad de cada nodo roxmltree ya copiado.
        let mut map: Vec<Option<(usize, usize)>> = vec![None; total];
        let root = tree.root_element();
        for node in root.descendants() {
            let parent = node
                .parent()
                .and_then(|p| map.get(slot_of(p)).copied().flatten());
            let depth = parent.map_or(1, |(_, d)| d + 1);
            let kind = if node.is_element() {
                if depth > max_depth {
                    return Err(XmlParserError::TooDeep { limit: max_depth });
                }
                let scope = doc.scope_for(node, parent.map(|(i, _)| i), &mut interner);
                let mut intern = |name: &'t str| doc_intern(&mut doc.names, &mut interner, name);
                let attrs = node
                    .attributes()
                    .map(|a| Attr {
                        qname: intern(text.get(a.range_qname()).unwrap_or_else(|| a.name())),
                        local: intern(a.name()),
                        namespace: a.namespace().map(&mut intern),
                        value: a.value().into(),
                    })
                    .collect();
                Kind::Element {
                    qname: intern(source_qname(text, node.range().start)),
                    local: intern(node.tag_name().name()),
                    namespace: node.tag_name().namespace().map(&mut intern),
                    attrs,
                    scope,
                    last_descendant: 0,
                }
            } else if let Some(t) = node.text().filter(|_| node.is_text()) {
                Kind::Text(t.into())
            } else {
                // Comentarios e instrucciones de proceso no forman parte de
                // `textContent` ni de las consultas que usan los parsers.
                continue;
            };
            let index = doc.nodes.len();
            doc.nodes.push(NodeData {
                kind,
                parent: parent.map(|(i, _)| i),
                children: (0, 0),
            });
            if let Some(slot) = map.get_mut(slot_of(node)) {
                *slot = Some((index, depth));
            }
        }
        doc.index_children();
        // Último descendiente de cada elemento (recorrido inverso: el último
        // hijo, de índice mayor, ya está resuelto).
        for index in (0..doc.nodes.len()).rev() {
            let last = doc
                .nodes
                .get(index)
                .and_then(|n| {
                    n.children
                        .1
                        .checked_sub(1)
                        .filter(|_| n.children.1 > n.children.0)
                })
                .and_then(|at| doc.children.get(at).copied())
                .map_or(index, |c| doc.last_descendant_of(c));
            if let Some(NodeData {
                kind: Kind::Element {
                    last_descendant, ..
                },
                ..
            }) = doc.nodes.get_mut(index)
            {
                *last_descendant = last;
            }
        }
        Ok(doc)
    }

    /// Rellena `children` y el rango de cada nodo: cuenta los hijos por
    /// padre, reparte posiciones y coloca cada nodo en orden de documento.
    fn index_children(&mut self) {
        let mut ends = vec![0usize; self.nodes.len()];
        for node in &self.nodes {
            if let Some(count) = node.parent.and_then(|p| ends.get_mut(p)) {
                *count += 1;
            }
        }
        let mut start = 0;
        for (node, end) in self.nodes.iter_mut().zip(ends.iter_mut()) {
            let count = *end;
            node.children = (start, start);
            start += count;
            *end = start;
        }
        self.children = vec![0; start];
        for index in 0..self.nodes.len() {
            let Some(parent) = self.nodes.get(index).and_then(|n| n.parent) else {
                continue;
            };
            if let Some(range) = self.nodes.get_mut(parent).map(|p| &mut p.children) {
                if let Some(slot) = self.children.get_mut(range.1) {
                    *slot = index;
                }
                range.1 += 1;
            }
        }
    }

    fn last_descendant_of(&self, index: usize) -> usize {
        match self.nodes.get(index).map(|n| &n.kind) {
            Some(Kind::Element {
                last_descendant, ..
            }) => *last_descendant,
            _ => index,
        }
    }

    /// Ámbito de espacios de nombres de `node`: el del padre o el último
    /// creado si coinciden (comparados sin copiarlos; así los hermanos que
    /// redeclaran los mismos prefijos comparten ámbito) o uno nuevo.
    fn scope_for<'t>(
        &mut self,
        node: roxmltree::Node<'t, 't>,
        parent: Option<usize>,
        interner: &mut HashMap<&'t str, Name>,
    ) -> usize {
        let named = || {
            node.namespaces()
                .filter_map(|ns| Some((ns.name()?, ns.uri())))
        };
        let parent_scope = parent.and_then(|p| match &self.nodes.get(p)?.kind {
            Kind::Element { scope, .. } => Some(*scope),
            Kind::Text(_) => None,
        });
        let last = self.scopes.len().checked_sub(1);
        for id in [parent_scope, last].into_iter().flatten() {
            let same = self.scopes.get(id).is_some_and(|scope| {
                scope.len() == named().count()
                    && scope.iter().zip(named()).all(|(&(prefix, uri), (p, u))| {
                        self.name(prefix) == p && self.name(uri) == u
                    })
            });
            if same {
                return id;
            }
        }
        let scope = named()
            .map(|(prefix, uri)| {
                (
                    doc_intern(&mut self.names, interner, prefix),
                    doc_intern(&mut self.names, interner, uri),
                )
            })
            .collect();
        self.scopes.push(scope);
        self.scopes.len() - 1
    }

    fn name(&self, id: Name) -> &str {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.names.get(i))
            .map_or("", |name| name)
    }

    /// Elemento raíz (`documentElement`).
    #[must_use]
    pub fn root(&self) -> Node<'_> {
        Node {
            doc: self,
            index: 0,
            attr: None,
        }
    }

    pub(crate) fn node(&self, index: usize) -> Node<'_> {
        Node {
            doc: self,
            index,
            attr: None,
        }
    }
}

/// Rechaza un anidamiento mayor que `max_depth` con un recorrido léxico, sin
/// recursión ([`exceeds_depth`]). Cuenta como `roxmltree` en todo XML que este
/// acepta; en el mal formado, hasta el primer error.
fn check_depth(text: &str, max_depth: usize) -> Result<(), XmlParserError> {
    if exceeds_depth(text, max_depth) {
        Err(XmlParserError::TooDeep { limit: max_depth })
    } else {
        Ok(())
    }
}

/// Rechaza, antes de analizar, los elementos con más de
/// [`DEFAULT_MAX_ATTRIBUTES`] atributos (`roxmltree` busca duplicados en
/// tiempo cuadrático) y los ámbitos de espacios de nombres que superan
/// [`DEFAULT_MAX_NAMESPACE_SCOPE`] (se copian por cada elemento que declara).
fn check_tree(text: &str) -> Result<(), XmlParserError> {
    match check_tree_markup(text, DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE) {
        Ok(()) => Ok(()),
        Err(MarkupRejection::TooManyAttributes) => Err(XmlParserError::TooManyAttributes {
            limit: DEFAULT_MAX_ATTRIBUTES,
        }),
        Err(_) => Err(XmlParserError::TooManyNamespaces {
            limit: DEFAULT_MAX_NAMESPACE_SCOPE,
        }),
    }
}

/// Identificador interno de `name`, que se añade a la tabla la primera vez.
fn doc_intern<'t>(
    names: &mut Vec<Box<str>>,
    interner: &mut HashMap<&'t str, Name>,
    name: &'t str,
) -> Name {
    *interner.entry(name).or_insert_with(|| {
        names.push(name.into());
        Name::try_from(names.len() - 1).unwrap_or(Name::MAX)
    })
}

/// Posición de un nodo roxmltree (su identificador es un índice).
fn slot_of(node: roxmltree::Node<'_, '_>) -> usize {
    usize::try_from(node.id().get()).unwrap_or(usize::MAX)
}

/// Nombre calificado tal como aparece tras `<` en el texto fuente.
fn source_qname(text: &str, start: usize) -> &str {
    let rest = text.get(start + 1..).unwrap_or_default();
    let end = rest
        .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
        .unwrap_or(rest.len());
    rest.get(..end).unwrap_or_default()
}

/// Nodo de un [`XmlDocument`]: elemento, texto o atributo.
///
/// Se copia sin coste (`Copy`).
#[derive(Clone, Copy)]
pub struct Node<'a> {
    doc: &'a XmlDocument,
    index: usize,
    /// Índice del atributo dentro del elemento `index`.
    attr: Option<usize>,
}

impl PartialEq for Node<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.doc, other.doc) && self.index == other.index && self.attr == other.attr
    }
}

impl Eq for Node<'_> {}

impl fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("name", &self.name())
            .field("index", &self.index)
            .field("attr", &self.attr)
            .finish()
    }
}

impl<'a> Node<'a> {
    fn data(&self) -> &'a NodeData {
        &self.doc.nodes[self.index]
    }

    fn attr_data(&self) -> Option<&'a Attr> {
        match (&self.data().kind, self.attr) {
            (Kind::Element { attrs, .. }, Some(i)) => attrs.get(i),
            _ => None,
        }
    }

    pub(crate) fn document(&self) -> &'a XmlDocument {
        self.doc
    }

    pub(crate) fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn attr_index(&self) -> Option<usize> {
        self.attr
    }

    /// `true` si es un elemento.
    #[must_use]
    pub fn is_element(&self) -> bool {
        self.attr.is_none() && matches!(self.data().kind, Kind::Element { .. })
    }

    /// `true` si es un atributo.
    #[must_use]
    pub fn is_attribute(&self) -> bool {
        self.attr.is_some()
    }

    /// Nombre calificado (`nodeName`): `cbc:ID`, `CreditNote`, `#text`.
    #[must_use]
    pub fn name(&self) -> &'a str {
        if let Some(attr) = self.attr_data() {
            return self.doc.name(attr.qname);
        }
        match &self.data().kind {
            Kind::Element { qname, .. } => self.doc.name(*qname),
            Kind::Text(_) => "#text",
        }
    }

    /// Nombre local (`localName`).
    #[must_use]
    pub fn local_name(&self) -> &'a str {
        if let Some(attr) = self.attr_data() {
            return self.doc.name(attr.local);
        }
        match &self.data().kind {
            Kind::Element { local, .. } => self.doc.name(*local),
            Kind::Text(_) => "",
        }
    }

    /// URI del espacio de nombres (`namespaceURI`).
    #[must_use]
    pub fn namespace(&self) -> Option<&'a str> {
        if let Some(attr) = self.attr_data() {
            return attr.namespace.map(|id| self.doc.name(id));
        }
        match &self.data().kind {
            Kind::Element { namespace, .. } => namespace.map(|id| self.doc.name(id)),
            Kind::Text(_) => None,
        }
    }

    /// Valor del nodo (`nodeValue`): para un elemento, el texto de todos sus
    /// descendientes (`textContent`); para un atributo, su valor.
    #[must_use]
    pub fn value(&self) -> String {
        if let Some(attr) = self.attr_data() {
            return attr.value.to_string();
        }
        let mut out = String::new();
        let last = self.doc.last_descendant_of(self.index);
        for data in self.doc.nodes.get(self.index..=last).unwrap_or_default() {
            if let Kind::Text(t) = &data.kind {
                out.push_str(t);
            }
        }
        out
    }

    /// Valor del atributo por nombre calificado, o cadena vacía si no existe.
    #[must_use]
    pub fn attribute(&self, name: &str) -> String {
        match (&self.data().kind, self.attr) {
            (Kind::Element { attrs, .. }, None) => attrs
                .iter()
                .find(|a| self.doc.name(a.qname) == name)
                .map(|a| a.value.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Elemento padre (para un atributo, el elemento que lo contiene).
    #[must_use]
    pub fn parent(&self) -> Option<Node<'a>> {
        if self.attr.is_some() {
            return Some(self.doc.node(self.index));
        }
        self.data().parent.map(|index| self.doc.node(index))
    }

    /// Hijos elemento y texto en orden de documento.
    pub fn children(&self) -> impl Iterator<Item = Node<'a>> + use<'a> {
        let doc = self.doc;
        let children: &'a [usize] = if self.attr.is_none() {
            let (start, end) = self.data().children;
            doc.children.get(start..end).unwrap_or_default()
        } else {
            &[]
        };
        children.iter().map(move |&i| doc.node(i))
    }

    /// Hijos elemento.
    pub(crate) fn child_elements(&self) -> impl Iterator<Item = Node<'a>> + use<'a> {
        self.children().filter(Node::is_element)
    }

    /// Descendientes elemento (sin incluir el propio nodo), en orden.
    pub(crate) fn descendant_elements(&self) -> impl Iterator<Item = Node<'a>> + use<'a> {
        let doc = self.doc;
        let range = if self.attr.is_some() {
            1..1
        } else {
            self.index + 1..doc.last_descendant_of(self.index) + 1
        };
        range.map(move |i| doc.node(i)).filter(Node::is_element)
    }

    /// Atributos de un elemento.
    pub(crate) fn attributes(&self) -> impl Iterator<Item = Node<'a>> + use<'a> {
        let doc = self.doc;
        let index = self.index;
        let count = match (&self.data().kind, self.attr) {
            (Kind::Element { attrs, .. }, None) => attrs.len(),
            _ => 0,
        };
        (0..count).map(move |i| Node {
            doc,
            index,
            attr: Some(i),
        })
    }

    /// URI del prefijo en ámbito del nodo, como `xmlGetNsList()` (para un
    /// atributo o texto, el de su elemento).
    pub(crate) fn lookup_namespace(&self, prefix: &str) -> Option<&'a str> {
        let element = if self.is_element() {
            Some(*self)
        } else {
            self.parent().filter(Node::is_element)
        }?;
        let Kind::Element { scope, .. } = &element.data().kind else {
            return None;
        };
        self.doc
            .scopes
            .get(*scope)?
            .iter()
            .find(|&&(p, _)| self.doc.name(p) == prefix)
            .map(|&(_, uri)| self.doc.name(uri))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(xml: &str) -> XmlDocument {
        XmlDocument::parse(xml.as_bytes()).unwrap_or_else(|e| unreachable!("{e}"))
    }

    #[test]
    fn sibling_scopes_are_shared_and_resolved_per_element() {
        let d = doc(
            "<r xmlns:a='urn:a' xmlns='urn:d'><x xmlns:b='urn:b'><y/></x><x xmlns:b='urn:b'/><z xmlns:b='urn:b2'/><w/></r>",
        );
        let root = d.root();
        let elements: Vec<_> = root.child_elements().collect();
        let scope_of = |n: Node<'_>| match &n.data().kind {
            Kind::Element { scope, .. } => *scope,
            Kind::Text(_) => usize::MAX,
        };
        // Los dos `x` comparten ámbito; `z` redeclara `b` con otra URI.
        assert_eq!(scope_of(elements[0]), scope_of(elements[1]));
        assert_ne!(scope_of(elements[1]), scope_of(elements[2]));
        assert_eq!(scope_of(elements[3]), scope_of(root));
        let y = elements[0].child_elements().next().unwrap_or(root);
        assert_eq!(y.lookup_namespace("b"), Some("urn:b"));
        assert_eq!(y.lookup_namespace("a"), Some("urn:a"));
        assert_eq!(elements[2].lookup_namespace("b"), Some("urn:b2"));
        assert_eq!(elements[3].lookup_namespace("b"), None);
        // El espacio por defecto no se resuelve por prefijo.
        assert_eq!(root.lookup_namespace(""), None);
        assert_eq!(d.scopes.len(), 3);
    }

    #[test]
    fn names_values_and_scope() {
        let d = doc(
            "<a:r xmlns:a='urn:a' x='1'><!--c--><b xmlns:c='urn:c'>t<![CDATA[<u>]]><c:d a:y='2'>v</c:d></b>w</a:r>",
        );
        let root = d.root();
        assert_eq!(root.name(), "a:r");
        assert_eq!(root.local_name(), "r");
        assert_eq!(root.namespace(), Some("urn:a"));
        assert_eq!(root.value(), "t<u>vw");
        assert_eq!(root.attribute("x"), "1");
        assert_eq!(root.attribute("z"), "");
        let b = root.child_elements().next().unwrap_or(root);
        assert_eq!(b.name(), "b");
        assert_eq!(b.lookup_namespace("c"), Some("urn:c"));
        assert_eq!(root.lookup_namespace("c"), None);
        assert_eq!(root.lookup_namespace("a"), Some("urn:a"));
        let d2 = b.descendant_elements().next().unwrap_or(root);
        assert_eq!(d2.name(), "c:d");
        let attr = d2.attributes().next().unwrap_or(root);
        assert_eq!(
            (attr.name(), attr.local_name(), attr.value().as_str()),
            ("a:y", "y", "2")
        );
        assert_eq!(attr.namespace(), Some("urn:a"));
        assert!(attr.is_attribute() && !attr.is_element());
        assert_eq!(attr.parent(), Some(d2));
    }

    #[test]
    fn depth_prescan() {
        assert!(
            check_depth(
                "<a><!-- <b><c> --><b x='>' y=\"<\"/><![CDATA[<d><e>]]><?p <f>?></a>",
                2
            )
            .is_ok()
        );
        assert!(check_depth("<a><b/></a>", 2).is_ok());
        // El elemento vacío también ocupa un nivel, como en `from_tree`.
        assert!(check_depth("<a><b><c/></b></a>", 2).is_err());
        assert!(check_depth("<a><b><c></c></b></a>", 2).is_err());
        assert!(check_depth("<!DOCTYPE a [<!ELEMENT a ANY>]><a><b/></a>", 2).is_ok());
        let deep = format!("{}{}", "<a>".repeat(100_000), "</a>".repeat(100_000));
        assert_eq!(
            XmlDocument::parse(deep.as_bytes()).err(),
            Some(XmlParserError::TooDeep { limit: 64 })
        );
        let ok = format!("{}{}", "<a>".repeat(64), "</a>".repeat(64));
        assert!(XmlDocument::parse(ok.as_bytes()).is_ok());
    }

    #[test]
    fn rejects_hostile_input() {
        let parse = |xml: &[u8], limits: &Limits| XmlDocument::parse_with_limits(xml, limits);
        let base = Limits::default();
        assert_eq!(parse(b"", &base).err(), Some(XmlParserError::Empty));
        assert!(matches!(
            parse(b"<r>", &base),
            Err(XmlParserError::Malformed { .. })
        ));
        assert_eq!(
            parse(b"<!DOCTYPE r [<!ENTITY e 'x'>]><r>&e;</r>", &base).err(),
            Some(XmlParserError::Dtd)
        );
        let small = Limits {
            max_bytes: 4,
            ..base
        };
        assert!(matches!(
            parse(b"<r></r>", &small),
            Err(XmlParserError::TooLarge { size: 7, limit: 4 })
        ));
        let shallow = Limits {
            max_depth: 2,
            ..base
        };
        assert!(parse(b"<a><b/></a>", &shallow).is_ok());
        assert_eq!(
            parse(b"<a><b><c/></b></a>", &shallow).err(),
            Some(XmlParserError::TooDeep { limit: 2 })
        );
        let few = Limits {
            max_nodes: 3,
            ..base
        };
        assert_eq!(
            parse(b"<a><b/><c/><d/></a>", &few).err(),
            Some(XmlParserError::TooManyNodes { limit: 3 })
        );
    }
}
