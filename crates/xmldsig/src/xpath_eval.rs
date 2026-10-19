//! Evaluador XPath 1.0 sobre [`Document`] para la transformación XPath de
//! XMLDSig (`http://www.w3.org/TR/1999/REC-xpath-19991116`).
//!
//! Reproduce lo observable de libxml2, que es lo que usa la referencia:
//!
//! - el eje `namespace` incluye `xml` primero y también la declaración
//!   vacía `xmlns=""` ([`Document::namespace_nodes`]);
//! - en un conjunto ordenado, los nodos de namespace van **después** de todos
//!   los demás, agrupados por elemento;
//! - texto y CDATA son nodos de texto distintos;
//! - `string(número)` sigue `xmlXPathFormatNumber` y `number(cadena)` acepta
//!   exponente.
//!
//! Para acotar el coste ante expresiones hostiles (p. ej. `following::*` en
//! cada nodo de un documento grande), la evaluación cuenta los nodos que
//! recorren los ejes y falla al superar [`MAX_VISITS`].

#![allow(
    clippy::float_arithmetic,
    reason = "XPath 1.0 define sus números como dobles IEEE 754"
)]

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::dom::{Document, NodeId, NodeKind, NsDecl, XML_NS};
use crate::error::Error;

/// Máximo de nodos que los ejes pueden recorrer en una evaluación.
pub(crate) const MAX_VISITS: usize = 10_000_000;

/// Longitud máxima de una expresión, en caracteres. El analizador y el
/// evaluador son recursivos y las expresiones de una firma recibida son
/// datos no confiables: sin tope, `((((…` o `1+1+1+…` agotan la pila y abortan
/// el proceso. Las transformaciones reales miden unas decenas de caracteres.
const MAX_EXPR_CHARS: usize = 16 * 1024;
/// Anidamiento máximo (paréntesis, argumentos, predicados, signos).
const MAX_EXPR_DEPTH: usize = 32;
/// Operadores binarios, de unión y signos por expresión: acota también la
/// profundidad de los árboles de asociatividad izquierda.
const MAX_EXPR_OPERATORS: usize = 128;

/// Nodo XPath: nodo del árbol, atributo (elemento, índice) o nodo de
/// namespace (elemento, índice en [`Document::namespace_nodes`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum XNode {
    Node(NodeId),
    Attr(NodeId, usize),
    Ns(NodeId, usize),
}

fn err(msg: impl Into<String>) -> Error {
    Error::XPath(msg.into())
}

// ----------------------------------------------------------------------
// Léxico
// ----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Slash,
    DSlash,
    LParen,
    RParen,
    LBrack,
    RBrack,
    Dot,
    DDot,
    At,
    Comma,
    DColon,
    Pipe,
    Plus,
    Minus,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    Star,
    Mul,
    And,
    Or,
    Mod,
    Div,
    Literal(String),
    Number(f64),
    Name(String),
    Var(String),
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || (!c.is_ascii() && !c.is_whitespace())
}

fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '.' || c == '-'
}

