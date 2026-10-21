//! Canonicalización XML 1.0 inclusiva y exclusiva (W3C), con y sin comentarios.
//!
//! Implementación propia sobre [`Document`] siguiendo
//! [Canonical XML 1.0](https://www.w3.org/TR/2001/REC-xml-c14n-20010315) y
//! [Exclusive XML Canonicalization 1.0](https://www.w3.org/TR/xml-exc-c14n/),
//! con el mismo algoritmo que libxml2 (`xmlC14NExecute`):
//!
//! - un documento sin conjunto de nodos se canonicaliza completo;
//! - un elemento usa el conjunto `(.//. | .//@* | .//namespace::*)`;
//! - la transformación XPath usa `(.//. | .//@* | .//namespace::*)[expr]`.
//!
//! Con un conjunto de nodos, libxml2 imprime las declaraciones de namespace
//! y los atributos visibles aunque su elemento no lo sea; se reproduce igual.

use std::collections::{HashMap, HashSet};

use crate::dom::{
    Attribute, Document, Element, NodeId, NodeKind, NsDecl, XML_NS, retain_last_per_prefix,
};
use crate::error::Error;
use crate::xpath_eval::{Evaluator, XNode};

/// URI de C14N 1.0 inclusivo.
pub const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
/// URI de C14N 1.0 inclusivo con comentarios.
pub const C14N_COMMENTS: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315#WithComments";
/// URI de C14N exclusivo.
pub const EXC_C14N: &str = "http://www.w3.org/2001/10/xml-exc-c14n#";
/// URI de C14N exclusivo con comentarios.
pub const EXC_C14N_COMMENTS: &str = "http://www.w3.org/2001/10/xml-exc-c14n#WithComments";

/// Variante de canonicalización.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Method {
    /// Exclusiva (`true`) o inclusiva.
    pub exclusive: bool,
    /// Conserva comentarios.
    pub with_comments: bool,
    /// Prefijos tratados de forma inclusiva en el modo exclusivo
    /// (`InclusiveNamespaces PrefixList`; `#default` para el namespace por defecto).
    pub inclusive_prefixes: Vec<String>,
}

impl Method {
    /// Método inclusivo o exclusivo, con o sin comentarios y sin `PrefixList`.
    #[must_use]
    pub fn new(exclusive: bool, with_comments: bool) -> Self {
        Self {
            exclusive,
            with_comments,
            inclusive_prefixes: Vec::new(),
        }
    }

    /// Fija los prefijos tratados de forma inclusiva en el modo exclusivo.
    #[must_use]
    pub fn with_inclusive_prefixes(mut self, prefixes: Vec<String>) -> Self {
        self.inclusive_prefixes = prefixes;
        self
    }

    /// Método a partir de su URI; `None` si no es uno de los cuatro de C14N 1.0.
    #[must_use]
    pub fn from_uri(uri: &str) -> Option<Self> {
        let (exclusive, with_comments) = match uri {
            C14N => (false, false),
            C14N_COMMENTS => (false, true),
            EXC_C14N => (true, false),
            EXC_C14N_COMMENTS => (true, true),
            _ => return None,
        };
        Some(Self {
            exclusive,
            with_comments,
            inclusive_prefixes: Vec::new(),
        })
    }
}

/// Canonicaliza `node`: el documento completo o el subárbol de un elemento
/// (con sus atributos y nodos de namespace, heredados incluidos).
#[must_use]
pub fn canonicalize(doc: &Document, node: NodeId, method: &Method) -> String {
    Engine::new(doc, method, node_visibility(doc, node), None).run()
}

/// Como [`canonicalize`], pero entrega la salida a `sink` en bloques de
/// [`SINK_CHUNK`] bytes aproximadamente, sin construir la cadena completa
/// (p. ej. para calcular el digest de un documento grande).
pub(crate) fn canonicalize_to(
    doc: &Document,
    node: NodeId,
    method: &Method,
    sink: &mut dyn FnMut(&str),
) {
    Engine::new(doc, method, node_visibility(doc, node), Some(sink)).run();
}

