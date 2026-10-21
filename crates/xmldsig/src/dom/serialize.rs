//! Serialización igual a `saveXML()` de libxml2 sin formato.

use std::fmt::Write as _;

use super::{Document, NodeId, NodeKind};
use crate::encoding::Charset;

impl Document {
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
        let ascii = self.ascii_only();
        for child in self.children(self.document_node()) {
            self.write_node(&mut out, *child, ascii);
            out.push('\n');
        }
        self.output_charset().escape_unencodable(out)
    }

    /// `saveXML()` en bytes: el texto de [`Self::to_xml`] codificado en la
    /// codificación declarada (ISO-8859-1, …); UTF-8 si no declara otra.
    #[must_use]
    pub fn to_xml_bytes(&self) -> Vec<u8> {
        self.output_charset().encode(self.to_xml())
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
        self.write_node(&mut out, node, self.ascii_only());
        out
    }

    fn ascii_only(&self) -> bool {
        self.declaration
            .as_ref()
            .and_then(|d| d.encoding.as_ref())
            .is_none()
    }

    /// Escribe `node`; `ascii` (sin codificación declarada) escribe lo que no
    /// es ASCII como referencia de carácter.
    fn write_node(&self, out: &mut String, node: NodeId, ascii: bool) {
        match self.kind(node) {
            NodeKind::Document => {
                for c in self.children(node) {
                    self.write_node(out, *c, ascii);
                }
            }
            NodeKind::Element(e) => {
                out.push('<');
                e.push_qname(out);
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
                    a.push_qname(out);
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
                        self.write_node(out, *c, ascii);
                    }
                    out.push_str("</");
                    e.push_qname(out);
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

/// Copia `text` por tramos: los bytes ASCII que `escape` reemplaza y, con
/// `ascii`, cada carácter no ASCII (como `&#xN;`) cortan el tramo; el resto
/// se copia en bloque.
fn escape_with(out: &mut String, text: &str, ascii: bool, escape: fn(u8) -> Option<&'static str>) {
    let bytes = text.as_bytes();
    let (mut start, mut index) = (0, 0);
    while let Some(&byte) = bytes.get(index) {
        if let Some(replacement) = escape(byte) {
            out.push_str(&text[start..index]);
            out.push_str(replacement);
            index += 1;
            start = index;
        } else if ascii && !byte.is_ascii() {
            out.push_str(&text[start..index]);
            let Some(c) = text[index..].chars().next() else {
                break;
            };
            push_char_ref(out, c);
            index += c.len_utf8();
            start = index;
        } else {
            index += 1;
        }
    }
    out.push_str(&text[start..]);
}

/// Escape de texto de libxml2 (`xmlEscapeContent`).
fn escape_text(out: &mut String, text: &str, ascii: bool) {
    escape_with(out, text, ascii, |b| match b {
        b'<' => Some("&lt;"),
        b'>' => Some("&gt;"),
        b'&' => Some("&amp;"),
        b'\r' => Some("&#13;"),
        _ => None,
    });
}

/// Escape de valores de atributo de libxml2 (`xmlBufAttrSerializeTxtContent`).
fn escape_attribute(out: &mut String, text: &str, ascii: bool) {
    escape_with(out, text, ascii, |b| match b {
        b'<' => Some("&lt;"),
        b'>' => Some("&gt;"),
        b'&' => Some("&amp;"),
        b'"' => Some("&quot;"),
        b'\n' => Some("&#10;"),
        b'\r' => Some("&#13;"),
        b'\t' => Some("&#9;"),
        _ => None,
    });
}
