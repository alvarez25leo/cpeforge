//! Árbol XML mutable con la semántica observable del DOM sobre libxml2.
//!
//! - Análisis seguro: se rechazan DTD y entidades no predefinidas (no hay XXE
//!   ni expansión de entidades), con límites de tamaño, profundidad y nodos.
//! - Serialización igual a `saveXML()` de libxml2 sin formato:
//!   declaración, `<a/>` para elementos vacíos, escapes y saltos de línea entre
//!   nodos de primer nivel.
//! - Entrada como bytes: UTF-8 o la codificación de un byte declarada
//!   (ISO-8859-1, US-ASCII, windows-1252, ISO-8859-15). Un `&str` se trata
//!   como sus bytes UTF-8.
//! - Mutaciones con la reconciliación de namespaces del DOM al insertar un
//!   nodo (`dom_reconcile_ns`): se eliminan declaraciones redundantes.

use std::fmt::Write as _;

use quick_xml::events::{BytesStart, Event};

use crate::encoding::{self, Charset};
use crate::error::XmlError;

/// Namespace de los atributos `xml:*`.
pub const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// Identificador de un nodo dentro de un [`Document`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

/// Declaración de namespace (`xmlns` o `xmlns:p`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct NsDecl {
    /// Prefijo; `None` para el namespace por defecto.
    pub prefix: Option<String>,
    /// URI declarada (vacía para `xmlns=""`).
    pub uri: String,
}

/// Atributo de un elemento.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Attribute {
    /// Prefijo tal como se escribió.
    pub prefix: Option<String>,
    /// Nombre local.
    pub local: String,
    /// Namespace resuelto (`None` si no tiene).
    pub namespace: Option<String>,
    /// Valor ya decodificado y normalizado.
    pub value: String,
}

impl Attribute {
    /// Nombre calificado (`prefijo:local`).
    #[must_use]
    pub fn qname(&self) -> String {
        qualified(self.prefix.as_deref(), &self.local)
    }

    /// Escribe el nombre calificado sin asignar una cadena intermedia.
    pub(crate) fn push_qname(&self, out: &mut String) {
        push_qualified(out, self.prefix.as_deref(), &self.local);
    }
}

/// Datos de un elemento.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Element {
    /// Prefijo tal como se escribió.
    pub prefix: Option<String>,
    /// Nombre local.
    pub local: String,
    /// Namespace resuelto.
    pub namespace: Option<String>,
    /// Declaraciones de namespace en el orden de serialización.
    pub ns_decls: Vec<NsDecl>,
    /// Atributos en orden de documento.
    pub attributes: Vec<Attribute>,
}

impl Element {
    /// Nombre calificado (`prefijo:local`).
    #[must_use]
    pub fn qname(&self) -> String {
        qualified(self.prefix.as_deref(), &self.local)
    }

    /// Escribe el nombre calificado sin asignar una cadena intermedia.
    pub(crate) fn push_qname(&self, out: &mut String) {
        push_qualified(out, self.prefix.as_deref(), &self.local);
    }
}

/// Tipo y contenido de un nodo.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeKind {
    /// Nodo documento (raíz del árbol).
    Document,
    /// Elemento.
    Element(Element),
    /// Texto.
    Text(String),
    /// Sección CDATA.
    CData(String),
    /// Comentario.
    Comment(String),
    /// Instrucción de procesamiento.
    ProcessingInstruction {
        /// Destino.
        target: String,
        /// Datos (puede ser vacío).
        data: String,
    },
}

#[derive(Debug, Clone)]
struct NodeData {
    kind: NodeKind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

/// Declaración XML del documento.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct XmlDeclaration {
    /// Versión (`1.0`).
    pub version: String,
    /// Codificación tal como se escribió (`utf-8`, `UTF-8`, …).
    pub encoding: Option<String>,
    /// `standalone`.
    pub standalone: Option<String>,
}

impl XmlDeclaration {
    /// Declaración con versión, codificación y `standalone` (para asignarla a
    /// [`Document::declaration`]).
    #[must_use]
    pub fn new(
        version: impl Into<String>,
        encoding: Option<String>,
        standalone: Option<String>,
    ) -> Self {
        Self {
            version: version.into(),
            encoding,
            standalone,
        }
    }
}

/// Límites del analizador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseOptions {
    /// Tamaño máximo de entrada en bytes (10 MiB por defecto).
    pub max_bytes: usize,
    /// Profundidad máxima de elementos (256, como libxml2 sin `XML_PARSE_HUGE`).
    pub max_depth: usize,
    /// Número máximo de nodos.
    pub max_nodes: usize,
    /// Conserva los textos formados solo por espacios (`preserveWhiteSpace`).
    pub preserve_whitespace: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024,
            max_depth: 256,
            max_nodes: 1_000_000,
            preserve_whitespace: true,
        }
    }
}

/// Documento XML.
#[derive(Debug, Clone)]
pub struct Document {
    nodes: Vec<NodeData>,
    /// Declaración XML; `None` si la entrada no tenía.
    pub declaration: Option<XmlDeclaration>,
    /// Codificación con que se decodificó la entrada, mientras se analiza.
    input_charset: Charset,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

fn qualified(prefix: Option<&str>, local: &str) -> String {
    match prefix {
        Some(p) if !p.is_empty() => format!("{p}:{local}"),
        _ => local.to_owned(),
    }
}

/// Conserva la primera declaración de cada prefijo, en el orden recibido.
/// Lineal en el número de declaraciones (un documento puede declarar miles).
fn first_per_prefix(decls: Vec<&NsDecl>) -> Vec<&NsDecl> {
    if decls.len() <= 16 {
        let mut out: Vec<&NsDecl> = Vec::with_capacity(decls.len());
        for d in decls {
            if !out.iter().any(|x| x.prefix == d.prefix) {
                out.push(d);
            }
        }
        return out;
    }
    let mut seen = std::collections::HashSet::with_capacity(decls.len());
    decls
        .into_iter()
        .filter(|d| seen.insert(d.prefix.as_deref()))
        .collect()
}

/// De declaraciones ordenadas de la más externa a la más interna, conserva
/// la última de cada prefijo en el orden de esa última aparición.
fn retain_last_per_prefix(decls: Vec<&NsDecl>) -> Vec<&NsDecl> {
    let mut out = first_per_prefix(decls.into_iter().rev().collect());
    out.reverse();
    out
}

fn push_qualified(out: &mut String, prefix: Option<&str>, local: &str) {
    if let Some(p) = prefix.filter(|p| !p.is_empty()) {
        out.push_str(p);
        out.push(':');
    }
    out.push_str(local);
}

fn split_qname(qname: &str) -> (Option<String>, String) {
    match qname.split_once(':') {
        Some((p, l)) => (Some(p.to_owned()), l.to_owned()),
        None => (None, qname.to_owned()),
    }
}

fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

fn check_chars(text: &str) -> Result<(), XmlError> {
    match text.chars().find(|c| !is_xml_char(*c)) {
        Some(c) => Err(XmlError::Malformed(format!(
            "carácter no permitido U+{:04X}",
            u32::from(c)
        ))),
        None => Ok(()),
    }
}

/// Resuelve una referencia (`amp`, `#10`, `#x41`); no hay entidades definidas por DTD.
fn resolve_reference(name: &str) -> Result<char, XmlError> {
    let resolved = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            // Solo dígitos: `from_str_radix`/`parse` aceptan un `+` inicial
            // que XML no admite (`&#+65;`).
            let code = if let Some(hex) = name.strip_prefix("#x") {
                hex.bytes()
                    .all(|b| b.is_ascii_hexdigit())
                    .then(|| u32::from_str_radix(hex, 16).ok())
                    .flatten()
            } else if let Some(dec) = name.strip_prefix('#') {
                dec.bytes()
                    .all(|b| b.is_ascii_digit())
                    .then(|| dec.parse::<u32>().ok())
                    .flatten()
            } else {
                return Err(XmlError::UndefinedEntity(name.to_owned()));
            };
            code.and_then(char::from_u32).filter(|c| is_xml_char(*c))
        }
    };
    resolved.ok_or_else(|| XmlError::Malformed(format!("referencia inválida &{name};")))
}