fn node_visibility(doc: &Document, node: NodeId) -> Visibility {
    if node == doc.document_node() {
        Visibility::All
    } else {
        Visibility::Subtree(node)
    }
}

/// Canonicaliza el conjunto `(.//. | .//@* | .//namespace::*)[expr]`
/// evaluado desde `node`, como la transformación XPath de XMLDSig.
/// `namespaces` son los prefijos registrados (los del elemento `ds:XPath`).
///
/// # Errors
///
/// [`Error::XPath`] si la expresión es inválida, usa un prefijo o una función
/// no registrados, o supera el límite de coste de evaluación.
pub fn canonicalize_xpath(
    doc: &Document,
    node: NodeId,
    method: &Method,
    expr: &str,
    namespaces: &[(String, String)],
) -> Result<String, Error> {
    let visibility = xpath_visibility(doc, node, expr, namespaces)?;
    Ok(Engine::new(doc, method, visibility, None).run())
}

/// Como [`canonicalize_xpath`], entregando la salida a `sink` por bloques.
pub(crate) fn canonicalize_xpath_to(
    doc: &Document,
    node: NodeId,
    method: &Method,
    expr: &str,
    namespaces: &[(String, String)],
    sink: &mut dyn FnMut(&str),
) -> Result<(), Error> {
    let visibility = xpath_visibility(doc, node, expr, namespaces)?;
    Engine::new(doc, method, visibility, Some(sink)).run();
    Ok(())
}

fn xpath_visibility(
    doc: &Document,
    node: NodeId,
    expr: &str,
    namespaces: &[(String, String)],
) -> Result<Visibility, Error> {
    let registered: HashMap<String, String> = namespaces.iter().cloned().collect();
    let query = format!("(.//. | .//@* | .//namespace::*)[{expr}]");
    let mut evaluator = Evaluator::new(doc);
    let selected = evaluator.select(&query, node, &registered)?;
    // Los nodos de namespace se identifican por elemento y prefijo.
    let mut set = HashSet::with_capacity(selected.len());
    for n in selected {
        set.insert(match n {
            XNode::Node(id) => Key::Node(id),
            XNode::Attr(id, i) => Key::Attr(id, i),
            XNode::Ns(id, i) => Key::Ns(id, evaluator.ns_nodes(id)[i].prefix.clone()),
        });
    }
    Ok(Visibility::Set(set))
}

/// Tamaño de bloque con que [`canonicalize_to`] entrega la salida.
pub(crate) const SINK_CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Node(NodeId),
    Attr(NodeId, usize),
    Ns(NodeId, Option<String>),
}

/// Conjunto de nodos visibles (`xmlC14NIsVisible`).
enum Visibility {
    /// Sin conjunto: todo es visible.
    All,
    /// Subárbol de un elemento con sus atributos y nodos de namespace.
    Subtree(NodeId),
    Set(HashSet<Key>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pos {
    Before,
    Inside,
    After,
}

/// Pila de namespaces visibles de libxml2 (`xmlC14NVisibleNsStack`).
struct NsStack<'a> {
    entries: Vec<(Option<&'a str>, &'a str, NodeId)>,
    /// Índices de `entries` por prefijo, creado solo cuando la pila crece
    /// (un elemento con miles de declaraciones haría cuadrática la búsqueda).
    index: Option<HashMap<&'a str, Vec<usize>>>,
    prev_start: usize,
    prev_end: usize,
}

/// A partir de este tamaño la pila mantiene el índice por prefijo.
const NS_INDEX_THRESHOLD: usize = 64;

fn empty_prefix(p: Option<&str>) -> bool {
    p.is_none_or(str::is_empty)
}

fn same_prefix(a: Option<&str>, b: Option<&str>) -> bool {
    a.unwrap_or("") == b.unwrap_or("")
}

impl<'a> NsStack<'a> {
    fn save(&self) -> (usize, usize, usize) {
        (self.entries.len(), self.prev_start, self.prev_end)
    }

