//! Análisis seguro del texto XML a un [`Document`]: decodificación,
//! normalización de finales de línea, resolución de espacios de nombres y
//! límites de tamaño, profundidad, nodos y trabajo de espacios de nombres.

use quick_xml::events::{BytesStart, Event};

use super::{
    Attribute, Document, Element, NodeId, NodeKind, NsDecl, ParseOptions, XML_NS, XmlDeclaration,
    is_xml_char, split_qname,
};
use crate::encoding::{self, Charset};
use crate::error::XmlError;

/// Rechaza el primer carácter que no es [`is_xml_char`].
///
/// Un `str` no contiene sustitutos, así que los únicos caracteres excluidos
/// posibles son los controles C0 (salvo tabulador, LF y CR) y U+FFFE/U+FFFF
/// (`EF BF BE`/`EF BF BF` en UTF-8): se buscan sobre los bytes, sin
/// decodificar cada carácter.
fn check_chars(text: &str) -> Result<(), XmlError> {
    let bytes = text.as_bytes();
    let invalid = bytes.iter().enumerate().position(|(i, &b)| match b {
        b'\t' | b'\n' | b'\r' => false,
        0..0x20 => true,
        0xEF => bytes.get(i + 1) == Some(&0xBF) && matches!(bytes.get(i + 2), Some(0xBE | 0xBF)),
        _ => false,
    });
    match invalid
        .and_then(|i| text.get(i..))
        .and_then(|t| t.chars().next())
    {
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

/// Texto de un tramo que entrega quick-xml. La entrada es `&str` y el lector
/// presta subcadenas suyas cortadas en delimitadores ASCII: si `bytes` está
/// dentro de `source`, basta comprobar los dos límites de carácter (O(1)) en
/// lugar de validar otra vez el UTF-8; un tramo propio se valida.
fn utf8<'a>(source: &'a str, bytes: &'a [u8]) -> Result<&'a str, XmlError> {
    let offset = bytes.as_ptr().addr().wrapping_sub(source.as_ptr().addr());
    if let Some(text) = offset
        .checked_add(bytes.len())
        .filter(|end| offset <= source.len() && *end <= source.len())
        .and_then(|end| source.get(offset..end))
    {
        return Ok(text);
    }
    std::str::from_utf8(bytes).map_err(|e| XmlError::Malformed(e.to_string()))
}

/// Decodifica y normaliza el valor de un atributo (XML 1.0 §3.3.3).
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

/// Hasta este número de atributos los duplicados se buscan comparando con
/// los anteriores, sin crear un conjunto (un elemento UBL tiene uno o dos).
const SMALL_ATTRIBUTE_SET: usize = 16;

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
        let text = normalize_eol(&text);
        let mut doc = Self::new();
        doc.input_charset = charset;
        // Cada elemento empieza en un `<` y cada texto sigue a uno: contarlos
        // estima el número de nodos y evita copiar el árbol al crecer. La
        // reserva no pasa de `max_nodes`, lo máximo que el análisis admite.
        let estimate = text.bytes().filter(|b| *b == b'<').count();
        doc.nodes.reserve(estimate.min(options.max_nodes));
        let root = doc.document_node();
        doc.parse_into(&text, root, options, &mut Vec::new(), true)?;
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
                        xml,
                        &decl
                            .version()
                            .map_err(|e| XmlError::Malformed(e.to_string()))?,
                    )?
                    .to_owned();
                    let encoding = decl
                        .encoding()
                        .transpose()
                        .map_err(|e| XmlError::Malformed(e.to_string()))?
                        .map(|e| utf8(xml, &e).map(str::to_owned))
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
                        .map(|s| utf8(xml, &s).map(str::to_owned))
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
                    let id = self.parse_element(xml, &start, scopes, options)?;
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
                    let id = self.parse_element(xml, &start, scopes, options)?;
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
                    let text: &str = utf8(xml, &text)?;
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
                    let c = resolve_reference(utf8(xml, &reference)?)?;
                    self.push_text(current, c.encode_utf8(&mut [0; 4]), options)?;
                }
                Event::CData(cdata) => {
                    if at_top {
                        return Err(XmlError::Malformed("CDATA fuera del elemento raíz".into()));
                    }
                    let text = utf8(xml, &cdata.into_inner())?.to_owned();
                    check_chars(&text)?;
                    let id = self.alloc(NodeKind::CData(text), options)?;
                    self.attach(current, id);
                }
                Event::Comment(comment) => {
                    let text = utf8(xml, &comment.into_inner())?.to_owned();
                    check_chars(&text)?;
                    let id = self.alloc(NodeKind::Comment(text), options)?;
                    self.attach(current, id);
                }
                Event::PI(pi) => {
                    let target = utf8(xml, pi.target())?.to_owned();
                    if target.eq_ignore_ascii_case("xml") {
                        return Err(XmlError::Malformed("instrucción 'xml' reservada".into()));
                    }
                    let data = utf8(xml, pi.content())?.trim_start().to_owned();
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
        source: &str,
        start: &BytesStart<'_>,
        scopes: &mut Vec<Scope>,
        options: ParseOptions,
    ) -> Result<NodeId, XmlError> {
        let (prefix, local) = split_qname(checked_qname(utf8(source, start.name().0)?)?);
        let mut ns_decls = Vec::new();
        // Nombres prestados de la etiqueta: se separan en prefijo y nombre
        // local una sola vez, después de leer las declaraciones.
        let mut raw_attrs: Vec<(&str, String)> = Vec::new();
        for attr in start.attributes().with_checks(true) {
            let attr = attr.map_err(|e| XmlError::Malformed(e.to_string()))?;
            let key = checked_qname(utf8(source, attr.key.0)?)?;
            let value = decode_attribute(utf8(source, &attr.value)?)?;
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
                raw_attrs.push((key, value));
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
        // Duplicados por (namespace, nombre local): con pocos atributos se
        // comparan con los anteriores; con muchos, un conjunto mantiene el
        // tiempo lineal (comparar con todos era cuadrático: 160 000 atributos
        // → 15 s).
        let mut seen: Option<std::collections::HashSet<(String, String)>> = (raw_attrs.len()
            > SMALL_ATTRIBUTE_SET)
            .then(|| std::collections::HashSet::with_capacity(raw_attrs.len()));
        for (key, value) in raw_attrs {
            let (p, l) = split_qname(key);
            let namespace = p
                .as_deref()
                .and_then(|p| lookup_scope(scopes, Some(p)))
                .map(str::to_owned);
            if let (Some(p), None) = (p.as_deref(), &namespace) {
                return Err(XmlError::Malformed(format!("prefijo no declarado {p}")));
            }
            if let Some(ns) = &namespace {
                let duplicate = match seen.as_mut() {
                    Some(seen) => !seen.insert((ns.clone(), l.clone())),
                    None => attributes
                        .iter()
                        .any(|a| a.local == l && a.namespace.as_ref() == Some(ns)),
                };
                if duplicate {
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
}
