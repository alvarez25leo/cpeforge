//! Validación del envelope SOAP 1.1/1.2 de una respuesta y decodificación de
//! sus faults.
//!
//! Las reglas de validación, los códigos y los textos de los faults son parte
//! del contrato de interoperabilidad con los servicios SOAP de SUNAT.
//!
//! El cliente de referencia lee la respuesta con el modelo de nodos de
//! libxml2, que difiere del de roxmltree: descarta los comentarios, une los
//! textos contiguos, mantiene cada CDATA como nodo propio y descarta los
//! textos en blanco que acompañan a otros nodos. [`segments`] reconstruye esa
//! lista de hijos para que el texto y el `detail` de un fault se decodifiquen
//! igual.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Write as _;

use roxmltree::{Document, Node, NodeType};

use crate::WsError;

/// Espacio de nombres del envelope SOAP 1.1.
pub(crate) const SOAP11: &str = "http://schemas.xmlsoap.org/soap/envelope/";
/// Espacio de nombres del envelope SOAP 1.2.
const SOAP12: &str = "http://www.w3.org/2003/05/soap-envelope";
/// Codificación SOAP 1.1 (único `encodingStyle` admitido).
const SOAP11_ENC: &str = "http://schemas.xmlsoap.org/soap/encoding/";
/// Espacio de nombres `xsi` (XML Schema instance).
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Fault de código `Client`, generado localmente al leer la respuesta.
fn client_fault(message: &str) -> WsError {
    WsError::SoapFault {
        code: "Client".to_owned(),
        message: message.to_owned(),
        detail: None,
    }
}

/// Un texto escalar del fault contiene elementos o varios nodos.
fn violation() -> WsError {
    client_fault("SOAP-ERROR: Encoding: Violation of encoding rules")
}

/// El cuerpo no es un documento XML legible (malformado, codificación no
/// admitida o fuera de los límites de profundidad).
pub(crate) fn no_xml_document() -> WsError {
    client_fault("looks like we got no XML document")
}

/// La respuesta declara un DTD, que nunca se procesa.
pub(crate) fn dtd_fault() -> WsError {
    client_fault("DTD are not supported by SOAP")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Version {
    Soap11,
    Soap12,
}

impl Version {
    const fn namespace(self) -> &'static str {
        match self {
            Self::Soap11 => SOAP11,
            Self::Soap12 => SOAP12,
        }
    }
}

fn is(node: Node<'_, '_>, namespace: &str, name: &str) -> bool {
    node.is_element()
        && node.tag_name().namespace() == Some(namespace)
        && node.tag_name().name() == name
}

/// Primer hijo elemento llamado `name`, en cualquier espacio de nombres: los
/// hijos de `Fault` se buscan por nombre local, sin prefijo obligatorio.
fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