    fn restore(&mut self, state: (usize, usize, usize)) {
        while self.entries.len() > state.0 {
            let Some((prefix, _, _)) = self.entries.pop() else {
                break;
            };
            if let Some(list) = self
                .index
                .as_mut()
                .and_then(|index| index.get_mut(prefix.unwrap_or("")))
            {
                list.pop();
            }
        }
        self.prev_start = state.1;
        self.prev_end = state.2;
    }

    fn shift(&mut self) {
        self.prev_start = self.prev_end;
        self.prev_end = self.entries.len();
    }

    fn add(&mut self, prefix: Option<&'a str>, href: &'a str, owner: NodeId) {
        if self.index.is_none() && self.entries.len() >= NS_INDEX_THRESHOLD {
            let mut index: HashMap<&'a str, Vec<usize>> = HashMap::new();
            for (i, (p, _, _)) in self.entries.iter().enumerate() {
                index.entry(p.unwrap_or("")).or_default().push(i);
            }
            self.index = Some(index);
        }
        if let Some(index) = self.index.as_mut() {
            index
                .entry(prefix.unwrap_or(""))
                .or_default()
                .push(self.entries.len());
        }
        self.entries.push((prefix, href, owner));
    }

    /// Entrada más reciente con ese prefijo.
    fn latest(&self, prefix: Option<&str>) -> Option<usize> {
        match &self.index {
            Some(index) => index.get(prefix.unwrap_or(""))?.last().copied(),
            None => self
                .entries
                .iter()
                .rposition(|(p, _, _)| same_prefix(*p, prefix)),
        }
    }

    /// `xmlC14NVisibleNsStackFind`.
    fn find(&self, prefix: Option<&str>, href: &str) -> bool {
        let has_empty = empty_prefix(prefix) && href.is_empty();
        let start = if has_empty { 0 } else { self.prev_start };
        match self.latest(prefix) {
            Some(i) if i >= start => self.entries[i].1 == href,
            _ => has_empty,
        }
    }
}

struct Engine<'a, 's> {
    doc: &'a Document,
    method: &'a Method,
    visibility: Visibility,
    /// Nodos del subárbol (`Visibility::Subtree`): pertenencia en O(1). Subir
    /// por los padres en cada consulta multiplicaba el costo por la
    /// profundidad.
    subtree: Option<HashSet<NodeId>>,
    stack: NsStack<'a>,
    /// Namespaces en ámbito de los elementos abiertos (solo C14N inclusiva):
    /// el primer elemento visitado y cada uno que declara alguno apilan su
    /// lista; los demás usan la del ancestro más cercano. Equivale a
    /// [`Document::in_scope_namespace_refs`] sin recorrer los ancestros en
    /// cada elemento.
    in_scope: Vec<Vec<&'a NsDecl>>,
    pos: Pos,
    parent_is_doc: bool,
    out: String,
    /// Destino por bloques; sin él la salida se acumula en `out`.
    sink: Option<&'s mut dyn FnMut(&str)>,
}

impl<'a, 's> Engine<'a, 's> {
    fn new(
        doc: &'a Document,
        method: &'a Method,
        visibility: Visibility,
        sink: Option<&'s mut dyn FnMut(&str)>,
    ) -> Self {
        let subtree = match &visibility {
            Visibility::Subtree(apex) => {
                let mut set: HashSet<NodeId> = doc.descendants(*apex).into_iter().collect();
                set.insert(*apex);
                Some(set)
            }
            Visibility::All | Visibility::Set(_) => None,
        };
        Self {
            doc,
            method,
            visibility,
            subtree,
            stack: NsStack {
                entries: Vec::new(),
                index: None,
                prev_start: 0,
                prev_end: 0,
            },
            in_scope: Vec::new(),
            pos: Pos::Before,
            parent_is_doc: true,
            out: String::with_capacity(4096),
            sink,
        }
    }

    /// Entrega lo acumulado al destino cuando supera el bloque (o siempre,
    /// con `force`).
    fn flush(&mut self, force: bool) {
        if let Some(sink) = self.sink.as_mut() {
            if force || self.out.len() >= SINK_CHUNK {
                sink(&self.out);
                self.out.clear();
            }
        }
    }