/// Decodifica y normaliza el valor de un atributo (XML 1.0 §3.3.3).
/// Nombre calificado válido para Namespaces in XML: a lo sumo un `:`, con
/// prefijo y nombre local no vacíos.
fn checked_qname(name: &str) -> Result<&str, XmlError> {
    let valid = match name.split_once(':') {
        Some((p, l)) => !p.is_empty() && !l.is_empty() && !l.contains(':'),
        None => !name.is_empty(),
    };
    if valid {
        Ok(name)
    } else {
        Err(XmlError::Malformed(format!(
            "nombre calificado inválido '{name}'"
        )))
    }
}

/// Texto de `createElementNS($ns, $nombre, $valor)`: libxml2 interpreta las
/// referencias del valor (`xmlNodeParseContent`). Caracterizado con libxml2 2.15:
///
/// - entidades predefinidas y referencias de carácter válidas se sustituyen;
/// - una referencia de carácter mal formada se descarta hasta el primer
///   carácter inválido (`&#65 b` → ` b`);
/// - `&nombre;` desconocido (el nombre llega hasta el siguiente `;`, sea cual
///   sea su contenido) crea una referencia sin texto;
/// - un `&` sin `;` posterior se descarta y el resto se copia literal.
#[must_use]
pub fn parse_node_content(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        if let Some(digits) = after.strip_prefix('#') {
            let (hex, body) = match digits.strip_prefix('x') {
                Some(b) => (true, b),
                None => (false, digits),
            };
            let mut value: u32 = 0;
            let mut ok = true;
            let mut consumed = body.len();
            for (i, c) in body.char_indices() {
                if c == ';' {
                    consumed = i + 1;
                    break;
                }
                match c.to_digit(if hex { 16 } else { 10 }) {
                    Some(d) => {
                        value = value
                            .saturating_mul(if hex { 16 } else { 10 })
                            .saturating_add(d)
                    }
                    None => {
                        ok = false;
                        consumed = i;
                        break;
                    }
                }
            }
            if consumed == body.len() && !body.contains(';') {
                ok = false;
            }
            if ok {
                if let Some(c) = char::from_u32(value).filter(|c| is_xml_char(*c)) {
                    out.push(c);
                }
            }
            rest = &body[consumed..];
            continue;
        }
        match after.find(';') {
            None => {
                out.push_str(after);
                return out;
            }
            Some(end) => {
                match &after[..end] {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "apos" => out.push('\''),
                    _ => {}
                }
                rest = &after[end + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// La entrada es `&str`, así que los bytes que entrega quick-xml son UTF-8.
fn utf8(bytes: &[u8]) -> Result<&str, XmlError> {
    std::str::from_utf8(bytes).map_err(|e| XmlError::Malformed(e.to_string()))
}

fn decode_attribute(raw: &str) -> Result<String, XmlError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(pos) = rest.find(['&', '<', '\t', '\n']) {
        out.push_str(&rest[..pos]);
        let c = rest.as_bytes()[pos];
        match c {
            b'<' => return Err(XmlError::Malformed("'<' en el valor de un atributo".into())),
            b'\t' | b'\n' => {
                out.push(' ');
                rest = &rest[pos + 1..];
            }
            _ => {
                let end = rest[pos..]
                    .find(';')
                    .ok_or_else(|| XmlError::Malformed("referencia sin ';'".into()))?;
                out.push(resolve_reference(&rest[pos + 1..pos + end])?);
                rest = &rest[pos + end + 1..];
            }
        }
    }
    out.push_str(rest);
    check_chars(&out)?;
    Ok(out)
}

/// Normaliza los finales de línea (XML 1.0 §2.11).
fn normalize_eol(input: &str) -> std::borrow::Cow<'_, str> {
    if input.contains('\r') {
        std::borrow::Cow::Owned(input.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        std::borrow::Cow::Borrowed(input)
    }
}

struct Scope {
    decls: Vec<(Option<String>, String)>,
    /// Declaraciones en ámbito en este nivel (propias y de los ancestros).
    in_scope: usize,
}

/// Presupuesto de trabajo de namespaces por documento: suma, por elemento,
/// de las declaraciones en ámbito. La resolución de prefijos y la
/// canonicalización inclusiva recorren ese conjunto en cada elemento, de modo
/// que miles de declaraciones en la raíz por miles de descendientes serían
/// cuadráticas. Un comprobante real queda órdenes de magnitud por debajo.
const MAX_NAMESPACE_WORK: usize = 1 << 24;

fn lookup_scope<'s>(scopes: &'s [Scope], prefix: Option<&str>) -> Option<&'s str> {
    if prefix == Some("xml") {
        return Some(XML_NS);
    }
    for scope in scopes.iter().rev() {
        for (p, uri) in scope.decls.iter().rev() {
            if p.as_deref() == prefix {
                return if uri.is_empty() { None } else { Some(uri) };
            }
        }
    }
    None
}

fn charge_namespace_work(work: usize, scopes: &[Scope]) -> Result<usize, XmlError> {
    let work = work.saturating_add(scopes.last().map_or(0, |s| s.in_scope));
    if work > MAX_NAMESPACE_WORK {
        return Err(XmlError::LimitExceeded("namespaces"));
    }
    Ok(work)
}

impl Document {
    /// Documento vacío (solo el nodo documento).
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![NodeData {
                kind: NodeKind::Document,
                parent: None,
                children: Vec::new(),
            }],
            declaration: None,
            input_charset: Charset::Utf8,
        }
    }

    /// Nodo documento.
    #[must_use]
    pub fn document_node(&self) -> NodeId {
        NodeId(0)
    }

    /// Analiza un documento con los límites por defecto. El texto se trata
    /// como sus bytes UTF-8 ([`Self::parse_bytes`]), igual que `loadXML` con
    /// una cadena de bytes.
    ///
    /// # Errors
    ///
    /// [`XmlError`] si el XML está mal formado, tiene DTD o excede los límites.
    pub fn parse(xml: &str) -> Result<Self, XmlError> {
        Self::parse_with(xml, ParseOptions::default())
    }

    /// Analiza un documento con límites explícitos (ver [`Self::parse`]).
    ///
    /// # Errors
    ///
    /// [`XmlError`] si el XML está mal formado, tiene DTD o excede los límites.
    pub fn parse_with(xml: &str, options: ParseOptions) -> Result<Self, XmlError> {
        Self::parse_bytes_with(xml.as_bytes(), options)
    }

    /// `loadXML($bytes)`: decodifica según la declaración (UTF-8 si no
    /// declara codificación) y analiza.
    ///
    /// # Errors
    ///
    /// [`XmlError::UnsupportedEncoding`] con una codificación no admitida,
    /// [`XmlError::Malformed`] con bytes inválidos para la codificación, y
    /// los demás errores de análisis.
    pub fn parse_bytes(xml: &[u8]) -> Result<Self, XmlError> {
        Self::parse_bytes_with(xml, ParseOptions::default())
    }

    /// [`Self::parse_bytes`] con límites explícitos.
    ///
    /// # Errors
    ///
    /// Los de [`Self::parse_bytes`].
    pub fn parse_bytes_with(xml: &[u8], options: ParseOptions) -> Result<Self, XmlError> {
        if xml.is_empty() {
            return Err(XmlError::Empty);
        }
        if xml.len() > options.max_bytes {
            return Err(XmlError::LimitExceeded("tamaño"));
        }
        let charset = match encoding::declared(xml) {
            Some(name) if !encoding::valid_name(&name) => {
                return Err(XmlError::Malformed(format!(
                    "nombre de codificación inválido: {name}"
                )));
            }
            Some(name) => Charset::from_name(&name).ok_or(XmlError::UnsupportedEncoding(name))?,
            None => Charset::Utf8,
        };
        let text = charset.decode(xml)?;
        let mut doc = Self::new();
        doc.input_charset = charset;
        let root = doc.document_node();
        doc.parse_into(&normalize_eol(&text), root, options, &mut Vec::new(), true)?;
        if doc.document_element().is_none() {
            return Err(XmlError::Malformed("documento sin elemento raíz".into()));
        }
        Ok(doc)
    }

    /// Analiza `xml` y agrega los nodos como hijos de `parent`.
    fn parse_into(
        &mut self,
        xml: &str,
        parent: NodeId,
        options: ParseOptions,
        scopes: &mut Vec<Scope>,
        document: bool,
    ) -> Result<(), XmlError> {
        let mut reader = quick_xml::Reader::from_str(xml);
        let config = reader.config_mut();
        config.check_end_names = true;
        config.check_comments = true;
        config.expand_empty_elements = false;
        config.trim_text(false);
        let base_depth = scopes.len();
        let mut stack: Vec<NodeId> = vec![parent];
        let mut seen_root = false;
        let mut first = true;
        let mut ns_work = 0usize;
        loop {
            let event = reader
                .read_event()
                .map_err(|err| XmlError::Malformed(err.to_string()))?;
            let at_top = document && stack.len() == 1;
            let is_first = std::mem::replace(&mut first, false);
            let current = *stack
                .last()
                .ok_or_else(|| XmlError::Malformed("pila vacía".into()))?;
            match event {
                Event::Decl(decl) => {
                    if !document || !is_first {
                        return Err(XmlError::Malformed(
                            "declaración XML fuera del inicio".into(),
                        ));
                    }
                    let version = utf8(
                        &decl
                            .version()
                            .map_err(|e| XmlError::Malformed(e.to_string()))?,
                    )?
                    .to_owned();
                    let encoding = decl
                        .encoding()
                        .transpose()
                        .map_err(|e| XmlError::Malformed(e.to_string()))?
                        .map(|e| utf8(&e).map(str::to_owned))
                        .transpose()?;
                    // La declaración debe coincidir con la codificación con
                    // que se decodificaron los bytes.
                    if let Some(enc) = &encoding {
                        if Charset::from_name(enc) != Some(self.input_charset) {
                            return Err(XmlError::UnsupportedEncoding(enc.clone()));
                        }
                    }
                    let standalone = decl
                        .standalone()
                        .transpose()
                        .map_err(|e| XmlError::Malformed(e.to_string()))?
                        .map(|s| utf8(&s).map(str::to_owned))
                        .transpose()?;
                    self.declaration = Some(XmlDeclaration {
                        version,
                        encoding,
                        standalone,
                    });
                }
                Event::DocType(_) => return Err(XmlError::Dtd),
                Event::Start(start) => {
                    if at_top && seen_root {
                        return Err(XmlError::Malformed("más de un elemento raíz".into()));
                    }
                    seen_root |= at_top;
                    if scopes.len() - base_depth + 1 > options.max_depth {
                        return Err(XmlError::LimitExceeded("profundidad"));
                    }
                    let id = self.parse_element(&start, scopes, options)?;
                    ns_work = charge_namespace_work(ns_work, scopes)?;
                    self.attach(current, id);
                    stack.push(id);
                }
                Event::Empty(start) => {
                    if at_top && seen_root {
                        return Err(XmlError::Malformed("más de un elemento raíz".into()));
                    }
                    seen_root |= at_top;
                    if scopes.len() - base_depth + 1 > options.max_depth {
                        return Err(XmlError::LimitExceeded("profundidad"));
                    }
                    let id = self.parse_element(&start, scopes, options)?;
                    ns_work = charge_namespace_work(ns_work, scopes)?;
                    scopes.pop();
                    self.attach(current, id);
                }
                Event::End(_) => {
                    if stack.len() == 1 {
                        return Err(XmlError::Malformed(
                            "etiqueta de cierre sin apertura".into(),
                        ));
                    }
                    stack.pop();
                    scopes.pop();
                }
                Event::Text(text) => {
                    let text: &str = utf8(&text)?;
                    if text.contains("]]>") {
                        return Err(XmlError::Malformed("']]>' en el contenido".into()));
                    }
                    check_chars(text)?;
                    if at_top {
                        if !text.chars().all(|c| matches!(c, ' ' | '\t' | '\n')) {
                            return Err(XmlError::Malformed(
                                "texto fuera del elemento raíz".into(),
                            ));
                        }
                    } else {
                        self.push_text(current, text, options)?;
                    }
                }
                Event::GeneralRef(reference) => {
                    if at_top {
                        return Err(XmlError::Malformed(
                            "referencia fuera del elemento raíz".into(),
                        ));
                    }
                    let c = resolve_reference(utf8(&reference)?)?;
                    self.push_text(current, &c.to_string(), options)?;
                }
                Event::CData(cdata) => {
                    if at_top {
                        return Err(XmlError::Malformed("CDATA fuera del elemento raíz".into()));
                    }
                    let text = utf8(&cdata.into_inner())?.to_owned();
                    check_chars(&text)?;
                    let id = self.alloc(NodeKind::CData(text), options)?;
                    self.attach(current, id);
                }
                Event::Comment(comment) => {
                    let text = utf8(&comment.into_inner())?.to_owned();
                    check_chars(&text)?;
                    let id = self.alloc(NodeKind::Comment(text), options)?;
                    self.attach(current, id);
                }
                Event::PI(pi) => {
                    let target = utf8(pi.target())?.to_owned();
                    if target.eq_ignore_ascii_case("xml") {
                        return Err(XmlError::Malformed("instrucción 'xml' reservada".into()));
                    }
                    let data = utf8(pi.content())?.trim_start().to_owned();
                    let id =
                        self.alloc(NodeKind::ProcessingInstruction { target, data }, options)?;
                    self.attach(current, id);
                }
                Event::Eof => break,
            }
        }
        if stack.len() != 1 {
            return Err(XmlError::Malformed("elementos sin cerrar".into()));
        }
        if !options.preserve_whitespace {
            self.remove_blank_text(parent);
        }
        Ok(())
    }

    fn parse_element(
        &mut self,
        start: &BytesStart<'_>,
        scopes: &mut Vec<Scope>,
        options: ParseOptions,
    ) -> Result<NodeId, XmlError> {
        let (prefix, local) = split_qname(checked_qname(utf8(start.name().0)?)?);
        let mut ns_decls = Vec::new();
        let mut raw_attrs = Vec::new();
        for attr in start.attributes().with_checks(true) {
            let attr = attr.map_err(|e| XmlError::Malformed(e.to_string()))?;
            let key = checked_qname(utf8(attr.key.0)?)?;
            let value = decode_attribute(utf8(&attr.value)?)?;
            if key == "xmlns" {
                ns_decls.push(NsDecl {
                    prefix: None,
                    uri: value,
                });
            } else if let Some(p) = key.strip_prefix("xmlns:") {
                if p == "xmlns" || (p == "xml" && value != XML_NS) || value.is_empty() {
                    return Err(XmlError::Malformed(format!(
                        "declaración de namespace inválida xmlns:{p}"
                    )));
                }
                ns_decls.push(NsDecl {
                    prefix: Some(p.to_owned()),
                    uri: value,
                });
            } else {
                raw_attrs.push((key.to_owned(), value));
            }
        }
        let in_scope = scopes.last().map_or(0, |s| s.in_scope) + ns_decls.len();
        scopes.push(Scope {
            decls: ns_decls
                .iter()
                .map(|d| (d.prefix.clone(), d.uri.clone()))
                .collect(),
            in_scope,
        });
        let namespace = lookup_scope(scopes, prefix.as_deref()).map(str::to_owned);
        // Namespaces in XML §5: un prefijo debe estar declarado (libxml2 lo
        // rechaza). Sin esto `@x` de XPath coincidía con `p:x` y la C14N emitía
        // prefijos sin declarar.
        if let (Some(p), None) = (prefix.as_deref(), &namespace) {
            return Err(XmlError::Malformed(format!("prefijo no declarado {p}")));
        }
        let mut attributes: Vec<Attribute> = Vec::with_capacity(raw_attrs.len());
        // Duplicados por (namespace, nombre local) en tiempo lineal: comparar
        // con todos los anteriores era cuadrático (160 000 atributos → 15 s).
        let mut seen: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::with_capacity(raw_attrs.len());
        for (key, value) in raw_attrs {
            let (p, l) = split_qname(&key);
            let namespace = p
                .as_deref()
                .and_then(|p| lookup_scope(scopes, Some(p)))
                .map(str::to_owned);
            if let (Some(p), None) = (p.as_deref(), &namespace) {
                return Err(XmlError::Malformed(format!("prefijo no declarado {p}")));
            }
            if let Some(ns) = &namespace {
                if !seen.insert((ns.clone(), l.clone())) {
                    return Err(XmlError::Malformed(format!("atributo duplicado {key}")));
                }
            }
            attributes.push(Attribute {
                prefix: p,
                local: l,
                namespace,
                value,
            });
        }
        self.alloc(
            NodeKind::Element(Element {
                prefix,
                local,
                namespace,
                ns_decls,
                attributes,
            }),
            options,
        )
    }

    fn push_text(
        &mut self,
        parent: NodeId,
        text: &str,
        options: ParseOptions,
    ) -> Result<(), XmlError> {
        if let Some(&last) = self.nodes[parent.0].children.last() {
            if let NodeKind::Text(existing) = &mut self.nodes[last.0].kind {
                existing.push_str(text);
                return Ok(());
            }
        }
        let id = self.alloc(NodeKind::Text(text.to_owned()), options)?;
        self.attach(parent, id);
        Ok(())
    }

    /// `preserveWhiteSpace = false`: elimina textos en blanco con hermanos.
    fn remove_blank_text(&mut self, node: NodeId) {
        let children = self.nodes[node.0].children.clone();
        let only_child = children.len() == 1;
        for child in children {
            let blank = matches!(&self.nodes[child.0].kind, NodeKind::Text(t) if t.chars().all(|c| matches!(c, ' ' | '\t' | '\n')));
            if blank && !only_child {
                self.detach(child);
            } else {
                self.remove_blank_text(child);
            }
        }
    }

    fn alloc(&mut self, kind: NodeKind, options: ParseOptions) -> Result<NodeId, XmlError> {
        if self.nodes.len() >= options.max_nodes {
            return Err(XmlError::LimitExceeded("nodos"));
        }
        Ok(self.new_node(kind))
    }

    fn new_node(&mut self, kind: NodeKind) -> NodeId {
        self.nodes.push(NodeData {
            kind,
            parent: None,
            children: Vec::new(),
        });
        NodeId(self.nodes.len() - 1)
    }

    fn attach(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[child.0].parent = Some(parent);
        self.nodes[parent.0].children.push(child);
    }

    fn detach(&mut self, node: NodeId) {
        if let Some(parent) = self.nodes[node.0].parent.take() {
            self.nodes[parent.0].children.retain(|c| *c != node);
        }
    }

    // ------------------------------------------------------------------
    // Consulta
    // ------------------------------------------------------------------

    /// Tipo y datos del nodo.
    #[must_use]
    pub fn kind(&self, node: NodeId) -> &NodeKind {
        &self.nodes[node.0].kind
    }

    /// Elemento, si el nodo lo es.
    #[must_use]
    pub fn element(&self, node: NodeId) -> Option<&Element> {
        match &self.nodes[node.0].kind {
            NodeKind::Element(e) => Some(e),
            _ => None,
        }
    }

    fn element_mut(&mut self, node: NodeId) -> Option<&mut Element> {
        match &mut self.nodes[node.0].kind {
            NodeKind::Element(e) => Some(e),
            _ => None,
        }
    }

    /// Padre del nodo.
    #[must_use]
    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.nodes[node.0].parent
    }

    /// Hijos del nodo.
    #[must_use]
    pub fn children(&self, node: NodeId) -> &[NodeId] {
        &self.nodes[node.0].children
    }

    /// Elemento raíz.
    #[must_use]
    pub fn document_element(&self) -> Option<NodeId> {
        self.children(self.document_node())
            .iter()
            .copied()
            .find(|c| self.element(*c).is_some())
    }

    /// `true` si `node` está unido al árbol del documento.
    #[must_use]
    pub fn is_attached(&self, node: NodeId) -> bool {
        let mut current = node;
        loop {
            if current == self.document_node() {
                return true;
            }
            match self.parent(current) {
                Some(p) => current = p,
                None => return false,
            }
        }
    }

    /// Hermano anterior.
    #[must_use]
    pub fn previous_sibling(&self, node: NodeId) -> Option<NodeId> {
        let parent = self.parent(node)?;
        let siblings = self.children(parent);
        let pos = siblings.iter().position(|c| *c == node)?;
        pos.checked_sub(1).map(|p| siblings[p])
    }

    /// Hermano siguiente.
    #[must_use]
    pub fn next_sibling(&self, node: NodeId) -> Option<NodeId> {
        let parent = self.parent(node)?;
        let siblings = self.children(parent);
        let pos = siblings.iter().position(|c| *c == node)?;
        siblings.get(pos + 1).copied()
    }

    /// Descendientes de `node` (excluido) en orden de documento.
    #[must_use]
    pub fn descendants(&self, node: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.children(node).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(self.children(n).iter().rev().copied());
        }
        out
    }

    /// Elementos descendientes con namespace y nombre local dados (`.//ns:local`).
    #[must_use]
    pub fn descendants_ns(
        &self,
        node: NodeId,
        namespace: Option<&str>,
        local: &str,
    ) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.children(node).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            if let Some(e) = self.element(n) {
                if e.local == local && e.namespace.as_deref() == namespace {
                    out.push(n);
                }
            }
            stack.extend(self.children(n).iter().rev().copied());
        }
        out
    }

    /// Primer hijo elemento con namespace y nombre local dados (`./ns:local`).
    #[must_use]
    pub fn child_ns(&self, node: NodeId, namespace: &str, local: &str) -> Option<NodeId> {
        self.children_ns(node, namespace, local).into_iter().next()
    }

    /// Hijos elemento con namespace y nombre local dados.
    #[must_use]
    pub fn children_ns(&self, node: NodeId, namespace: &str, local: &str) -> Vec<NodeId> {
        self.children(node)
            .iter()
            .copied()
            .filter(|c| {
                self.element(*c)
                    .is_some_and(|e| e.local == local && e.namespace.as_deref() == Some(namespace))
            })
            .collect()
    }

    /// `getElementsByTagName(nombre)`: descendientes cuyo nombre local o
    /// calificado coincide, en cualquier namespace.
    #[must_use]
    pub fn elements_by_tag_name(&self, node: NodeId, name: &str) -> Vec<NodeId> {
        self.descendants(node)
            .into_iter()
            .filter(|n| {
                self.element(*n)
                    .is_some_and(|e| e.local == name || e.qname() == name)
            })
            .collect()
    }

    /// Nombre local de un elemento.
    #[must_use]
    pub fn local_name(&self, node: NodeId) -> Option<&str> {
        self.element(node).map(|e| e.local.as_str())
    }

    /// Namespace de un elemento.
    #[must_use]
    pub fn namespace_uri(&self, node: NodeId) -> Option<&str> {
        self.element(node).and_then(|e| e.namespace.as_deref())
    }

    /// `getAttribute(nombre)`: atributo sin namespace por nombre, o por
    /// nombre calificado si contiene `:`.
    #[must_use]
    pub fn attribute(&self, node: NodeId, name: &str) -> Option<&str> {
        let element = self.element(node)?;
        let found = if name.contains(':') {
            element.attributes.iter().find(|a| a.qname() == name)
        } else {
            element
                .attributes
                .iter()
                .find(|a| a.local == name && a.prefix.is_none())
        };
        found.map(|a| a.value.as_str())
    }

    /// Atributo por namespace y nombre local.
    #[must_use]
    pub fn attribute_ns(&self, node: NodeId, namespace: Option<&str>, local: &str) -> Option<&str> {
        let element = self.element(node)?;
        element
            .attributes
            .iter()
            .find(|a| a.local == local && a.namespace.as_deref() == namespace)
            .map(|a| a.value.as_str())
    }

    /// Contenido de texto (`textContent`): texto y CDATA descendientes.
    #[must_use]
    pub fn text_content(&self, node: NodeId) -> String {
        match self.kind(node) {
            NodeKind::Text(t) | NodeKind::CData(t) | NodeKind::Comment(t) => t.clone(),
            NodeKind::ProcessingInstruction { data, .. } => data.clone(),
            NodeKind::Element(_) | NodeKind::Document => {
                let mut out = String::new();
                for d in self.descendants(node) {
                    if let NodeKind::Text(t) | NodeKind::CData(t) = self.kind(d) {
                        out.push_str(t);
                    }
                }
                out
            }
        }
    }

    /// Namespace asociado a un prefijo en el ámbito de `node`.
    #[must_use]
    pub fn lookup_namespace(&self, node: NodeId, prefix: Option<&str>) -> Option<String> {
        if prefix == Some("xml") {
            return Some(XML_NS.to_owned());
        }
        let mut current = Some(node);
        while let Some(n) = current {
            if let Some(e) = self.element(n) {
                if let Some(d) = e.ns_decls.iter().find(|d| d.prefix.as_deref() == prefix) {
                    return if d.uri.is_empty() {
                        None
                    } else {
                        Some(d.uri.clone())
                    };
                }
            }
            current = self.parent(n);
        }
        None
    }

    /// Prefijo declarado para un namespace en el ámbito de `node`
    /// (`lookupPrefix`); `None` para el namespace por defecto o si no existe.
    #[must_use]
    pub fn lookup_prefix(&self, node: NodeId, uri: &str) -> Option<String> {
        let mut current = Some(node);
        while let Some(n) = current {
            if let Some(e) = self.element(n) {
                for d in &e.ns_decls {
                    if d.uri == uri {
                        if let Some(p) = &d.prefix {
                            if self.lookup_namespace(node, Some(p)).as_deref() == Some(uri) {
                                return Some(p.clone());
                            }
                        }
                    }
                }
            }
            current = self.parent(n);
        }
        None
    }

    /// Nodos del eje `namespace` de XPath en el orden de libxml2: primero
    /// `xml` y luego las declaraciones en ámbito (incluida `xmlns=""`) en
    /// orden inverso al de `xmlGetNsList`, que recorre las propias en orden de
    /// declaración y después las de los ancestros sin repetir prefijos.
    #[must_use]
    pub fn namespace_nodes(&self, element: NodeId) -> Vec<NsDecl> {
        // Del ámbito más interno al más externo; gana la primera declaración
        // de cada prefijo.
        let mut inner_first: Vec<&NsDecl> = Vec::new();
        let mut current = Some(element);
        while let Some(n) = current {
            if let Some(e) = self.element(n) {
                inner_first.extend(e.ns_decls.iter());
            }
            current = self.parent(n);
        }
        let list: Vec<NsDecl> = first_per_prefix(inner_first).into_iter().cloned().collect();
        let mut out = vec![NsDecl {
            prefix: Some("xml".to_owned()),
            uri: XML_NS.to_owned(),
        }];
        out.extend(
            list.into_iter()
                .rev()
                .filter(|d| d.prefix.as_deref() != Some("xml")),
        );
        out
    }

    /// Namespaces en ámbito (prefijo → URI), del más externo al más interno.
    #[must_use]
    pub fn in_scope_namespaces(&self, node: NodeId) -> Vec<NsDecl> {
        self.in_scope_namespace_refs(node)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Como [`Self::in_scope_namespaces`], sin copiar las declaraciones.
    pub(crate) fn in_scope_namespace_refs(&self, node: NodeId) -> Vec<&NsDecl> {
        let mut chain = Vec::new();
        let mut current = Some(node);
        while let Some(n) = current {
            if self.element(n).is_some_and(|e| !e.ns_decls.is_empty()) {
                chain.push(n);
            }
            current = self.parent(n);
        }
        let all: Vec<&NsDecl> = chain
            .into_iter()
            .rev()
            .filter_map(|n| self.element(n))
            .flat_map(|e| e.ns_decls.iter())
            .collect();
        retain_last_per_prefix(all)
    }

    // ------------------------------------------------------------------
    // Mutación
    // ------------------------------------------------------------------

    /// `createElementNS($ns, $qname, $valor)`: elemento sin padre que declara
    /// su propio namespace.
    pub fn create_element_ns(
        &mut self,
        namespace: Option<&str>,
        qname: &str,
        value: Option<&str>,
    ) -> NodeId {
        let (prefix, local) = split_qname(qname);
        let ns_decls = match namespace {
            Some(uri) if prefix.as_deref() != Some("xml") => vec![NsDecl {
                prefix: prefix.clone(),
                uri: uri.to_owned(),
            }],
            _ => Vec::new(),
        };
        let id = self.new_node(NodeKind::Element(Element {
            prefix,
            local,
            namespace: namespace.map(str::to_owned),
            ns_decls,
            attributes: Vec::new(),
        }));
        if let Some(value) = value {
            let text = self.new_node(NodeKind::Text(parse_node_content(value)));
            self.attach(id, text);
        }
        id
    }

    /// `createTextNode($texto)`.
    pub fn create_text(&mut self, text: &str) -> NodeId {
        self.new_node(NodeKind::Text(text.to_owned()))
    }

    /// `setAttribute($nombre, $valor)`: reemplaza el atributo sin namespace
    /// con ese nombre local o agrega uno nuevo al final.
    pub fn set_attribute(&mut self, node: NodeId, name: &str, value: &str) {
        if let Some(e) = self.element_mut(node) {
            if let Some(a) = e
                .attributes
                .iter_mut()
                .find(|a| a.local == name && a.prefix.is_none())
            {
                a.value = value.to_owned();
            } else {
                e.attributes.push(Attribute {
                    prefix: None,
                    local: name.to_owned(),
                    namespace: None,
                    value: value.to_owned(),
                });
            }
        }
    }

    /// `setAttributeNS($ns, $qname, $valor)`; declara el prefijo si hace falta.
    pub fn set_attribute_ns(
        &mut self,
        node: NodeId,
        namespace: Option<&str>,
        qname: &str,
        value: &str,
    ) {
        let Some(namespace) = namespace else {
            self.set_attribute(node, qname, value);
            return;
        };
        let (prefix, local) = split_qname(qname);
        if namespace == "http://www.w3.org/2000/xmlns/" {
            if let Some(e) = self.element_mut(node) {
                let p = if prefix.as_deref() == Some("xmlns") {
                    Some(local)
                } else {
                    None
                };
                e.ns_decls.retain(|d| d.prefix != p);
                e.ns_decls.push(NsDecl {
                    prefix: p,
                    uri: value.to_owned(),
                });
            }
            return;
        }
        let declared = self.lookup_namespace(node, prefix.as_deref()).as_deref() == Some(namespace);
        if let Some(e) = self.element_mut(node) {
            if !declared && prefix.as_deref() != Some("xml") {
                e.ns_decls.push(NsDecl {
                    prefix: prefix.clone(),
                    uri: namespace.to_owned(),
                });
            }
            if let Some(a) = e
                .attributes
                .iter_mut()
                .find(|a| a.local == local && a.namespace.as_deref() == Some(namespace))
            {
                a.value = value.to_owned();
                a.prefix = prefix;
            } else {
                e.attributes.push(Attribute {
                    prefix,
                    local,
                    namespace: Some(namespace.to_owned()),
                    value: value.to_owned(),
                });
            }
        }
    }

    /// `appendChild`.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> NodeId {
        self.insert_before(parent, child, None)
    }

    /// `insertBefore($nuevo, $referencia)`; con `None` agrega al final.
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        child: NodeId,
        before: Option<NodeId>,
    ) -> NodeId {
        self.detach(child);
        self.nodes[child.0].parent = Some(parent);
        let siblings = &mut self.nodes[parent.0].children;
        match before.and_then(|b| siblings.iter().position(|c| *c == b)) {
            Some(pos) => siblings.insert(pos, child),
            None => siblings.push(child),
        }
        self.reconcile(child);
        child
    }

    /// `removeChild`.
    pub fn remove_child(&mut self, child: NodeId) {
        self.detach(child);
    }

    /// `replaceChild($nuevo, $viejo)`.
    pub fn replace_child(&mut self, new: NodeId, old: NodeId) {
        if let Some(parent) = self.parent(old) {
            self.insert_before(parent, new, Some(old));
            self.detach(old);
        }
    }

    /// Reconciliación de namespaces del DOM tras insertar `node`
    /// (`dom_reconcile_ns`): primero retira del nodo insertado las
    /// declaraciones que el padre ya provee y luego aplica `xmlReconciliateNs`,
    /// que resuelve cada namespace usado en el subárbol **buscando por URI
    /// desde el nodo insertado** (no desde cada nodo) y, si no lo encuentra,
    /// lo declara en el nodo insertado con el prefijo original, `default` o
    /// un sufijo numérico si el prefijo ya está ocupado.
    fn reconcile(&mut self, node: NodeId) {
        let Some(parent) = self.parent(node) else {
            return;
        };
        let Some(element) = self.element(node).cloned() else {
            return;
        };
        let mut kept = Vec::new();
        for decl in element.ns_decls {
            let redundant = !decl.uri.is_empty()
                && self
                    .search_ns_by_href(parent, &decl.uri)
                    .is_some_and(|p| decl.prefix.is_none() || p == decl.prefix);
            if !redundant {
                kept.push(decl);
            }
        }
        if let Some(e) = self.element_mut(node) {
            e.ns_decls = kept;
        }
        let mut targets = vec![node];
        targets.extend(self.descendants(node));
        for n in targets {
            let Some(e) = self.element(n).cloned() else {
                continue;
            };
            if let Some(uri) = &e.namespace {
                let prefix = self.reconciled_prefix(node, uri, e.prefix.as_deref());
                if let Some(el) = self.element_mut(n) {
                    el.prefix = prefix;
                }
            }
            for (i, a) in e.attributes.iter().enumerate() {
                let Some(uri) = &a.namespace else { continue };
                let prefix = self.reconciled_prefix(node, uri, a.prefix.as_deref());
                if let Some(el) = self.element_mut(n) {
                    el.attributes[i].prefix = prefix;
                }
            }
        }
    }

    /// `xmlNewReconciledNs(doc, tree, ns)`: prefijo con el que `tree` ve `uri`,
    /// declarándolo en `tree` si hace falta.
    fn reconciled_prefix(
        &mut self,
        tree: NodeId,
        uri: &str,
        prefix: Option<&str>,
    ) -> Option<String> {
        if uri == XML_NS {
            return Some("xml".to_owned());
        }
        if let Some(found) = self.search_ns_by_href(tree, uri) {
            return found;
        }
        let base = prefix.unwrap_or("default").to_owned();
        let mut candidate = base.clone();
        let mut counter = 1;
        while self.declared_prefix(tree, &candidate) && counter <= 1000 {
            candidate = format!("{base}{counter}");
            counter += 1;
        }
        if let Some(e) = self.element_mut(tree) {
            e.ns_decls.push(NsDecl {
                prefix: Some(candidate.clone()),
                uri: uri.to_owned(),
            });
        }
        Some(candidate)
    }

    /// `xmlSearchNs(doc, node, prefijo)` para un prefijo no vacío.
    fn declared_prefix(&self, node: NodeId, prefix: &str) -> bool {
        let mut current = Some(node);
        while let Some(n) = current {
            if self.element(n).is_some_and(|e| {
                e.ns_decls
                    .iter()
                    .any(|d| d.prefix.as_deref() == Some(prefix))
            }) {
                return true;
            }
            current = self.parent(n);
        }
        false
    }

    /// `xmlSearchNsByHref(doc, node, uri)`: prefijo (`Some(None)` para el
    /// namespace por defecto) de la primera declaración en ámbito con esa URI.
    fn search_ns_by_href(&self, node: NodeId, uri: &str) -> Option<Option<String>> {
        let mut current = Some(node);
        while let Some(n) = current {
            if let Some(e) = self.element(n) {
                for d in &e.ns_decls {
                    if d.uri == uri
                        && self.lookup_namespace(node, d.prefix.as_deref()).as_deref() == Some(uri)
                    {
                        return Some(d.prefix.clone());
                    }
                }
            }
            current = self.parent(n);
        }
        None
    }

    /// `importNode($nodo, true)`: copia profunda desde otro documento, sin padre.
    pub fn import_node(&mut self, source: &Document, node: NodeId) -> NodeId {
        let kind = source.kind(node).clone();
        let id = self.new_node(kind);
        for &child in source.children(node) {
            let copy = self.import_node(source, child);
            self.attach(id, copy);
        }
        id
    }

    /// Copia profunda de un nodo del propio documento, sin padre; equivale a
    /// `import_node(&self.clone(), node)` sin copiar el documento completo.
    pub fn duplicate_node(&mut self, node: NodeId) -> NodeId {
        let kind = self.kind(node).clone();
        let id = self.new_node(kind);
        let children = self.nodes[node.0].children.clone();
        for child in children {
            let copy = self.duplicate_node(child);
            self.attach(id, copy);
        }
        id
    }

    /// Analiza un fragmento bien balanceado en el contexto de namespaces de
    /// `context` (`createDocumentFragment()->appendXML()`), devolviendo los
    /// nodos raíz sin padre.
    ///
    /// # Errors
    ///
    /// [`XmlError`] si el fragmento está mal formado.
    pub fn parse_fragment(&mut self, context: NodeId, xml: &str) -> Result<Vec<NodeId>, XmlError> {
        let holder = self.new_node(NodeKind::Element(Element {
            prefix: None,
            local: "#fragment".into(),
            namespace: None,
            ns_decls: Vec::new(),
            attributes: Vec::new(),
        }));
        let decls: Vec<(Option<String>, String)> = self
            .in_scope_namespaces(context)
            .into_iter()
            .map(|d| (d.prefix, d.uri))
            .collect();
        let mut scopes = vec![Scope {
            in_scope: decls.len(),
            decls,
        }];
        self.parse_into(
            &normalize_eol(xml),
            holder,
            ParseOptions::default(),
            &mut scopes,
            false,
        )?;
        let nodes = self.children(holder).to_vec();
        for n in &nodes {
            self.nodes[n.0].parent = None;
        }
        self.nodes[holder.0].children.clear();
        Ok(nodes)
    }

    // ------------------------------------------------------------------
    // Serialización
    // ------------------------------------------------------------------

    /// `saveXML()`: documento completo como lo serializa libxml2, como texto.
    /// Con una codificación declarada distinta de UTF-8, los caracteres que
    /// esta no puede representar ya salen como `&#N;`; los bytes exactos los
    /// da [`Self::to_xml_bytes`].
    #[must_use]
    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        match &self.declaration {
            Some(d) => {
                let _ = write!(out, "<?xml version=\"{}\"", d.version);
                if let Some(enc) = &d.encoding {
                    let _ = write!(out, " encoding=\"{enc}\"");
                }
                if let Some(s) = &d.standalone {
                    let _ = write!(out, " standalone=\"{s}\"");
                }
                out.push_str("?>\n");
            }
            None => out.push_str("<?xml version=\"1.0\"?>\n"),
        }
        for child in self.children(self.document_node()) {
            self.write_node(&mut out, *child);
            out.push('\n');
        }
        self.output_charset().escape_unencodable(&out)
    }

    /// `saveXML()` en bytes: el texto de [`Self::to_xml`] codificado en la
    /// codificación declarada (ISO-8859-1, …); UTF-8 si no declara otra.
    #[must_use]
    pub fn to_xml_bytes(&self) -> Vec<u8> {
        self.output_charset().encode(&self.to_xml())
    }

    /// Codificación de salida de `saveXML()` según la declaración.
    fn output_charset(&self) -> Charset {
        self.declaration
            .as_ref()
            .and_then(|d| d.encoding.as_deref())
            .and_then(Charset::from_name)
            .unwrap_or(Charset::Utf8)
    }

    /// `saveXML($nodo)`: un nodo sin declaración ni salto final.
    #[must_use]
    pub fn node_to_xml(&self, node: NodeId) -> String {
        if node == self.document_node() {
            return self.to_xml();
        }
        let mut out = String::new();
        self.write_node(&mut out, node);
        out
    }

    fn ascii_only(&self) -> bool {
        self.declaration
            .as_ref()
            .and_then(|d| d.encoding.as_ref())
            .is_none()
    }

    fn write_node(&self, out: &mut String, node: NodeId) {
        let ascii = self.ascii_only();
        match self.kind(node) {
            NodeKind::Document => {
                for c in self.children(node) {
                    self.write_node(out, *c);
                }
            }
            NodeKind::Element(e) => {
                out.push('<');
                out.push_str(&e.qname());
                for d in &e.ns_decls {
                    out.push_str(" xmlns");
                    if let Some(p) = &d.prefix {
                        out.push(':');
                        out.push_str(p);
                    }
                    out.push_str("=\"");
                    escape_attribute(out, &d.uri, ascii);
                    out.push('"');
                }
                for a in &e.attributes {
                    out.push(' ');
                    out.push_str(&a.qname());
                    out.push_str("=\"");
                    escape_attribute(out, &a.value, ascii);
                    out.push('"');
                }
                let children = self.children(node);
                if children.is_empty() {
                    out.push_str("/>");
                } else {
                    out.push('>');
                    for c in children {
                        self.write_node(out, *c);
                    }
                    out.push_str("</");
                    out.push_str(&e.qname());
                    out.push('>');
                }
            }
            NodeKind::Text(t) => escape_text(out, t, ascii),
            NodeKind::CData(t) => {
                out.push_str("<![CDATA[");
                out.push_str(t);
                out.push_str("]]>");
            }
            NodeKind::Comment(t) => {
                out.push_str("<!--");
                out.push_str(t);
                out.push_str("-->");
            }
            NodeKind::ProcessingInstruction { target, data } => {
                out.push_str("<?");
                out.push_str(target);
                if !data.is_empty() {
                    out.push(' ');
                    out.push_str(data);
                }
                out.push_str("?>");
            }
        }
    }
}