fn tokenize(input: &str) -> Result<Vec<Tok>, Error> {
    if input.len() > MAX_EXPR_CHARS * 4 || input.chars().count() > MAX_EXPR_CHARS {
        return Err(err("expresión XPath demasiado larga"));
    }
    let chars: Vec<char> = input.chars().collect();
    let mut toks: Vec<Tok> = Vec::new();
    let mut i = 0;
    // `*` y los nombres de operador dependen del token anterior (XPath 3.7).
    let operator_context = |toks: &[Tok]| match toks.last() {
        None => false,
        Some(t) => !matches!(
            t,
            Tok::At
                | Tok::DColon
                | Tok::LParen
                | Tok::LBrack
                | Tok::Comma
                | Tok::Slash
                | Tok::DSlash
                | Tok::Pipe
                | Tok::Plus
                | Tok::Minus
                | Tok::Eq
                | Tok::Neq
                | Tok::Lt
                | Tok::Le
                | Tok::Gt
                | Tok::Ge
                | Tok::Mul
                | Tok::And
                | Tok::Or
                | Tok::Mod
                | Tok::Div
        ),
    };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '/' if next == Some('/') => {
                toks.push(Tok::DSlash);
                i += 2;
            }
            '/' => {
                toks.push(Tok::Slash);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '[' => {
                toks.push(Tok::LBrack);
                i += 1;
            }
            ']' => {
                toks.push(Tok::RBrack);
                i += 1;
            }
            '.' if next == Some('.') => {
                toks.push(Tok::DDot);
                i += 2;
            }
            '.' if !next.is_some_and(|n| n.is_ascii_digit()) => {
                toks.push(Tok::Dot);
                i += 1;
            }
            '@' => {
                toks.push(Tok::At);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            ':' if next == Some(':') => {
                toks.push(Tok::DColon);
                i += 2;
            }
            '|' => {
                toks.push(Tok::Pipe);
                i += 1;
            }
            '+' => {
                toks.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '=' => {
                toks.push(Tok::Eq);
                i += 1;
            }
            '!' if next == Some('=') => {
                toks.push(Tok::Neq);
                i += 2;
            }
            '<' if next == Some('=') => {
                toks.push(Tok::Le);
                i += 2;
            }
            '<' => {
                toks.push(Tok::Lt);
                i += 1;
            }
            '>' if next == Some('=') => {
                toks.push(Tok::Ge);
                i += 2;
            }
            '>' => {
                toks.push(Tok::Gt);
                i += 1;
            }
            '*' => {
                toks.push(if operator_context(&toks) {
                    Tok::Mul
                } else {
                    Tok::Star
                });
                i += 1;
            }
            '"' | '\'' => {
                let end = chars[i + 1..]
                    .iter()
                    .position(|x| *x == c)
                    .ok_or_else(|| err("literal sin cerrar"))?;
                toks.push(Tok::Literal(chars[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
            }
            '$' => {
                i += 1;
                let start = i;
                while i < chars.len() && (is_name_char(chars[i]) || chars[i] == ':') {
                    i += 1;
                }
                toks.push(Tok::Var(chars[start..i].iter().collect()));
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '.' {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                // libxml2 acepta exponente en los números literales.
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    let mut j = i + 1;
                    if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                        j += 1;
                    }
                    if j < chars.len() && chars[j].is_ascii_digit() {
                        i = j;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                }
                let text: String = chars[start..i].iter().collect();
                toks.push(Tok::Number(
                    text.parse().map_err(|_| err("número inválido"))?,
                ));
            }
            c if is_name_start(c) => {
                let start = i;
                while i < chars.len() && is_name_char(chars[i]) {
                    i += 1;
                }
                // QName (`p:local`) o `p:*`, pero no `eje::`.
                if i + 1 < chars.len() && chars[i] == ':' && chars[i + 1] != ':' {
                    if chars[i + 1] == '*' {
                        i += 2;
                    } else if is_name_start(chars[i + 1]) {
                        i += 1;
                        while i < chars.len() && is_name_char(chars[i]) {
                            i += 1;
                        }
                    }
                }
                let name: String = chars[start..i].iter().collect();
                let op = operator_context(&toks);
                toks.push(match name.as_str() {
                    "and" if op => Tok::And,
                    "or" if op => Tok::Or,
                    "mod" if op => Tok::Mod,
                    "div" if op => Tok::Div,
                    _ => Tok::Name(name),
                });
            }
            other => return Err(err(format!("carácter inesperado '{other}'"))),
        }
    }
    Ok(toks)
}

// ----------------------------------------------------------------------
// Sintaxis
// ----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Ancestor,
    AncestorOrSelf,
    Attribute,
    Child,
    Descendant,
    DescendantOrSelf,
    Following,
    FollowingSibling,
    Namespace,
    Parent,
    Preceding,
    PrecedingSibling,
    Itself,
}

impl Axis {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "ancestor" => Self::Ancestor,
            "ancestor-or-self" => Self::AncestorOrSelf,
            "attribute" => Self::Attribute,
            "child" => Self::Child,
            "descendant" => Self::Descendant,
            "descendant-or-self" => Self::DescendantOrSelf,
            "following" => Self::Following,
            "following-sibling" => Self::FollowingSibling,
            "namespace" => Self::Namespace,
            "parent" => Self::Parent,
            "preceding" => Self::Preceding,
            "preceding-sibling" => Self::PrecedingSibling,
            "self" => Self::Itself,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
enum Test {
    /// Nombre con namespace resuelto (`None` sin prefijo).
    Name(Option<String>, String),
    Any,
    AnyInNs(String),
    Node,
    Text,
    Comment,
    Pi(Option<String>),
}

#[derive(Debug, Clone)]
struct Step {
    axis: Axis,
    test: Test,
    preds: Vec<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Or,
    And,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone)]
enum Expr {
    Binary(Op, Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Union(Box<Expr>, Box<Expr>),
    /// Ruta absoluta (`true`) o relativa.
    Path(bool, Vec<Step>),
    /// Expresión filtro, predicados y pasos posteriores.
    Filter(Box<Expr>, Vec<Expr>, Vec<Step>),
    Literal(String),
    Number(f64),
    Call(String, Vec<Expr>),
}

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    namespaces: &'a HashMap<String, String>,
    depth: usize,
    operators: usize,
}

const NODE_TYPES: [&str; 4] = ["comment", "text", "processing-instruction", "node"];

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn peek_at(&self, offset: usize) -> Option<&Tok> {
        self.toks.get(self.pos + offset)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn expect(&mut self, tok: &Tok) -> Result<(), Error> {
        if self.peek() == Some(tok) {
            self.pos += 1;
            Ok(())
        } else {
            Err(err(format!("se esperaba {tok:?}")))
        }
    }

    fn binary(
        &mut self,
        ops: &[(Tok, Op)],
        next: fn(&mut Self) -> Result<Expr, Error>,
    ) -> Result<Expr, Error> {
        let mut left = next(self)?;
        loop {
            let Some(op) = self
                .peek()
                .and_then(|t| ops.iter().find(|(tok, _)| tok == t).map(|(_, op)| *op))
            else {
                return Ok(left);
            };
            self.pos += 1;
            self.count_operator()?;
            let right = next(self)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn count_operator(&mut self) -> Result<(), Error> {
        self.operators += 1;
        if self.operators > MAX_EXPR_OPERATORS {
            return Err(err("expresión XPath demasiado compleja"));
        }
        Ok(())
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_EXPR_DEPTH {
            return Err(err("expresión XPath demasiado anidada"));
        }
        Ok(())
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        self.enter()?;
        let result = self.binary(&[(Tok::Or, Op::Or)], Self::and);
        self.depth -= 1;
        result
    }

    fn and(&mut self) -> Result<Expr, Error> {
        self.binary(&[(Tok::And, Op::And)], Self::equality)
    }

    fn equality(&mut self) -> Result<Expr, Error> {
        self.binary(&[(Tok::Eq, Op::Eq), (Tok::Neq, Op::Neq)], Self::relational)
    }

    fn relational(&mut self) -> Result<Expr, Error> {
        self.binary(
            &[
                (Tok::Lt, Op::Lt),
                (Tok::Le, Op::Le),
                (Tok::Gt, Op::Gt),
                (Tok::Ge, Op::Ge),
            ],
            Self::additive,
        )
    }

    fn additive(&mut self) -> Result<Expr, Error> {
        self.binary(
            &[(Tok::Plus, Op::Add), (Tok::Minus, Op::Sub)],
            Self::multiplicative,
        )
    }

    fn multiplicative(&mut self) -> Result<Expr, Error> {
        self.binary(
            &[
                (Tok::Mul, Op::Mul),
                (Tok::Div, Op::Div),
                (Tok::Mod, Op::Mod),
            ],
            Self::unary,
        )
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if self.peek() == Some(&Tok::Minus) {
            self.pos += 1;
            self.count_operator()?;
            self.enter()?;
            let operand = self.unary();
            self.depth -= 1;
            return Ok(Expr::Neg(Box::new(operand?)));
        }
        let mut left = self.path_expr()?;
        while self.peek() == Some(&Tok::Pipe) {
            self.pos += 1;
            self.count_operator()?;
            let right = self.path_expr()?;
            left = Expr::Union(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn starts_filter(&self) -> bool {
        match self.peek() {
            Some(Tok::Literal(_) | Tok::Number(_) | Tok::Var(_) | Tok::LParen) => true,
            Some(Tok::Name(n)) => {
                self.peek_at(1) == Some(&Tok::LParen) && !NODE_TYPES.contains(&n.as_str())
            }
            _ => false,
        }
    }

    fn path_expr(&mut self) -> Result<Expr, Error> {
        if !self.starts_filter() {
            return self.location_path();
        }
        let primary = self.primary()?;
        let mut preds = Vec::new();
        while self.peek() == Some(&Tok::LBrack) {
            preds.push(self.predicate()?);
        }
        let mut steps = Vec::new();
        if matches!(self.peek(), Some(Tok::Slash | Tok::DSlash)) {
            self.relative_steps(&mut steps, true)?;
        }
        if preds.is_empty() && steps.is_empty() {
            Ok(primary)
        } else {
            Ok(Expr::Filter(Box::new(primary), preds, steps))
        }
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.next() {
            Some(Tok::Literal(s)) => Ok(Expr::Literal(s)),
            Some(Tok::Number(n)) => Ok(Expr::Number(n)),
            Some(Tok::Var(v)) => Err(err(format!("variable no definida ${v}"))),
            Some(Tok::LParen) => {
                let e = self.expr()?;
                self.expect(&Tok::RParen)?;
                Ok(e)
            }
            Some(Tok::Name(name)) => {
                self.expect(&Tok::LParen)?;
                let mut args = Vec::new();
                if self.peek() != Some(&Tok::RParen) {
                    loop {
                        args.push(self.expr()?);
                        if self.peek() == Some(&Tok::Comma) {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RParen)?;
                if name.contains(':') {
                    return Err(err(format!("función no registrada {name}")));
                }
                Ok(Expr::Call(name, args))
            }
            other => Err(err(format!("expresión inválida en {other:?}"))),
        }
    }

    fn predicate(&mut self) -> Result<Expr, Error> {
        self.expect(&Tok::LBrack)?;
        let e = self.expr()?;
        self.expect(&Tok::RBrack)?;
        Ok(e)
    }

    fn starts_step(&self) -> bool {
        matches!(
            self.peek(),
            Some(Tok::Name(_) | Tok::Star | Tok::Dot | Tok::DDot | Tok::At)
        )
    }

    fn location_path(&mut self) -> Result<Expr, Error> {
        let mut steps = Vec::new();
        match self.peek() {
            Some(Tok::Slash) => {
                self.pos += 1;
                if self.starts_step() {
                    steps.push(self.step()?);
                    self.relative_steps(&mut steps, false)?;
                }
                Ok(Expr::Path(true, steps))
            }
            Some(Tok::DSlash) => {
                self.relative_steps(&mut steps, true)?;
                Ok(Expr::Path(true, steps))
            }
            _ => {
                steps.push(self.step()?);
                self.relative_steps(&mut steps, false)?;
                Ok(Expr::Path(false, steps))
            }
        }
    }

    /// `(('/' | '//') step)*`; con `required`, exige al menos un separador.
    fn relative_steps(&mut self, steps: &mut Vec<Step>, required: bool) -> Result<(), Error> {
        let mut first = true;
        loop {
            match self.peek() {
                Some(Tok::Slash) => {
                    self.pos += 1;
                }
                Some(Tok::DSlash) => {
                    self.pos += 1;
                    steps.push(Step {
                        axis: Axis::DescendantOrSelf,
                        test: Test::Node,
                        preds: Vec::new(),
                    });
                }
                _ => {
                    if required && first {
                        return Err(err("se esperaba '/'"));
                    }
                    return Ok(());
                }
            }
            first = false;
            steps.push(self.step()?);
        }
    }

    fn resolve(&self, prefix: &str) -> Result<String, Error> {
        self.namespaces
            .get(prefix)
            .cloned()
            .ok_or_else(|| err(format!("prefijo de namespace no definido: {prefix}")))
    }

    fn step(&mut self) -> Result<Step, Error> {
        match self.peek() {
            Some(Tok::Dot) => {
                self.pos += 1;
                return Ok(Step {
                    axis: Axis::Itself,
                    test: Test::Node,
                    preds: Vec::new(),
                });
            }
            Some(Tok::DDot) => {
                self.pos += 1;
                return Ok(Step {
                    axis: Axis::Parent,
                    test: Test::Node,
                    preds: Vec::new(),
                });
            }
            _ => {}
        }
        let mut axis = Axis::Child;
        if self.peek() == Some(&Tok::At) {
            self.pos += 1;
            axis = Axis::Attribute;
        } else if let (Some(Tok::Name(n)), Some(Tok::DColon)) = (self.peek(), self.peek_at(1)) {
            axis = Axis::from_name(n).ok_or_else(|| err(format!("eje desconocido {n}")))?;
            self.pos += 2;
        }
        let test = match self.next() {
            Some(Tok::Star) => Test::Any,
            Some(Tok::Name(name)) => {
                if self.peek() == Some(&Tok::LParen) && NODE_TYPES.contains(&name.as_str()) {
                    self.pos += 1;
                    let test = match name.as_str() {
                        "comment" => Test::Comment,
                        "text" => Test::Text,
                        "node" => Test::Node,
                        _ => match self.peek() {
                            Some(Tok::Literal(l)) => {
                                let l = l.clone();
                                self.pos += 1;
                                Test::Pi(Some(l))
                            }
                            _ => Test::Pi(None),
                        },
                    };
                    self.expect(&Tok::RParen)?;
                    test
                } else if let Some(prefix) = name.strip_suffix(":*") {
                    Test::AnyInNs(self.resolve(prefix)?)
                } else if let Some((prefix, local)) = name.split_once(':') {
                    Test::Name(Some(self.resolve(prefix)?), local.to_owned())
                } else {
                    Test::Name(None, name)
                }
            }
            other => return Err(err(format!("prueba de nodo inválida {other:?}"))),
        };
        let mut preds = Vec::new();
        while self.peek() == Some(&Tok::LBrack) {
            preds.push(self.predicate()?);
        }
        Ok(Step { axis, test, preds })
    }
}

// ----------------------------------------------------------------------
// Evaluación
// ----------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Value {
    Nodes(Vec<XNode>),
    Bool(bool),
    Num(f64),
    Str(String),
}

#[derive(Clone, Copy)]
struct Ctx {
    node: XNode,
    pos: usize,
    size: usize,
}

/// Evaluador con los índices de orden de documento del árbol.
pub(crate) struct Evaluator<'a> {
    doc: &'a Document,
    order: HashMap<NodeId, usize>,
    preorder: Vec<NodeId>,
    ns_cache: HashMap<NodeId, Vec<NsDecl>>,
    visits: usize,
}

impl<'a> Evaluator<'a> {
    pub(crate) fn new(doc: &'a Document) -> Self {
        let mut preorder = vec![doc.document_node()];
        preorder.extend(doc.descendants(doc.document_node()));
        let order = preorder.iter().enumerate().map(|(i, n)| (*n, i)).collect();
        Self {
            doc,
            order,
            preorder,
            ns_cache: HashMap::new(),
            visits: 0,
        }
    }

    /// Nodos de namespace del elemento (en caché).
    pub(crate) fn ns_nodes(&mut self, element: NodeId) -> &[NsDecl] {
        let doc = self.doc;
        self.ns_cache
            .entry(element)
            .or_insert_with(|| doc.namespace_nodes(element))
    }

    /// Evalúa `expr` con `context` como nodo contexto; el resultado debe ser
    /// un conjunto de nodos.
    pub(crate) fn select(
        &mut self,
        expr: &str,
        context: NodeId,
        namespaces: &HashMap<String, String>,
    ) -> Result<Vec<XNode>, Error> {
        let toks = tokenize(expr)?;
        let mut parser = Parser {
            toks,
            pos: 0,
            namespaces,
            depth: 0,
            operators: 0,
        };
        let ast = parser.expr()?;
        if parser.pos != parser.toks.len() {
            return Err(err("expresión inválida"));
        }
        let ctx = Ctx {
            node: XNode::Node(context),
            pos: 1,
            size: 1,
        };
        match self.eval(&ast, ctx)? {
            Value::Nodes(n) => Ok(n),
            _ => Err(err("la consulta XPath no devuelve un conjunto de nodos")),
        }
    }

    fn tick(&mut self, n: usize) -> Result<(), Error> {
        self.visits += n;
        if self.visits > MAX_VISITS {
            Err(err("evaluación XPath demasiado costosa"))
        } else {
            Ok(())
        }
    }

    /// Clave de orden: nodos no namespace en orden de documento (atributos
    /// tras su elemento) y, después, los de namespace por elemento.
    fn key(&self, n: XNode) -> (u8, usize, usize) {
        match n {
            XNode::Node(id) => (0, self.order.get(&id).copied().unwrap_or(usize::MAX), 0),
            XNode::Attr(id, i) => (0, self.order.get(&id).copied().unwrap_or(usize::MAX), 1 + i),
            XNode::Ns(id, i) => (1, self.order.get(&id).copied().unwrap_or(usize::MAX), i),
        }
    }

    fn sort(&self, nodes: &mut Vec<XNode>) {
        nodes.sort_by_key(|n| self.key(*n));
        nodes.dedup();
    }

    fn parent(&self, n: XNode) -> Option<XNode> {
        match n {
            XNode::Node(id) => self.doc.parent(id).map(XNode::Node),
            XNode::Attr(id, _) | XNode::Ns(id, _) => Some(XNode::Node(id)),
        }
    }

    fn is_attached(&self, id: NodeId) -> bool {
        self.order.contains_key(&id)
    }

    fn axis(&mut self, n: XNode, axis: Axis) -> Result<Vec<XNode>, Error> {
        let doc = self.doc;
        let out: Vec<XNode> = match axis {
            Axis::Itself => vec![n],
            Axis::Parent => self.parent(n).into_iter().collect(),
            Axis::Ancestor | Axis::AncestorOrSelf => {
                let mut v = Vec::new();
                if axis == Axis::AncestorOrSelf {
                    v.push(n);
                }
                let mut cur = self.parent(n);
                while let Some(p) = cur {
                    v.push(p);
                    cur = self.parent(p);
                }
                v
            }
            Axis::Child => match n {
                XNode::Node(id) => doc.children(id).iter().map(|c| XNode::Node(*c)).collect(),
                _ => Vec::new(),
            },
            Axis::Descendant | Axis::DescendantOrSelf => {
                let mut v = Vec::new();
                if axis == Axis::DescendantOrSelf {
                    v.push(n);
                }
                if let XNode::Node(id) = n {
                    v.extend(doc.descendants(id).into_iter().map(XNode::Node));
                }
                v
            }
            Axis::Attribute => match n {
                XNode::Node(id) => doc
                    .element(id)
                    .map(|e| {
                        (0..e.attributes.len())
                            .map(|i| XNode::Attr(id, i))
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => Vec::new(),
            },
            Axis::Namespace => match n {
                XNode::Node(id) if doc.element(id).is_some() => {
                    let count = self.ns_nodes(id).len();
                    (0..count).map(|i| XNode::Ns(id, i)).collect()
                }
                _ => Vec::new(),
            },
            Axis::FollowingSibling | Axis::PrecedingSibling => match n {
                XNode::Node(id) => match doc.parent(id) {
                    Some(p) => {
                        let sib = doc.children(p);
                        let pos = sib.iter().position(|c| *c == id).unwrap_or(0);
                        if axis == Axis::FollowingSibling {
                            sib[pos + 1..].iter().map(|c| XNode::Node(*c)).collect()
                        } else {
                            sib[..pos].iter().rev().map(|c| XNode::Node(*c)).collect()
                        }
                    }
                    None => Vec::new(),
                },
                _ => Vec::new(),
            },
            Axis::Following => {
                // Desde un atributo o namespace, libxml2 (xmlXPathNextFollowing)
                // parte del elemento y omite sus descendientes.
                let anchor = match n {
                    XNode::Node(id) | XNode::Attr(id, _) | XNode::Ns(id, _) => id,
                };
                if !self.is_attached(anchor) {
                    Vec::new()
                } else {
                    let start = self.order[&anchor];
                    let skip_to = start + 1 + doc.descendants(anchor).len();
                    self.preorder[skip_to.min(self.preorder.len())..]
                        .iter()
                        .map(|c| XNode::Node(*c))
                        .collect()
                }
            }
            Axis::Preceding => {
                let anchor = match n {
                    XNode::Node(id) | XNode::Attr(id, _) | XNode::Ns(id, _) => id,
                };
                if !self.is_attached(anchor) {
                    Vec::new()
                } else {
                    let mut ancestors = Vec::new();
                    let mut cur = doc.parent(anchor);
                    while let Some(p) = cur {
                        ancestors.push(p);
                        cur = doc.parent(p);
                    }
                    let end = self.order[&anchor];
                    self.preorder[..end]
                        .iter()
                        .rev()
                        .filter(|c| !ancestors.contains(c))
                        .map(|c| XNode::Node(*c))
                        .collect()
                }
            }
        };
        self.tick(out.len())?;
        Ok(out)
    }

    fn matches(&mut self, n: XNode, axis: Axis, test: &Test) -> bool {
        let doc = self.doc;
        let principal_attr = axis == Axis::Attribute;
        let principal_ns = axis == Axis::Namespace;
        match test {
            Test::Node => true,
            Test::Text => matches!(
                n,
                XNode::Node(id) if matches!(doc.kind(id), NodeKind::Text(_) | NodeKind::CData(_))
            ),
            Test::Comment => {
                matches!(n, XNode::Node(id) if matches!(doc.kind(id), NodeKind::Comment(_)))
            }
            Test::Pi(target) => match n {
                XNode::Node(id) => match doc.kind(id) {
                    NodeKind::ProcessingInstruction { target: t, .. } => {
                        target.as_deref().is_none_or(|x| x == t)
                    }
                    _ => false,
                },
                _ => false,
            },
            Test::Any => match n {
                XNode::Attr(..) => principal_attr,
                XNode::Ns(..) => principal_ns,
                XNode::Node(id) => !principal_attr && !principal_ns && doc.element(id).is_some(),
            },
            Test::AnyInNs(uri) => match n {
                XNode::Attr(id, i) if principal_attr => doc
                    .element(id)
                    .is_some_and(|e| e.attributes[i].namespace.as_deref() == Some(uri.as_str())),
                XNode::Node(id) if !principal_attr && !principal_ns => doc
                    .element(id)
                    .is_some_and(|e| e.namespace.as_deref() == Some(uri.as_str())),
                _ => false,
            },
            Test::Name(ns, local) => match n {
                XNode::Attr(id, i) if principal_attr => doc.element(id).is_some_and(|e| {
                    let a = &e.attributes[i];
                    a.local == *local && a.namespace == *ns
                }),
                XNode::Ns(id, i) if principal_ns => {
                    ns.is_none() && self.ns_nodes(id)[i].prefix.as_deref() == Some(local.as_str())
                }
                XNode::Node(id) if !principal_attr && !principal_ns => doc
                    .element(id)
                    .is_some_and(|e| e.local == *local && e.namespace == *ns),
                _ => false,
            },
        }
    }

    fn apply_preds(&mut self, mut nodes: Vec<XNode>, preds: &[Expr]) -> Result<Vec<XNode>, Error> {
        for pred in preds {
            let size = nodes.len();
            let mut kept = Vec::new();
            for (i, n) in nodes.iter().enumerate() {
                let ctx = Ctx {
                    node: *n,
                    pos: i + 1,
                    size,
                };
                let keep = match self.eval(pred, ctx)? {
                    #[allow(clippy::float_cmp, reason = "posición exacta de XPath")]
                    Value::Num(x) => x == (i + 1) as f64,
                    other => self.boolean(&other),
                };
                if keep {
                    kept.push(*n);
                }
            }
            nodes = kept;
        }
        Ok(nodes)
    }

    fn steps(&mut self, start: Vec<XNode>, steps: &[Step]) -> Result<Vec<XNode>, Error> {
        let mut current = start;
        for step in steps {
            let mut next = Vec::new();
            for n in &current {
                let candidates = self.axis(*n, step.axis)?;
                let filtered: Vec<XNode> = candidates
                    .into_iter()
                    .filter(|c| self.matches(*c, step.axis, &step.test))
                    .collect();
                next.extend(self.apply_preds(filtered, &step.preds)?);
            }
            self.sort(&mut next);
            current = next;
        }
        Ok(current)
    }

    fn eval(&mut self, e: &Expr, ctx: Ctx) -> Result<Value, Error> {
        Ok(match e {
            Expr::Literal(s) => Value::Str(s.clone()),
            Expr::Number(n) => Value::Num(*n),
            Expr::Neg(x) => Value::Num(-self.eval_num(x, ctx)?),
            Expr::Union(a, b) => {
                let (Value::Nodes(mut x), Value::Nodes(y)) =
                    (self.eval(a, ctx)?, self.eval(b, ctx)?)
                else {
                    return Err(err("la unión exige conjuntos de nodos"));
                };
                x.extend(y);
                self.sort(&mut x);
                Value::Nodes(x)
            }
            Expr::Path(absolute, steps) => {
                let start = if *absolute {
                    vec![XNode::Node(self.doc.document_node())]
                } else {
                    vec![ctx.node]
                };
                Value::Nodes(self.steps(start, steps)?)
            }
            Expr::Filter(primary, preds, steps) => {
                let Value::Nodes(mut nodes) = self.eval(primary, ctx)? else {
                    return Err(err("el filtro exige un conjunto de nodos"));
                };
                self.sort(&mut nodes);
                let nodes = self.apply_preds(nodes, preds)?;
                Value::Nodes(self.steps(nodes, steps)?)
            }
            Expr::Binary(op, a, b) => self.binary(*op, a, b, ctx)?,
            Expr::Call(name, args) => self.call(name, args, ctx)?,
        })
    }

    fn eval_num(&mut self, e: &Expr, ctx: Ctx) -> Result<f64, Error> {
        let v = self.eval(e, ctx)?;
        Ok(self.number(&v))
    }

    fn binary(&mut self, op: Op, a: &Expr, b: &Expr, ctx: Ctx) -> Result<Value, Error> {
        match op {
            Op::Or => {
                let l = self.eval(a, ctx)?;
                if self.boolean(&l) {
                    return Ok(Value::Bool(true));
                }
                let r = self.eval(b, ctx)?;
                Ok(Value::Bool(self.boolean(&r)))
            }
            Op::And => {
                let l = self.eval(a, ctx)?;
                if !self.boolean(&l) {
                    return Ok(Value::Bool(false));
                }
                let r = self.eval(b, ctx)?;
                Ok(Value::Bool(self.boolean(&r)))
            }
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod => {
                let (x, y) = (self.eval_num(a, ctx)?, self.eval_num(b, ctx)?);
                Ok(Value::Num(match op {
                    Op::Add => x + y,
                    Op::Sub => x - y,
                    Op::Mul => x * y,
                    Op::Div => x / y,
                    _ => x % y,
                }))
            }
            _ => {
                let (l, r) = (self.eval(a, ctx)?, self.eval(b, ctx)?);
                Ok(Value::Bool(self.compare(op, &l, &r)))
            }
        }
    }

    fn compare(&mut self, op: Op, l: &Value, r: &Value) -> bool {
        match (l, r) {
            (Value::Nodes(a), Value::Nodes(b)) => {
                let bs: Vec<String> = b.iter().map(|n| self.string_value(*n)).collect();
                a.iter().any(|x| {
                    let xs = self.string_value(*x);
                    bs.iter().any(|ys| {
                        self.cmp_atoms(op, &Value::Str(xs.clone()), &Value::Str(ys.clone()))
                    })
                })
            }
            (Value::Nodes(a), other) | (other, Value::Nodes(a)) => {
                let swapped = matches!(r, Value::Nodes(_)) && !matches!(l, Value::Nodes(_));
                if let Value::Bool(bv) = other {
                    let nb = !a.is_empty();
                    let (x, y) = if swapped { (*bv, nb) } else { (nb, *bv) };
                    return self.cmp_atoms(op, &Value::Bool(x), &Value::Bool(y));
                }
                let nodes = a.clone();
                nodes.iter().any(|n| {
                    let sv = self.string_value(*n);
                    let nv = match other {
                        Value::Num(_) => Value::Num(str_to_num(&sv)),
                        _ => Value::Str(sv),
                    };
                    if swapped {
                        self.cmp_atoms(op, other, &nv)
                    } else {
                        self.cmp_atoms(op, &nv, other)
                    }
                })
            }
            _ => self.cmp_atoms(op, l, r),
        }
    }

    /// Comparación de valores que no son conjuntos de nodos.
    #[allow(clippy::float_cmp, reason = "igualdad numérica de XPath")]
    fn cmp_atoms(&self, op: Op, l: &Value, r: &Value) -> bool {
        match op {
            Op::Eq | Op::Neq => {
                let eq = if matches!(l, Value::Bool(_)) || matches!(r, Value::Bool(_)) {
                    self.boolean(l) == self.boolean(r)
                } else if matches!(l, Value::Num(_)) || matches!(r, Value::Num(_)) {
                    self.number(l) == self.number(r)
                } else {
                    self.string(l) == self.string(r)
                };
                if op == Op::Eq { eq } else { !eq }
            }
            _ => {
                let (x, y) = (self.number(l), self.number(r));
                match op {
                    Op::Lt => x < y,
                    Op::Le => x <= y,
                    Op::Gt => x > y,
                    _ => x >= y,
                }
            }
        }
    }

    fn boolean(&self, v: &Value) -> bool {
        match v {
            Value::Nodes(n) => !n.is_empty(),
            Value::Bool(b) => *b,
            Value::Num(x) => *x != 0.0 && !x.is_nan(),
            Value::Str(s) => !s.is_empty(),
        }
    }

    fn number(&self, v: &Value) -> f64 {
        match v {
            Value::Num(x) => *x,
            Value::Bool(b) => f64::from(u8::from(*b)),
            Value::Str(s) => str_to_num(s),
            Value::Nodes(n) => {
                let first = n.iter().min_by_key(|x| self.key(**x)).copied();
                first.map_or(f64::NAN, |f| str_to_num(&self.string_value(f)))
            }
        }
    }

    fn string(&self, v: &Value) -> String {
        match v {
            Value::Str(s) => s.clone(),
            Value::Bool(b) => if *b { "true" } else { "false" }.to_owned(),
            Value::Num(x) => format_number(*x),
            Value::Nodes(n) => n
                .iter()
                .min_by_key(|x| self.key(**x))
                .map(|f| self.string_value(*f))
                .unwrap_or_default(),
        }
    }

    fn string_value(&self, n: XNode) -> String {
        let doc = self.doc;
        match n {
            XNode::Node(id) => match doc.kind(id) {
                NodeKind::Text(t) | NodeKind::CData(t) | NodeKind::Comment(t) => t.clone(),
                NodeKind::ProcessingInstruction { data, .. } => data.clone(),
                NodeKind::Element(_) | NodeKind::Document => doc.text_content(id),
            },
            XNode::Attr(id, i) => doc
                .element(id)
                .map(|e| e.attributes[i].value.clone())
                .unwrap_or_default(),
            XNode::Ns(id, i) => doc
                .namespace_nodes(id)
                .get(i)
                .map(|d| d.uri.clone())
                .unwrap_or_default(),
        }
    }

    /// (`local-name`, `namespace-uri`, `name`) de un nodo.
    fn names(&self, n: XNode) -> (String, String, String) {
        let doc = self.doc;
        match n {
            XNode::Node(id) => match doc.kind(id) {
                NodeKind::Element(e) => (
                    e.local.clone(),
                    e.namespace.clone().unwrap_or_default(),
                    e.qname(),
                ),
                NodeKind::ProcessingInstruction { target, .. } => {
                    (target.clone(), String::new(), target.clone())
                }
                _ => (String::new(), String::new(), String::new()),
            },
            XNode::Attr(id, i) => doc
                .element(id)
                .map(|e| {
                    let a = &e.attributes[i];
                    (
                        a.local.clone(),
                        a.namespace.clone().unwrap_or_default(),
                        a.qname(),
                    )
                })
                .unwrap_or_default(),
            XNode::Ns(id, i) => {
                let p = doc
                    .namespace_nodes(id)
                    .get(i)
                    .and_then(|d| d.prefix.clone())
                    .unwrap_or_default();
                (p.clone(), String::new(), p)
            }
        }
    }

    fn nodes_arg(&mut self, e: &Expr, ctx: Ctx) -> Result<Vec<XNode>, Error> {
        match self.eval(e, ctx)? {
            Value::Nodes(n) => Ok(n),
            _ => Err(err("se esperaba un conjunto de nodos")),
        }
    }

    fn str_arg(&mut self, args: &[Expr], i: usize, ctx: Ctx) -> Result<String, Error> {
        match args.get(i) {
            Some(e) => {
                let v = self.eval(e, ctx)?;
                Ok(self.string(&v))
            }
            None => Ok(self.string_value(ctx.node)),
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], ctx: Ctx) -> Result<Value, Error> {
        let arity = |min: usize, max: usize| -> Result<(), Error> {
            if args.len() < min || args.len() > max {
                Err(err(format!("número de argumentos inválido para {name}()")))
            } else {
                Ok(())
            }
        };
        Ok(match name {
            "last" => {
                arity(0, 0)?;
                Value::Num(ctx.size as f64)
            }
            "position" => {
                arity(0, 0)?;
                Value::Num(ctx.pos as f64)
            }
            "count" => {
                arity(1, 1)?;
                Value::Num(self.nodes_arg(&args[0], ctx)?.len() as f64)
            }
            "local-name" | "namespace-uri" | "name" => {
                arity(0, 1)?;
                let node = match args.first() {
                    Some(a) => {
                        let nodes = self.nodes_arg(a, ctx)?;
                        nodes.iter().min_by_key(|x| self.key(**x)).copied()
                    }
                    None => Some(ctx.node),
                };
                let (local, ns, qname) = node.map(|n| self.names(n)).unwrap_or_default();
                Value::Str(match name {
                    "local-name" => local,
                    "namespace-uri" => ns,
                    _ => qname,
                })
            }
            "string" => {
                arity(0, 1)?;
                Value::Str(self.str_arg(args, 0, ctx)?)
            }
            "concat" => {
                if args.len() < 2 {
                    return Err(err("concat() exige al menos dos argumentos"));
                }
                let mut out = String::new();
                for i in 0..args.len() {
                    out.push_str(&self.str_arg(args, i, ctx)?);
                }
                Value::Str(out)
            }
            "starts-with" | "contains" | "substring-before" | "substring-after" => {
                arity(2, 2)?;
                let (s, t) = (self.str_arg(args, 0, ctx)?, self.str_arg(args, 1, ctx)?);
                match name {
                    "starts-with" => Value::Bool(s.starts_with(&t)),
                    "contains" => Value::Bool(s.contains(&t)),
                    "substring-before" => {
                        Value::Str(s.find(&t).map(|i| s[..i].to_owned()).unwrap_or_default())
                    }
                    _ => Value::Str(
                        s.find(&t)
                            .map(|i| s[i + t.len()..].to_owned())
                            .unwrap_or_default(),
                    ),
                }
            }
            "substring" => {
                arity(2, 3)?;
                let s = self.str_arg(args, 0, ctx)?;
                let start = xpath_round(self.eval_num(&args[1], ctx)?);
                let end = match args.get(2) {
                    Some(l) => start + xpath_round(self.eval_num(l, ctx)?),
                    None => f64::INFINITY,
                };
                Value::Str(
                    s.chars()
                        .enumerate()
                        .filter(|(i, _)| {
                            let p = (*i + 1) as f64;
                            p >= start && p < end
                        })
                        .map(|(_, c)| c)
                        .collect(),
                )
            }
            "string-length" => {
                arity(0, 1)?;
                Value::Num(self.str_arg(args, 0, ctx)?.chars().count() as f64)
            }
            "normalize-space" => {
                arity(0, 1)?;
                let s = self.str_arg(args, 0, ctx)?;
                Value::Str(
                    s.split([' ', '\t', '\n', '\r'])
                        .filter(|p| !p.is_empty())
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            }
            "translate" => {
                arity(3, 3)?;
                let s = self.str_arg(args, 0, ctx)?;
                let from: Vec<char> = self.str_arg(args, 1, ctx)?.chars().collect();
                let to: Vec<char> = self.str_arg(args, 2, ctx)?.chars().collect();
                Value::Str(
                    s.chars()
                        .filter_map(|c| match from.iter().position(|f| *f == c) {
                            Some(i) => to.get(i).copied(),
                            None => Some(c),
                        })
                        .collect(),
                )
            }
            "boolean" => {
                arity(1, 1)?;
                let v = self.eval(&args[0], ctx)?;
                Value::Bool(self.boolean(&v))
            }
            "not" => {
                arity(1, 1)?;
                let v = self.eval(&args[0], ctx)?;
                Value::Bool(!self.boolean(&v))
            }
            "true" => {
                arity(0, 0)?;
                Value::Bool(true)
            }
            "false" => {
                arity(0, 0)?;
                Value::Bool(false)
            }
            "lang" => {
                arity(1, 1)?;
                let wanted = self.str_arg(args, 0, ctx)?.to_ascii_lowercase();
                // xmlNodeGetLang: desde el nodo (o el elemento del atributo)
                // hacia arriba; libxml2 no lo resuelve desde un namespace.
                let mut cur = match ctx.node {
                    XNode::Ns(..) => None,
                    XNode::Node(id) | XNode::Attr(id, _) => Some(id),
                };
                let mut lang = None;
                while let Some(e) = cur {
                    if let Some(l) = self.doc.attribute_ns(e, Some(XML_NS), "lang") {
                        lang = Some(l.to_ascii_lowercase());
                        break;
                    }
                    cur = self.doc.parent(e);
                }
                Value::Bool(lang.is_some_and(|l| {
                    l == wanted || (l.starts_with(&wanted) && l[wanted.len()..].starts_with('-'))
                }))
            }
            "number" => {
                arity(0, 1)?;
                match args.first() {
                    Some(a) => Value::Num(self.eval_num(a, ctx)?),
                    None => Value::Num(str_to_num(&self.string_value(ctx.node))),
                }
            }
            "sum" => {
                arity(1, 1)?;
                let nodes = self.nodes_arg(&args[0], ctx)?;
                Value::Num(
                    nodes
                        .iter()
                        .map(|n| str_to_num(&self.string_value(*n)))
                        .sum(),
                )
            }
            "floor" | "ceiling" | "round" => {
                arity(1, 1)?;
                let x = self.eval_num(&args[0], ctx)?;
                Value::Num(match name {
                    "floor" => x.floor(),
                    "ceiling" => x.ceil(),
                    _ => xpath_round(x),
                })
            }
            "id" => {
                arity(1, 1)?;
                let v = self.eval(&args[0], ctx)?;
                let text = match &v {
                    Value::Nodes(n) => n
                        .iter()
                        .map(|x| self.string_value(*x))
                        .collect::<Vec<_>>()
                        .join(" "),
                    other => self.string(other),
                };
                let ids: Vec<&str> = text
                    .split([' ', '\t', '\n', '\r'])
                    .filter(|t| !t.is_empty())
                    .collect();
                let doc = self.doc;
                let mut found: Vec<XNode> = self
                    .preorder
                    .iter()
                    .filter(|n| {
                        doc.attribute_ns(**n, Some(XML_NS), "id")
                            .is_some_and(|v| ids.contains(&v))
                    })
                    .map(|n| XNode::Node(*n))
                    .collect();
                self.sort(&mut found);
                Value::Nodes(found)
            }
            other => return Err(err(format!("función no registrada {other}()"))),
        })
    }
}

/// `round()` de XPath: `floor(x + 0.5)`, con `-0` en `[-0.5, 0)`.
fn xpath_round(x: f64) -> f64 {
    if x.is_nan() || x.is_infinite() {
        return x;
    }
    if (-0.5..0.5).contains(&x) {
        return x * 0.0;
    }
    (x + 0.5).floor()
}

/// `number(cadena)` de libxml2: espacios alrededor, signo `-` opcional,
/// dígitos con punto y exponente opcional; otra cosa es `NaN`.
pub(crate) fn str_to_num(s: &str) -> f64 {
    let t = s.trim_matches([' ', '\t', '\n', '\r']);
    let body = t.strip_prefix('-').unwrap_or(t);
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut digits = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return f64::NAN;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j == exp_start {
            return f64::NAN;
        }
        i = j;
    }
    if i != bytes.len() {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// `xmlXPathFormatNumber`: enteros sin decimales; fuera de `[1e-5, 1e9]`
/// notación científica con 15 cifras; si no, 15 cifras significativas; sin
/// ceros finales.
#[must_use]
pub(crate) fn format_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    }
    if x == 0.0 {
        return "0".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if x > f64::from(i32::MIN) && x < f64::from(i32::MAX) && x.fract() == 0.0 {
        return format!("{x:.0}");
    }
    let abs = x.abs();
    let mut work = if !(1e-5..=1e9).contains(&abs) {
        // "%*.*e" con 14 decimales; Rust da "1.23e19", C "1.23e+19".
        let s = format!("{x:.14e}");
        let (mantissa, exp) = s.split_once('e').unwrap_or((s.as_str(), "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        let mut c = String::from(mantissa);
        let _ = write!(c, "e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
        c
    } else {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "(int)log10 de libxml2, acotado por el rango anterior"
        )]
        let integer_place = abs.log10() as i32;
        let fraction = if integer_place > 0 {
            15 - integer_place - 1
        } else {
            15 - integer_place
        };
        format!("{x:.*}", usize::try_from(fraction).unwrap_or(0))
    };
    // Quitar ceros finales de la parte fraccionaria (antes de la `e`).
    let (number, exponent) = match work.find('e') {
        Some(i) => (work[..i].to_owned(), work[i..].to_owned()),
        None => (work.clone(), String::new()),
    };
    if number.contains('.') {
        let trimmed = number.trim_end_matches('0');
        let trimmed = trimmed.strip_suffix('.').unwrap_or(trimmed);
        work = format!("{trimmed}{exponent}");
    }
    work
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format_matches_libxml() {
        for (x, s) in [
            (1.0 / 3.0, "0.333333333333333"),
            (1e20, "1e+20"),
            (0.1 + 0.2, "0.3"),
            (-0.0, "0"),
            (1000.0, "1000"),
            (12_345_678_901_234_567_890.5, "1.23456789012346e+19"),
            (0.000_001, "1e-06"),
            (f64::INFINITY, "Infinity"),
            (f64::NAN, "NaN"),
            (-1.0, "-1"),
        ] {
            assert_eq!(format_number(x), s, "{x}");
        }
        assert_eq!(str_to_num("  12 "), 12.0);
        assert_eq!(str_to_num("1e3"), 1000.0);
        assert!(str_to_num("+1").is_nan());
        assert!(xpath_round(-0.5).is_sign_negative());
    }

    #[test]
    fn evaluates_paths_and_functions() {
        let doc = Document::parse("<r xmlns:a='ua'><a:x k='1'>t</a:x><y/><!--c--></r>").unwrap();
        let mut ns = HashMap::new();
        ns.insert("p".to_owned(), "ua".to_owned());
        let mut ev = Evaluator::new(&doc);
        let root = doc.document_node();
        assert_eq!(ev.select("//p:x", root, &ns).unwrap().len(), 1);
        assert_eq!(
            ev.select("//*[position() = last()]", root, &ns)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(ev.select("//@k[. = 1]", root, &ns).unwrap().len(), 1);
        assert_eq!(ev.select("//namespace::*", root, &ns).unwrap().len(), 6);
        assert_eq!(
            ev.select("(//node())[self::comment()]", root, &ns)
                .unwrap()
                .len(),
            1
        );
        assert!(ev.select("//q:x", root, &ns).is_err());
        assert!(ev.select("count(//*)", root, &ns).is_err());
        assert!(ev.select("not(", root, &ns).is_err());
    }
}