    fn run(mut self) -> String {
        // Con `Subtree`, fuera del subárbol nada es visible: esos nodos no
        // escriben nada ni tocan la pila (solo los visibles añaden
        // namespaces), y lo heredado se lee de los ancestros en el DOM.
        // Empezar en el ápice da la misma salida que recorrer todo el
        // documento, como libxml2, sin visitar el resto (p. ej. al firmar
        // `SignedInfo` dentro de una factura grande).
        let start = match self.visibility {
            Visibility::Subtree(apex) => apex,
            Visibility::All | Visibility::Set(_) => self.doc.document_node(),
        };
        self.node(start);
        self.flush(true);
        self.out
    }

    fn in_subtree(&self, apex: NodeId, node: NodeId) -> bool {
        if let Some(set) = &self.subtree {
            return set.contains(&node);
        }
        let mut cur = Some(node);
        while let Some(n) = cur {
            if n == apex {
                return true;
            }
            cur = self.doc.parent(n);
        }
        false
    }

    fn visible_attr(&self, element: NodeId, index: usize) -> bool {
        match &self.visibility {
            Visibility::All => true,
            Visibility::Subtree(apex) => self.in_subtree(*apex, element),
            Visibility::Set(set) => set.contains(&Key::Attr(element, index)),
        }
    }

    fn visible(&self, key: &Key) -> bool {
        match &self.visibility {
            Visibility::All => true,
            Visibility::Subtree(apex) => match key {
                Key::Node(n) | Key::Attr(n, _) | Key::Ns(n, _) => self.in_subtree(*apex, *n),
            },
            Visibility::Set(set) => set.contains(key),
        }
    }

    /// Visibilidad de un nodo de namespace: XPath no define nodos para un
    /// `xmlns` sin declaración, pero libxml2 sí incluye `xmlns=""`.
    fn ns_visible(&self, element: NodeId, prefix: Option<&str>) -> bool {
        match &self.visibility {
            Visibility::All => true,
            Visibility::Subtree(apex) => self.in_subtree(*apex, element),
            Visibility::Set(set) => {
                let prefix = prefix.filter(|p| !p.is_empty()).map(str::to_owned);
                set.contains(&Key::Ns(element, prefix))
            }
        }
    }

    /// `xmlExcC14NVisibleNsStackFind`.
    fn exc_find(&self, prefix: Option<&str>, href: &str) -> bool {
        let has_empty = empty_prefix(prefix) && href.is_empty();
        match self.stack.latest(prefix) {
            Some(i) => {
                let (p, h, owner) = self.stack.entries[i];
                h == href && self.ns_visible(owner, p)
            }
            None => has_empty,
        }
    }

    fn node(&mut self, node: NodeId) {
        let doc = self.doc;
        match doc.kind(node) {
            NodeKind::Document => {
                for child in doc.children(node) {
                    self.node(*child);
                }
            }
            NodeKind::Element(_) => self.element(node),
            NodeKind::Text(t) | NodeKind::CData(t) => {
                if self.visible(&Key::Node(node)) {
                    escape_text(&mut self.out, t);
                    self.flush(false);
                }
            }
            NodeKind::Comment(t) => {
                if self.method.with_comments && self.visible(&Key::Node(node)) {
                    self.top_level_open();
                    self.out.push_str("<!--");
                    self.out.push_str(t);
                    self.out.push_str("-->");
                    self.top_level_close();
                }
            }
            NodeKind::ProcessingInstruction { target, data } => {
                if self.visible(&Key::Node(node)) {
                    self.top_level_open();
                    self.out.push_str("<?");
                    self.out.push_str(target);
                    if !data.is_empty() {
                        self.out.push(' ');
                        self.out.push_str(data);
                    }
                    self.out.push_str("?>");
                    self.top_level_close();
                }
            }
        }
    }

    fn top_level_open(&mut self) {
        if self.pos == Pos::After {
            self.out.push('\n');
        }
    }