/// Comprueba los atributos de `Envelope`, `Header` o `Body`.
fn check_attributes(
    node: Node<'_, '_>,
    version: Version,
    element: &str,
    unqualified_allowed: bool,
) -> Result<(), WsError> {
    for attribute in node.attributes() {
        match attribute.namespace() {
            None if unqualified_allowed => {}
            None => {
                return Err(client_fault(&format!(
                    "A SOAP {element} element cannot have non Namespace qualified attributes"
                )));
            }
            // `encodingStyle` se busca en el espacio de nombres SOAP 1.2 también
            // en los envelopes 1.1.
            Some(SOAP12) if attribute.name() == "encodingStyle" => {
                if version == Version::Soap12 {
                    return Err(client_fault(&format!(
                        "encodingStyle cannot be specified on the {element}"
                    )));
                }
                if attribute.value() != SOAP11_ENC {
                    return Err(client_fault("Unknown data encoding style"));
                }
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Valida el envelope y devuelve su `Body`. Un fault del servidor, o uno
/// generado al validar, es `Err(WsError::SoapFault)`.
pub(crate) fn body<'a, 'i>(doc: &'a Document<'i>) -> Result<Node<'a, 'i>, WsError> {
    let envelope = doc.root_element();
    let version = if is(envelope, SOAP11, "Envelope") {
        Version::Soap11
    } else if is(envelope, SOAP12, "Envelope") {
        Version::Soap12
    } else {
        return Err(WsError::SoapFault {
            code: "VersionMismatch".to_owned(),
            message: "Wrong Version".to_owned(),
            detail: None,
        });
    };
    let ns = version.namespace();
    check_attributes(envelope, version, "Envelope", false)?;

    let mut elements = envelope.children().filter(Node::is_element);
    let mut next = elements.next();
    let header = next.filter(|n| is(*n, ns, "Header"));
    if header.is_some() {
        next = elements.next();
    }
    let body = next
        .filter(|n| is(*n, ns, "Body"))
        .ok_or_else(|| client_fault("Body must be present in a SOAP envelope"))?;
    check_attributes(body, version, "Body", version == Version::Soap11)?;
    if version == Version::Soap12 && elements.next().is_some() {
        return Err(client_fault(
            "A SOAP 1.2 envelope can contain only Header and Body",
        ));
    }
    if let Some(header) = header {
        check_attributes(header, version, "Header", false)?;
    }

    if let Some(fault) = body.children().find(|n| is(*n, ns, "Fault")) {
        return Err(read_fault(doc.input_text(), fault, version));
    }
    Ok(body)
}

/// Hijo de un elemento en el modelo de nodos de libxml2.
enum Segment<'a, 'i> {
    Text { value: String, blank: bool },
    CData(String),
    Element(Node<'a, 'i>),
    Pi(String),
}

/// Hijos de `node` en el modelo de nodos de libxml2.
fn segments<'a, 'i>(source: &str, node: Node<'a, 'i>) -> Vec<Segment<'a, 'i>> {
    let mut out: Vec<Segment<'a, 'i>> = Vec::new();
    for child in node.children() {
        match child.node_type() {
            NodeType::Element => out.push(Segment::Element(child)),
            NodeType::PI => out.push(Segment::Pi(
                child
                    .pi()
                    .and_then(|pi| pi.value)
                    .unwrap_or_default()
                    .to_owned(),
            )),
            NodeType::Text => {
                for piece in text_pieces(source, child) {
                    match (piece, out.last_mut()) {
                        // El texto se une al nodo de texto anterior, también
                        // a través de un comentario descartado.
                        (
                            Segment::Text { value, blank },
                            Some(Segment::Text {
                                value: previous,
                                blank: previous_blank,
                            }),
                        ) => {
                            previous.push_str(&value);
                            *previous_blank &= blank;
                        }
                        (piece, _) => out.push(piece),
                    }
                }
            }
            NodeType::Comment | NodeType::Root => {}
        }
    }
    // Un texto en blanco solo se conserva si es todo el contenido del
    // elemento.
    if out.len() > 1 {
        out.retain(|segment| !matches!(segment, Segment::Text { blank: true, .. }));
    }
    out
}

/// Separa un nodo de texto de roxmltree (que une texto y CDATA) en los
/// trozos que libxml2 mantiene separados, leyendo su fuente.
fn text_pieces<'a, 'i>(source: &str, text: Node<'a, 'i>) -> Vec<Segment<'a, 'i>> {
    let start = text.range().start;
    let end = match text.next_sibling() {
        Some(next) => next.range().start,
        // Sin hermano siguiente, el texto acaba en la etiqueta de cierre.
        None => text.parent().map_or(start, |parent| {
            let range = parent.range();
            source
                .get(range.clone())
                .and_then(|raw| raw.rfind("</"))
                .map_or(range.end, |offset| range.start + offset)
        }),
    };
    let raw = source.get(start..end).unwrap_or_default();
    let decoded = text.text().unwrap_or_default();
    if !raw.contains("<![CDATA[") {
        return vec![Segment::Text {
            value: decoded.to_owned(),
            blank: is_blank(raw),
        }];
    }
    let mut pieces = Vec::new();
    let mut rest = raw;
    while let Some(open) = rest.find("<![CDATA[") {
        let (plain, after) = rest.split_at(open);
        if !plain.is_empty() {
            pieces.push(Segment::Text {
                value: decode_text(plain),
                blank: is_blank(plain),
            });
        }
        let content = after.get("<![CDATA[".len()..).unwrap_or_default();
        let close = content.find("]]>").unwrap_or(content.len());
        let (cdata, tail) = content.split_at(close);
        pieces.push(Segment::CData(normalize_newlines(cdata)));
        rest = tail.get("]]>".len()..).unwrap_or_default();
    }
    if !rest.is_empty() {
        pieces.push(Segment::Text {
            value: decode_text(rest),
            blank: is_blank(rest),
        });
    }
    pieces
}

/// Texto fuente formado solo por espacio, tabulador, CR o LF.
fn is_blank(raw: &str) -> bool {
    raw.bytes()
        .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

fn normalize_newlines(raw: &str) -> String {
    raw.replace("\r\n", "\n").replace('\r', "\n")
}

/// Texto fuente ya validado por roxmltree: saltos de línea y referencias
/// predefinidas o numéricas (no hay DTD, así que no hay otras entidades).
fn decode_text(raw: &str) -> String {
    let raw = normalize_newlines(raw);
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw.as_str();
    while let Some(amp) = rest.find('&') {
        let (before, after) = rest.split_at(amp);
        out.push_str(before);
        let Some(semi) = after.find(';') else {
            out.push_str(after);
            return out;
        };
        let name = after.get(1..semi).unwrap_or_default();
        let ch = match name {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name
                .strip_prefix("#x")
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| name.strip_prefix('#').map(str::parse))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match ch {
            Some(ch) => out.push(ch),
            None => out.push_str(after.get(..=semi).unwrap_or_default()),
        }
        rest = after.get(semi + 1..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}

/// `faultcode`: el contenido del primer hijo (`tmp->children->content`);
/// `None` si no hay hijos o el primero es un elemento.
fn first_content(source: &str, node: Node<'_, '_>) -> Option<String> {
    match segments(source, node).into_iter().next()? {
        Segment::Text { value, .. } | Segment::CData(value) | Segment::Pi(value) => Some(value),
        Segment::Element(_) => None,
    }
}

/// Texto escalar de un elemento: un único texto o CDATA; sin hijos, cadena
/// vacía. Cualquier otra combinación viola la codificación del fault.
fn scalar_text(source: &str, node: Node<'_, '_>) -> Result<String, WsError> {
    scalar_from_segments(segments(source, node))
}

fn scalar_from_segments(segments: Vec<Segment<'_, '_>>) -> Result<String, WsError> {
    let mut segments = segments.into_iter();
    match (segments.next(), segments.next()) {
        (None, _) => Ok(String::new()),
        (Some(Segment::Text { value, .. } | Segment::CData(value)), None) => Ok(value),
        _ => Err(violation()),
    }
}

/// Valor de un nodo del `detail`, decodificado sin esquema.
enum FaultValue<'i> {
    Null,
    String(String),
    /// Propiedades en orden de primera aparición.
    Object(Vec<(&'i str, FaultValue<'i>)>),
    List(Vec<FaultValue<'i>>),
}

/// Decodifica un nodo del `detail`: `xsi:nil` es nulo; sin elementos hijos,
/// su texto escalar; con elementos, un objeto con una propiedad por nombre
/// local, donde los nombres repetidos forman una lista y el texto suelto se
/// ignora.
///
/// La recursión está acotada por [`crate::soap::MAX_SOAP_DEPTH`], que se
/// comprueba antes de construir el árbol.
fn decode_fault_value<'i>(source: &str, node: Node<'_, 'i>) -> Result<FaultValue<'i>, WsError> {
    if node.has_attribute((XSI, "nil")) {
        return Ok(FaultValue::Null);
    }
    if node.has_attribute((XSI, "type")) {
        // Un valor tipado exigiría convertirlo al tipo XSD declarado; se
        // rechaza en lugar de devolver un texto distinto.
        return Err(WsError::InvalidResponse("detail de fault con xsi:type"));
    }
    let segments = segments(source, node);
    if !segments.iter().any(|s| matches!(s, Segment::Element(_))) {
        return scalar_from_segments(segments).map(FaultValue::String);
    }
    // Índice nombre → posición: agrupar es lineal aunque el `detail` tenga
    // miles de nombres distintos, y se conserva el orden de primera aparición.
    let mut index: HashMap<&'i str, usize> = HashMap::new();
    let mut properties: Vec<(&'i str, Vec<FaultValue<'i>>)> = Vec::new();
    for segment in segments {
        let Segment::Element(element) = segment else {
            continue;
        };
        let value = decode_fault_value(source, element)?;
        let name = element.tag_name().name();
        match index.entry(name) {
            Entry::Occupied(slot) => {
                if let Some((_, values)) = properties.get_mut(*slot.get()) {
                    values.push(value);
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(properties.len());
                properties.push((name, vec![value]));
            }
        }
    }
    Ok(FaultValue::Object(
        properties
            .into_iter()
            .map(|(name, mut values)| {
                let value = if values.len() == 1 {
                    values.pop().unwrap_or(FaultValue::Null)
                } else {
                    FaultValue::List(values)
                };
                (name, value)
            })
            .collect(),
    ))
}

/// Serializa el `detail` como JSON compacto: `/` se escapa como `\/` y todo
/// carácter no ASCII como secuencias `\uXXXX` (pares sustitutos UTF-16 fuera
/// del plano básico). El formato es el del mensaje de error que reciben los
/// consumidores y no debe cambiar.
fn write_fault_json(value: &FaultValue<'_>, out: &mut String) {
    match value {
        FaultValue::Null => out.push_str("null"),
        FaultValue::String(text) => write_json_string(text, out),
        FaultValue::List(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_fault_json(item, out);
            }
            out.push(']');
        }
        FaultValue::Object(properties) => {
            out.push('{');
            for (index, (name, item)) in properties.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_json_string(name, out);
                out.push(':');
                write_fault_json(item, out);
            }
            out.push('}');
        }
    }
}

fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ' '..='\u{7f}' => out.push(ch),
            _ => {
                let mut units = [0u16; 2];
                for unit in ch.encode_utf16(&mut units) {
                    // Escribir en un `String` no falla.
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// Lee el fault del servidor (SOAP 1.1: `faultcode`, `faultstring`,
/// `faultactor`, `detail`; SOAP 1.2: `Code/Value`, `Reason/Text`, `Detail`).
fn read_fault(source: &str, fault: Node<'_, '_>, version: Version) -> WsError {
    match decode_fault(source, fault, version) {
        Ok(error) | Err(error) => error,
    }
}

fn decode_fault(source: &str, fault: Node<'_, '_>, version: Version) -> Result<WsError, WsError> {
    let (code, message, detail) = match version {
        Version::Soap11 => {
            let code = child(fault, "faultcode").and_then(|n| first_content(source, n));
            let message = child(fault, "faultstring")
                .filter(|n| n.has_children())
                .map(|n| scalar_text(source, n))
                .transpose()?;
            if let Some(actor) = child(fault, "faultactor").filter(|n| n.has_children()) {
                scalar_text(source, actor)?;
            }
            (code, message, child(fault, "detail"))
        }
        Version::Soap12 => {
            let code = child(fault, "Code")
                .and_then(|n| child(n, "Value"))
                .and_then(|n| first_content(source, n));
            let message = child(fault, "Reason")
                .and_then(|n| child(n, "Text"))
                .filter(|n| n.has_children())
                .map(|n| scalar_text(source, n))
                .transpose()?;
            (code, message, child(fault, "Detail"))
        }
    };
    let detail = match detail.map(|n| decode_fault_value(source, n)).transpose()? {
        None | Some(FaultValue::Null) => None,
        Some(value) => {
            let mut json = String::new();
            write_fault_json(&value, &mut json);
            Some(json)
        }
    };
    // Sin `faultcode` no hay código que asignar al error: el fault no se
    // convierte en resultado y el envío devuelve `Err`.
    let code = code.ok_or(WsError::InvalidResponse("fault SOAP sin faultcode"))?;
    Ok(WsError::SoapFault {
        code,
        message: message.unwrap_or_default(),
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fault_json_format() {
        let mut out = String::new();
        write_fault_json(
            &FaultValue::Object(vec![
                ("a", FaultValue::String("\"/\\\t\u{7f}é😀".into())),
                (
                    "b",
                    FaultValue::List(vec![FaultValue::Null, FaultValue::String(String::new())]),
                ),
            ]),
            &mut out,
        );
        assert_eq!(
            out,
            "{\"a\":\"\\\"\\/\\\\\\t\u{7f}\\u00e9\\ud83d\\ude00\",\"b\":[null,\"\"]}"
        );
    }

    fn detail_json(detail: &str) -> Result<String, WsError> {
        let xml = format!("<detail>{detail}</detail>");
        let doc = Document::parse(&xml).map_err(|_| WsError::Xml("fixture"))?;
        let value = decode_fault_value(doc.input_text(), doc.root_element())?;
        let mut out = String::new();
        write_fault_json(&value, &mut out);
        Ok(out)
    }

    #[test]
    fn repeated_names_keep_first_appearance_order() -> Result<(), WsError> {
        assert_eq!(
            detail_json("<a>1</a><b>2</b><a>3</a><c><d>x</d></c><b>4</b>")?,
            "{\"a\":[\"1\",\"3\"],\"b\":[\"2\",\"4\"],\"c\":{\"d\":\"x\"}}"
        );
        let many: String = (0..3000)
            .map(|i| format!("<e{}>{i}</e{}>", i % 1500, i % 1500))
            .collect();
        let json = detail_json(&many)?;
        assert!(json.starts_with("{\"e0\":[\"0\",\"1500\"],\"e1\":[\"1\",\"1501\"]"));
        assert!(json.ends_with("\"e1499\":[\"1499\",\"2999\"]}"));
        Ok(())
    }

    #[test]
    fn decodes_references_and_newlines() {
        assert_eq!(decode_text("a&lt;&#65;&#x42;\r\nb"), "a<AB\nb");
    }
}
