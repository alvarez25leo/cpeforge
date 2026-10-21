//! Subconjunto de XPath 1.0 que usan los parsers.
//!
//! Admite rutas de pasos por nombre: absolutas (`/xt:Invoice/cbc:ID`),
//! relativas (`cac:Party/cbc:Name`), con descendientes (`//sac:X`, `a//b`),
//! comodín `*` y un atributo final (`cbc:ID/@schemeID`). Cada prefijo se
//! resuelve primero con los espacios de nombres en ámbito del nodo de contexto
//! y después con los registrados. Si alguno no está definido, la consulta no
//! tiene resultado ([`Eval::Undefined`]), aunque el paso no llegue a
//! evaluarse. Las demás expresiones (funciones, predicados, ejes explícitos)
//! se rechazan al compilar.
//!
//! La compilación toma prestados los nombres de la consulta y la evaluación
//! recorre el árbol sin vectores intermedios por nodo.

use crate::dom::{Node, XmlDocument};

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Test<'q> {
    Any,
    Name {
        prefix: Option<&'q str>,
        local: &'q str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step<'q> {
    /// `true` para `//` antes del paso (descendant-or-self).
    descendant: bool,
    attribute: bool,
    test: Test<'q>,
}

/// Consulta compilada; sus nombres son porciones de la consulta original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Path<'q> {
    absolute: bool,
    steps: Vec<Step<'q>>,
}

/// Resultado de evaluar una consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Eval<'a> {
    /// Nodos en orden de documento, sin duplicados.
    Nodes(Vec<Node<'a>>),
    /// Prefijo no definido: la consulta no tiene resultado.
    Undefined(String),
}

/// Nombre ya resuelto de un paso: `None` es el comodín.
type Resolved<'a> = Option<(Option<&'a str>, &'a str)>;

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.') || c == '\u{B7}'
}

fn ncname(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(is_name_start) && chars.all(is_name_char)
}

impl<'q> Path<'q> {
    /// Compila la consulta.
    ///
    /// # Errors
    ///
    /// Motivo legible si la expresión está fuera del subconjunto.
    pub(crate) fn compile(query: &'q str) -> Result<Self, String> {
        let (absolute, mut rest, mut descendant) = if let Some(r) = query.strip_prefix("//") {
            (true, r, true)
        } else if let Some(r) = query.strip_prefix('/') {
            (true, r, false)
        } else {
            (false, query, false)
        };
        let mut steps = Vec::with_capacity(rest.bytes().filter(|&b| b == b'/').count() + 1);
        loop {
            let end = rest.find('/').unwrap_or(rest.len());
            let (token, tail) = rest.split_at(end);
            if token.is_empty() {
                return Err("paso vacío".into());
            }
            if steps.last().is_some_and(|s: &Step| s.attribute) {
                return Err("el atributo debe ser el último paso".into());
            }
            let (attribute, name) = match token.strip_prefix('@') {
                Some(n) => (true, n),
                None => (false, token),
            };
            let test = if name == "*" {
                Test::Any
            } else {
                let (prefix, local) = match name.split_once(':') {
                    Some((p, l)) => (Some(p), l),
                    None => (None, name),
                };
                if !prefix.is_none_or(ncname) || !ncname(local) {
                    return Err(format!("expresión no admitida: {token:?}"));
                }
                Test::Name { prefix, local }
            };
            steps.push(Step {
                descendant,
                attribute,
                test,
            });
            if tail.is_empty() {
                break;
            }
            (rest, descendant) = match tail.strip_prefix("//") {
                Some(r) => (r, true),
                None => (tail.get(1..).unwrap_or_default(), false),
            };
        }
        Ok(Self { absolute, steps })
    }

    /// Evalúa desde `context` con los prefijos registrados `registered`.
    pub(crate) fn evaluate<'a>(
        &self,
        document: &'a XmlDocument,
        context: Node<'a>,
        registered: &[(&str, &str)],
    ) -> Eval<'a> {
        // Todos los prefijos se resuelven antes de evaluar, aunque el conjunto
        // de contexto quede vacío en un paso anterior: un prefijo indefinido
        // en cualquier paso deja la consulta sin resultado.
        let mut first_name = None;
        for (index, step) in self.steps.iter().enumerate() {
            match resolve(step.test, context, registered) {
                Err(prefix) => return Eval::Undefined(prefix.to_owned()),
                Ok(name) if index == 0 => first_name = name,
                Ok(_) => {}
            }
        }
        let mut steps = self.steps.iter();
        let mut current: Vec<Node<'a>> = Vec::new();
        // Primer paso desde el origen único (`None` es el nodo documento): el
        // resultado ya sale en orden de documento y sin repetidos.
        if let Some(step) = steps.next() {
            let origin = if self.absolute { None } else { Some(context) };
            extend_step(&mut current, document, origin, step, first_name);
        }
        let mut next: Vec<Node<'a>> = Vec::new();
        for step in steps {
            let name = resolve(step.test, context, registered).unwrap_or(None);
            next.clear();
            for &origin in &current {
                extend_step(&mut next, document, Some(origin), step, name);
            }
            // Desde un único origen el resultado ya está en orden de documento
            // y sin repetidos; con varios, los subárboles pueden solaparse.
            if current.len() > 1 {
                next.sort_by_key(order_key);
                next.dedup();
            }
            std::mem::swap(&mut current, &mut next);
        }
        Eval::Nodes(current)
    }
}