    fn top_level_close(&mut self) {
        if self.pos == Pos::Before {
            self.out.push('\n');
        }
    }

    fn element(&mut self, node: NodeId) {
        let doc = self.doc;
        let Some(element) = doc.element(node) else {
            return;
        };
        let visible = self.visible(&Key::Node(node));
        let state = self.stack.save();
        let scoped = !self.method.exclusive && self.enter_scope(node, element);
        let mut was_doc_element = false;
        if visible {
            if self.parent_is_doc {
                was_doc_element = true;
                self.parent_is_doc = false;
                self.pos = Pos::Inside;
            }
            self.out.push('<');
            element.push_qname(&mut self.out);
        }
        if self.method.exclusive {
            self.exclusive_namespaces(node, visible);
        } else {
            self.inclusive_namespaces(node, visible);
        }
        if visible {
            self.stack.shift();
        }
        self.attributes(node, visible);
        if visible {
            self.out.push('>');
        }
        for child in doc.children(node) {
            self.node(*child);
        }
        if visible {
            self.out.push_str("</");
            element.push_qname(&mut self.out);
            self.out.push('>');
            if was_doc_element {
                self.parent_is_doc = true;
                self.pos = Pos::After;
            }
        }
        if scoped {
            self.in_scope.pop();
        }
        self.stack.restore(state);
        self.flush(false);
    }

    /// Apila los namespaces en ámbito de `node` si no son los del ancestro
    /// más cercano ya apilado; devuelve si apiló. El orden es el de
    /// [`Document::in_scope_namespace_refs`]: la última declaración de cada
    /// prefijo, de la más externa a la más interna.
    fn enter_scope(&mut self, node: NodeId, element: &'a Element) -> bool {
        let list = match self.in_scope.last() {
            None => self.doc.in_scope_namespace_refs(node),
            Some(_) if element.ns_decls.is_empty() => return false,
            Some(inherited) => {
                retain_last_per_prefix(inherited.iter().copied().chain(&element.ns_decls).collect())
            }
        };
        self.in_scope.push(list);
        true
    }

    fn print_namespaces(&mut self, mut list: Vec<(Option<&str>, &str)>) {
        list.sort_by(|a, b| a.0.unwrap_or("").cmp(b.0.unwrap_or("")));
        list.dedup();
        for (prefix, uri) in list {
            self.print_ns(prefix, uri);
        }
    }

    fn print_ns(&mut self, prefix: Option<&str>, uri: &str) {
        self.out.push_str(" xmlns");
        if let Some(p) = prefix.filter(|p| !p.is_empty()) {
            self.out.push(':');
            self.out.push_str(p);
        }
        self.out.push_str("=\"");
        escape_attribute(&mut self.out, uri);
        self.out.push('"');
    }

    /// `xmlC14NProcessNamespacesAxis`.
    fn inclusive_namespaces(&mut self, node: NodeId, visible: bool) {
        let mut list = Vec::new();
        let mut has_empty_ns = false;
        // La lista se aparta mientras se recorre (la pila de ámbitos no cambia
        // hasta los hijos) y se devuelve al final.
        let scope = self.in_scope.pop().unwrap_or_default();
        for &decl in &scope {
            let prefix = decl.prefix.as_deref();
            if prefix == Some("xml") || !self.ns_visible(node, prefix) {
                continue;
            }
            let already = self.stack.find(prefix, &decl.uri);
            if visible {
                self.stack.add(prefix, &decl.uri, node);
            }
            if !already {
                list.push((prefix, decl.uri.as_str()));
            }
            if empty_prefix(prefix) {
                has_empty_ns = true;
            }
        }
        self.in_scope.push(scope);
        if visible && !has_empty_ns && !self.stack.find(None, "") {
            self.print_ns(None, "");
        }
        self.print_namespaces(list);
    }

