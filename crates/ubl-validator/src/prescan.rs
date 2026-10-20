//! Preescaneo lineal del XML de entrada, antes de libxml2.
//!
//! libxml2 anterior a 2.12 (la de las distribuciones LTS) compara cada
//! atributo con todos los anteriores del mismo elemento: un elemento con
//! decenas de miles de atributos tarda minutos. Y un `DOCTYPE` se analiza
//! entero (entidades, ciclos) antes de rechazarse. Este recorrido, en tiempo
//! lineal y sin estado global, rechaza ambos casos antes de entregar el
//! documento. UTF-16 se decodifica antes de recorrerlo (en UTF-16 los bytes
//! `<`, `!` y `=` no aparecen tal cual y el filtro se podría saltar); el resto
//! debe ser compatible con ASCII (UTF-8, ISO-8859-x, windows-125x): UTF-32,
//! UCS-4 y EBCDIC se rechazan.

/// Máximo de atributos por elemento, incluidas las declaraciones `xmlns`.
/// Un comprobante UBL usa pocos (la raíz, una docena de `xmlns`).
pub(crate) const MAX_ATTRIBUTES_PER_ELEMENT: usize = 256;

/// Motivo de rechazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rejection {
    /// `<!DOCTYPE`, `<!ENTITY` u otra declaración `<!…` fuera de comentarios y CDATA.
    Doctype,
    /// Un elemento con más de [`MAX_ATTRIBUTES_PER_ELEMENT`] atributos.
    Attributes,
    /// UTF-32, UCS-4, EBCDIC u otra codificación no compatible con ASCII.
    Encoding,
}

/// Recorre `xml` una vez.
pub(crate) fn prescan(xml: &[u8]) -> Result<(), Rejection> {
    if let Some(big_endian) = utf16(xml) {
        let units = xml.chunks_exact(2).map(|pair| {
            let pair = [pair[0], pair[1]];
            if big_endian {
                u16::from_be_bytes(pair)
            } else {
                u16::from_le_bytes(pair)
            }
        });
        let decoded: String = char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        return scan(decoded.as_bytes());
    }
    if !ascii_compatible(xml) {
        return Err(Rejection::Encoding);
    }
    scan(xml)
}

/// `Some(big_endian)` si `xml` está en UTF-16 (con BOM o por el `<` inicial).
fn utf16(xml: &[u8]) -> Option<bool> {
    match xml {
        [0xFE, 0xFF, ..] | [0x00, b'<', 0x00, _, ..] => Some(true),
        [0xFF, 0xFE, ..] | [b'<', 0x00, _, 0x00, ..] => Some(false),
        _ => None,
    }
}

fn scan(xml: &[u8]) -> Result<(), Rejection> {
    let n = xml.len();
    let mut i = 0;
    while i < n {
        let Some(offset) = xml[i..].iter().position(|&b| b == b'<') else {
            break;
        };
        i += offset;
        let rest = &xml[i..];
        if rest.starts_with(b"<!--") {
            i = skip_past(xml, i + 4, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            i = skip_past(xml, i + 9, b"]]>");
        } else if rest.starts_with(b"<!") {
            return Err(Rejection::Doctype);
        } else if rest.starts_with(b"<?") {
            i = skip_past(xml, i + 2, b"?>");
        } else if rest.starts_with(b"</") {
            i = skip_past(xml, i + 2, b">");
        } else {
            i = start_tag(xml, i + 1)?;
        }
    }
    Ok(())
}

/// Cuenta los `=` fuera de comillas hasta el `>` que cierra la etiqueta.
fn start_tag(xml: &[u8], mut i: usize) -> Result<usize, Rejection> {
    let mut attributes = 0usize;
    let mut quote: Option<u8> = None;
    while let Some(&c) = xml.get(i) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'=' => {
                    attributes += 1;
                    if attributes > MAX_ATTRIBUTES_PER_ELEMENT {
                        return Err(Rejection::Attributes);
                    }
                }
                b'>' => return Ok(i + 1),
                _ => {}
            },
        }
        i += 1;
    }
    Ok(i)
}