/// Agrega a `out` los nodos que el paso `step` selecciona desde `origin`
/// (`None` es el nodo documento), en orden de documento.
fn extend_step<'a>(
    out: &mut Vec<Node<'a>>,
    document: &'a XmlDocument,
    origin: Option<Node<'a>>,
    step: &Step<'_>,
    name: Resolved<'_>,
) {
    let keep = |n: &Node<'a>| matches(n, name);
    match (origin, step.descendant, step.attribute) {
        (None, false, false) => out.extend(Some(document.root()).filter(keep)),
        (None, false, true) => {}
        (Some(node), false, false) => out.extend(node.child_elements().filter(keep)),
        (Some(node), false, true) => out.extend(node.attributes().filter(keep)),
        // `//x` desde el documento: el raíz y todos sus descendientes
        // elemento.
        (None, true, false) => {
            let root = document.root();
            out.extend(
                std::iter::once(root)
                    .chain(root.descendant_elements())
                    .filter(keep),
            );
        }
        // `n//x`: los hijos de `n` y de cada descendiente son exactamente los
        // descendientes de `n`.
        (Some(node), true, false) => out.extend(node.descendant_elements().filter(keep)),
        // `//@a` y `n//@a`: atributos del nodo base (el raíz si se parte del
        // documento, que no tiene atributos) y de sus descendientes.
        (origin, true, true) => {
            let base = origin.unwrap_or_else(|| document.root());
            out.extend(
                std::iter::once(base)
                    .chain(base.descendant_elements())
                    .flat_map(|e| e.attributes())
                    .filter(keep),
            );
        }
    }
}

/// Resuelve el nombre de un paso en el ámbito de `context`; `Err` con el
/// prefijo si no está definido.
fn resolve<'s>(
    test: Test<'s>,
    context: Node<'s>,
    registered: &[(&'s str, &'s str)],
) -> Result<Resolved<'s>, &'s str> {
    Ok(match test {
        Test::Any => None,
        Test::Name {
            prefix: None,
            local,
        } => Some((None, local)),
        Test::Name {
            prefix: Some(prefix),
            local,
        } => {
            let uri = if prefix == "xml" {
                Some(XML_NAMESPACE)
            } else {
                context.lookup_namespace(prefix).or_else(|| {
                    registered
                        .iter()
                        .find(|(p, _)| *p == prefix)
                        .map(|(_, uri)| *uri)
                })
            };
            Some((Some(uri.ok_or(prefix)?), local))
        }
    })
}

fn matches(node: &Node<'_>, name: Resolved<'_>) -> bool {
    match name {
        None => true,
        Some((namespace, local)) => node.local_name() == local && node.namespace() == namespace,
    }
}