    /// `xmlExcC14NProcessNamespacesAxis`.
    fn exclusive_namespaces(&mut self, node: NodeId, visible: bool) {
        let doc = self.doc;
        let method = self.method;
        let Some(element) = doc.element(node) else {
            return;
        };
        let mut list = Vec::new();
        let mut has_empty_ns = false;
        let mut has_empty_in_list = false;
        let mut has_visibly_utilized_empty = false;
        for p in &method.inclusive_prefixes {
            let prefix = if p == "#default" || p.is_empty() {
                has_empty_in_list = true;
                None
            } else {
                Some(p.as_str())
            };
            let Some(uri) = declared_uri(doc, node, prefix) else {
                continue;
            };
            if prefix == Some("xml") || !self.ns_visible(node, prefix) {
                continue;
            }
            let already = self.stack.find(prefix, uri);
            if visible {
                self.stack.add(prefix, uri, node);
            }
            if !already {
                list.push((prefix, uri));
            }
            if empty_prefix(prefix) {
                has_empty_ns = true;
            }
        }
        // Namespace del elemento.
        let own = match &element.namespace {
            Some(uri) => Some((element.prefix.as_deref(), uri.as_str())),
            None => {
                has_visibly_utilized_empty = true;
                declared_uri(doc, node, None).map(|u| (None, u))
            }
        };
        if let Some((prefix, uri)) = own {
            if prefix != Some("xml") {
                if visible && self.ns_visible(node, prefix) && !self.exc_find(prefix, uri) {
                    list.push((prefix, uri));
                }
                if visible {
                    self.stack.add(prefix, uri, node);
                }
                if empty_prefix(prefix) {
                    has_empty_ns = true;
                }
            }
        }
        // Namespaces de los atributos visibles.
        for (i, a) in element.attributes.iter().enumerate() {
            let Some(uri) = &a.namespace else { continue };
            if uri == XML_NS || !self.visible(&Key::Attr(node, i)) {
                continue;
            }
            let already = self.exc_find(a.prefix.as_deref(), uri);
            self.stack.add(a.prefix.as_deref(), uri, node);
            if !already && visible {
                list.push((a.prefix.as_deref(), uri.as_str()));
            }
            if empty_prefix(a.prefix.as_deref()) {
                has_empty_ns = true;
            }
        }
        if visible && has_visibly_utilized_empty && !has_empty_ns && !has_empty_in_list {
            if !self.exc_find(None, "") {
                self.print_ns(None, "");
            }
        } else if visible && !has_empty_ns && has_empty_in_list && !self.stack.find(None, "") {
            self.print_ns(None, "");
        }
        self.print_namespaces(list);
    }

    /// `xmlC14NProcessAttrsAxis`.
    fn attributes(&mut self, node: NodeId, visible: bool) {
        let doc = self.doc;
        let Some(element) = doc.element(node) else {
            return;
        };
        // C14N 1.0: un elemento visible cuyo padre no lo es hereda los
        // atributos xml:* de sus ancestros.
        let inherited_from = (!self.method.exclusive && visible)
            .then(|| doc.parent(node))
            .flatten()
            .filter(|parent| !self.visible(&Key::Node(*parent)));
        if inherited_from.is_none() && element.attributes.len() <= 1 {
            // Como mucho un atributo y nada heredado: no hay que ordenar.
            if let Some(a) = element.attributes.first() {
                if self.visible_attr(node, 0) {
                    self.write_attribute(a);
                }
            }
            return;
        }
        let mut list: Vec<&Attribute> = element
            .attributes
            .iter()
            .enumerate()
            .filter(|(i, _)| self.visible_attr(node, *i))
            .map(|(_, a)| a)
            .collect();
        if let Some(parent) = inherited_from {
            // Nombres xml:* ya presentes (el más cercano gana): con un
            // conjunto, la herencia es lineal en los atributos de los
            // ancestros (antes, cuadrática: 2,9 MB → 35 s).
            let mut taken: HashSet<&str> = list
                .iter()
                .filter(|x| x.namespace.as_deref() == Some(XML_NS))
                .map(|x| x.local.as_str())
                .collect();
            let mut ancestor = Some(parent);
            while let Some(a) = ancestor {
                if let Some(e) = doc.element(a) {
                    for attr in &e.attributes {
                        if attr.namespace.as_deref() == Some(XML_NS)
                            && taken.insert(attr.local.as_str())
                        {
                            list.push(attr);
                        }
                    }
                }
                ancestor = doc.parent(a);
            }
        }
        if list.len() > 1 {
            list.sort_by(|a, b| {
                (
                    a.namespace.is_some(),
                    a.namespace.as_deref().unwrap_or(""),
                    a.local.as_str(),
                )
                    .cmp(&(
                        b.namespace.is_some(),
                        b.namespace.as_deref().unwrap_or(""),
                        b.local.as_str(),
                    ))
            });
        }
        for a in list {
            self.write_attribute(a);
        }
    }