/// Posición después de `marker` buscado desde `from` (o el final).
fn skip_past(xml: &[u8], from: usize, marker: &[u8]) -> usize {
    xml.get(from..)
        .and_then(|rest| rest.windows(marker.len()).position(|w| w == marker))
        .map_or(xml.len(), |p| from + p + marker.len())
}

/// Sin bytes nulos de UTF-32 ni una declaración de codificación de más de un
/// byte o EBCDIC.
fn ascii_compatible(xml: &[u8]) -> bool {
    if xml.iter().take(4).any(|&b| b == 0) {
        return false;
    }
    let body = xml.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(xml);
    if !body.starts_with(b"<?xml") {
        return true;
    }
    let end = body
        .windows(2)
        .position(|w| w == b"?>")
        .unwrap_or(body.len().min(256));
    let declaration = String::from_utf8_lossy(&body[..end]).to_ascii_uppercase();
    let Some(at) = declaration.find("ENCODING") else {
        return true;
    };
    let value: String = declaration[at + 8..]
        .trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '=')
        .trim_start_matches(['"', '\''])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
        .collect();
    ![
        "UTF-16", "UTF16", "UTF-32", "UTF32", "UCS-2", "UCS2", "UCS-4", "UCS4", "EBCDIC", "IBM037",
        "CP037",
    ]
    .iter()
    .any(|bad| value.starts_with(bad))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_doctype_attributes_and_wide_encodings() {
        assert_eq!(
            prescan(b"<!DOCTYPE a [<!ENTITY x 'y'>]><a/>"),
            Err(Rejection::Doctype)
        );
        assert_eq!(
            prescan(b"<?xml version='1.0'?>\n<!ENTITY x 'y'><a/>"),
            Err(Rejection::Doctype)
        );
        let many: String = (0..=MAX_ATTRIBUTES_PER_ELEMENT)
            .map(|i| format!(" a{i}=''"))
            .collect();
        assert_eq!(
            prescan(format!("<a{many}/>").as_bytes()),
            Err(Rejection::Attributes)
        );
        assert_eq!(prescan(b"\0\0\0<\0\0\0a"), Err(Rejection::Encoding));
        assert_eq!(
            prescan(b"<?xml version='1.0' encoding='UTF-16'?><a/>"),
            Err(Rejection::Encoding)
        );
    }

    #[test]
    fn accepts_comments_cdata_quotes_and_latin1() {
        let ok = br#"<?xml version="1.0" encoding="ISO-8859-1"?>
<!-- <!DOCTYPE dentro de un comentario --><a x="1>2 = 3" y='<!x'><![CDATA[<!DOCTYPE a=b=c>]]><?pi <!x?><b/></a>"#;
        assert_eq!(prescan(ok), Ok(()));
        let few: String = (0..MAX_ATTRIBUTES_PER_ELEMENT)
            .map(|i| format!(" a{i}=''"))
            .collect();
        assert_eq!(prescan(format!("<a{few}/>").as_bytes()), Ok(()));
    }

    #[test]
    fn utf16_is_decoded_before_scanning() {
        let encode = |text: &str, big_endian: bool| -> Vec<u8> {
            text.encode_utf16()
                .flat_map(|u| {
                    if big_endian {
                        u.to_be_bytes()
                    } else {
                        u.to_le_bytes()
                    }
                })
                .collect()
        };
        for big_endian in [false, true] {
            let dtd = encode("<!DOCTYPE a [<!ENTITY x 'y'>]><a/>", big_endian);
            assert_eq!(prescan(&dtd), Err(Rejection::Doctype));
            let bom: &[u8] = if big_endian {
                &[0xFE, 0xFF]
            } else {
                &[0xFF, 0xFE]
            };
            let mut with_bom = bom.to_vec();
            with_bom.extend(encode("<a b='1'/>", big_endian));
            assert_eq!(prescan(&with_bom), Ok(()));
        }
    }
}