fn order_key(node: &Node<'_>) -> (usize, usize) {
    (node.index(), node.attr_index().map_or(0, |a| a + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "<r xmlns='urn:r' xmlns:a='urn:a'><a:x i='1'><a:y>1</a:y></a:x><a:x><a:y>2</a:y><a:z xmlns:b='urn:b'><b:w>3</b:w></a:z></a:x></r>";

    fn values(doc: &XmlDocument, query: &str, context: Option<Node<'_>>) -> Option<Vec<String>> {
        let path = Path::compile(query).ok()?;
        match path.evaluate(
            doc,
            context.unwrap_or_else(|| doc.root()),
            &[("t", "urn:r")],
        ) {
            Eval::Nodes(nodes) => Some(nodes.iter().map(Node::value).collect()),
            Eval::Undefined(_) => None,
        }
    }

    #[test]
    fn paths() {
        let doc = XmlDocument::parse(XML.as_bytes()).unwrap_or_else(|e| unreachable!("{e}"));
        let v = |q: &str| values(&doc, q, None);
        assert_eq!(v("a:x/a:y"), Some(vec!["1".into(), "2".into()]));
        assert_eq!(v("/t:r/a:x/a:y"), Some(vec!["1".into(), "2".into()]));
        assert_eq!(v("//a:y"), Some(vec!["1".into(), "2".into()]));
        assert_eq!(v("a:x//a:y"), Some(vec!["1".into(), "2".into()]));
        assert_eq!(v("//t:r"), Some(vec!["123".into()]));
        assert_eq!(v("a:x/@i"), Some(vec!["1".into()]));
        assert_eq!(v("*/*"), Some(vec!["1".into(), "2".into(), "3".into()]));
        assert_eq!(v("r"), Some(vec![]));
        // `b` solo está en ámbito dentro de a:z; `zz` en ningún sitio.
        assert_eq!(v("//b:w"), None);
        assert_eq!(v("a:nada/zz:x"), None);
        let z = doc
            .root()
            .descendant_elements()
            .find(|n| n.local_name() == "z");
        assert_eq!(values(&doc, "//b:w", z), Some(vec!["3".into()]));
        assert_eq!(values(&doc, "b:w", z), Some(vec!["3".into()]));
    }

    /// Evaluación literal de la definición (descendant-or-self seguido de
    /// child/attribute, ordenar y quitar repetidos en cada paso), como la
    /// implementación anterior.
    fn reference<'a>(
        path: &Path<'_>,
        doc: &'a XmlDocument,
        context: Node<'a>,
        registered: &[(&str, &str)],
    ) -> Option<Vec<Node<'a>>> {
        let mut names = Vec::new();
        for step in &path.steps {
            names.push(resolve(step.test, context, registered).ok()?);
        }
        let mut current: Vec<Option<Node<'a>>> =
            vec![if path.absolute { None } else { Some(context) }];
        for (step, name) in path.steps.iter().zip(names) {
            let mut next = Vec::new();
            for origin in &current {
                let mut bases = vec![*origin];
                if step.descendant {
                    match origin {
                        None => {
                            bases.push(Some(doc.root()));
                            bases.extend(doc.root().descendant_elements().map(Some));
                        }
                        Some(n) => bases.extend(n.descendant_elements().map(Some)),
                    }
                }
                for base in bases {
                    let candidates: Vec<Node<'a>> = match (base, step.attribute) {
                        (None, false) => vec![doc.root()],
                        (None, true) => vec![],
                        (Some(n), false) => n.child_elements().collect(),
                        (Some(n), true) => n.attributes().collect(),
                    };
                    next.extend(candidates.into_iter().filter(|n| matches(n, name)));
                }
            }
            next.sort_by_key(order_key);
            next.dedup();
            current = next.into_iter().map(Some).collect();
        }
        Some(current.into_iter().flatten().collect())
    }

    #[test]
    fn matches_reference_semantics() {
        let docs = [
            XML,
            "<a:x xmlns:a='urn:a' k='0'><a:x k='1'><a:y k='2'/><a:x k='3'><a:y k='4'>t</a:y></a:x></a:x><a:y k='5'/><b xmlns='urn:a' k='6'><a:x><a:y k='7'/></a:x></b></a:x>",
            "<r xmlns:p='urn:p'><p:a><p:a><p:a/></p:a><p:b><p:a i='1'/></p:b></p:a><p:a/></r>",
        ];
        let queries = [
            "a:x",
            "a:x/a:y",
            "//a:x",
            "//a:y",
            "//a:x//a:y",
            "//a:x/a:y",
            "a:x//a:y",
            "//a:x//a:x//a:y",
            "//*",
            "*/*",
            "//@k",
            "a:x//@k",
            "//a:x/@k",
            "*//@k",
            "/a:x/a:x",
            "//p:a",
            "//p:a//p:a",
            "//p:a/p:a",
            "p:a//p:a/@i",
            "//p:b//p:a",
            "/*",
            "//*/*",
            "//t:r",
            "a:nada//a:y",
        ];
        for xml in docs {
            let doc = XmlDocument::parse(xml.as_bytes()).unwrap_or_else(|e| unreachable!("{e}"));
            let contexts = std::iter::once(doc.root()).chain(doc.root().descendant_elements());
            for context in contexts {
                for query in queries {
                    let Ok(path) = Path::compile(query) else {
                        continue;
                    };
                    let registered = [("t", "urn:r")];
                    let expected = reference(&path, &doc, context, &registered);
                    let actual = match path.evaluate(&doc, context, &registered) {
                        Eval::Nodes(nodes) => Some(nodes),
                        Eval::Undefined(_) => None,
                    };
                    assert_eq!(actual, expected, "{query} desde {context:?} en {xml}");
                }
            }
        }
    }

    #[test]
    fn rejects_outside_subset() {
        for q in [
            "", "/", "a//", "count(a)", "a[1]", "@a/b", "child::a", "a | b", ".", "a/",
        ] {
            assert!(Path::compile(q).is_err(), "{q}");
        }
    }
}