    fn write_attribute(&mut self, a: &Attribute) {
        self.out.push(' ');
        a.push_qname(&mut self.out);
        self.out.push_str("=\"");
        escape_attribute(&mut self.out, &a.value);
        self.out.push('"');
    }
}

/// URI de la declaración en ámbito para `prefix` (`xmlSearchNs`), incluida
/// la declaración vacía `xmlns=""`.
fn declared_uri<'d>(doc: &'d Document, node: NodeId, prefix: Option<&str>) -> Option<&'d str> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if let Some(e) = doc.element(n) {
            if let Some(d) = e.ns_decls.iter().find(|d| d.prefix.as_deref() == prefix) {
                return Some(d.uri.as_str());
            }
        }
        cur = doc.parent(n);
    }
    None
}

/// Copia `text` escapando los bytes ASCII que `escape` reemplaza; el resto
/// (incluido UTF-8 multibyte) se copia en bloques.
fn escape_with(out: &mut String, text: &str, escape: fn(u8) -> Option<&'static str>) {
    let mut start = 0;
    for (i, b) in text.bytes().enumerate() {
        if let Some(rep) = escape(b) {
            out.push_str(&text[start..i]);
            out.push_str(rep);
            start = i + 1;
        }
    }
    out.push_str(&text[start..]);
}

fn escape_text(out: &mut String, text: &str) {
    escape_with(out, text, |b| match b {
        b'&' => Some("&amp;"),
        b'<' => Some("&lt;"),
        b'>' => Some("&gt;"),
        b'\r' => Some("&#xD;"),
        _ => None,
    });
}

fn escape_attribute(out: &mut String, text: &str) {
    escape_with(out, text, |b| match b {
        b'&' => Some("&amp;"),
        b'<' => Some("&lt;"),
        b'"' => Some("&quot;"),
        b'\t' => Some("&#x9;"),
        b'\n' => Some("&#xA;"),
        b'\r' => Some("&#xD;"),
        _ => None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c14n(xml: &str, uri: &str) -> String {
        let doc = Document::parse(xml).unwrap();
        canonicalize(&doc, doc.document_node(), &Method::from_uri(uri).unwrap())
    }

    #[test]
    fn spec_examples() {
        assert_eq!(
            c14n("<a b='1' a='2'><c/></a>", C14N),
            "<a a=\"2\" b=\"1\"><c></c></a>"
        );
        assert_eq!(
            c14n("<?pi?><!--x--><a/><!--y-->", C14N_COMMENTS),
            "<?pi?>\n<!--x-->\n<a></a>\n<!--y-->"
        );
        assert_eq!(
            c14n("<a xmlns:x='u'><x:b xmlns:y='v'/></a>", EXC_C14N),
            "<a><x:b xmlns:x=\"u\"></x:b></a>"
        );
    }

    #[test]
    fn xpath_subset() {
        let doc = Document::parse("<r xmlns='u' a='1'><x>t</x><!--c--></r>").unwrap();
        let m = Method::from_uri(C14N).unwrap();
        let root = doc.document_node();
        let out = canonicalize_xpath(&doc, root, &m, "not(self::text())", &[]).unwrap();
        assert_eq!(out, "<r xmlns=\"u\" a=\"1\"><x></x></r>");
        assert!(canonicalize_xpath(&doc, root, &m, "zz:x", &[]).is_err());
    }
}