fn push_char_ref(out: &mut String, c: char) {
    let _ = write!(out, "&#x{:X};", u32::from(c));
}

/// Escape de texto de libxml2 (`xmlEscapeContent`).
fn escape_text(out: &mut String, text: &str, ascii: bool) {
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\r' => out.push_str("&#13;"),
            c if ascii && !c.is_ascii() => push_char_ref(out, c),
            c => out.push(c),
        }
    }
}

/// Escape de valores de atributo de libxml2 (`xmlBufAttrSerializeTxtContent`).
fn escape_attribute(out: &mut String, text: &str, ascii: bool) {
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if ascii && !c.is_ascii() => push_char_ref(out, c),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_content_like_libxml() {
        for (input, expected) in [
            ("a & b", "a  b"),
            ("a &x; b", "a  b"),
            ("a &amp; b", "a & b"),
            ("a &#65; b", "a A b"),
            ("a&b;c", "ac"),
            ("contains(., '&') or x", "contains(., '') or x"),
            ("a &lt b", "a lt b"),
            ("&#x42;&#0;", "B"),
            ("x&", "x"),
            ("&;", ""),
            ("a & b & c", "a  b & c"),
            ("a & b &amp; c", "a  c"),
            ("a &#65 b &amp; c", "a  b & c"),
            ("&amp; & &amp;", "& "),
            ("a &#xZZ; b &amp;", "a ZZ; b &"),
            ("a &#; b &amp;", "a  b &"),
            ("a &amp;&lt; b &x c &amp;", "a &< b "),
        ] {
            assert_eq!(parse_node_content(input), expected, "{input}");
        }
    }

    #[test]
    fn round_trip_basics() {
        let doc = Document::parse("<a x=\"1\"><b></b>t&amp;<c/></a>").unwrap();
        assert_eq!(
            doc.to_xml(),
            "<?xml version=\"1.0\"?>\n<a x=\"1\"><b/>t&amp;<c/></a>\n"
        );
    }

    #[test]
    fn rejects_dtd_and_unknown_entities() {
        assert_eq!(
            Document::parse("<!DOCTYPE a [<!ENTITY x \"y\">]><a>&x;</a>").unwrap_err(),
            XmlError::Dtd
        );
        assert!(matches!(
            Document::parse("<a>&x;</a>").unwrap_err(),
            XmlError::UndefinedEntity(_)
        ));
    }

    #[test]
    fn enforces_limits() {
        let deep = format!("{}{}", "<a>".repeat(300), "</a>".repeat(300));
        assert_eq!(
            Document::parse(&deep).unwrap_err(),
            XmlError::LimitExceeded("profundidad")
        );
        let options = ParseOptions {
            max_bytes: 10,
            ..ParseOptions::default()
        };
        assert_eq!(
            Document::parse_with("<a>0123456789</a>", options).unwrap_err(),
            XmlError::LimitExceeded("tamaño")
        );
    }
}
