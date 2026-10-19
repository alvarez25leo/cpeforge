//! Decodificación de la entrada a texto.
//!
//! UTF-16 se detecta por BOM o por los primeros bytes; las demás
//! codificaciones, por la declaración `encoding`. Se admiten UTF-8, UTF-16
//! (LE/BE) y las codificaciones de un byte
//! habituales en documentos en español: ISO-8859-1, US-ASCII, windows-1252 e
//! ISO-8859-15, con sus alias y sin distinguir mayúsculas. Donde el iconv de
//! glibc (servidores Linux) y el libiconv de macOS difieren, se sigue a glibc.
//! Las tablas son las de `cpeforge-xmldsig` (`src/encoding.rs`). Cualquier
//! otra codificación se rechaza.

use std::borrow::Cow;

use crate::error::XmlParserError;

/// Codificación de un byte o UTF-8 declarada en el prólogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Charset {
    Utf8,
    Latin1,
    Ascii,
    Windows1252,
    Latin9,
}

const UTF8: &[&str] = &["UTF-8", "UTF8"];
const LATIN1: &[&str] = &[
    "ISO-8859-1",
    "ISO_8859-1",
    "ISO8859-1",
    "ISO-IR-100",
    "ISO-LATIN-1",
    "LATIN1",
    "L1",
    "CP819",
    "IBM819",
    "CSISOLATIN1",
];
const ASCII: &[&str] = &[
    "US-ASCII",
    "ASCII",
    "ANSI_X3.4-1968",
    "ANSI_X3.4-1986",
    "ISO646-US",
    "ISO-IR-6",
    "CP367",
    "IBM367",
    "US",
    "CSASCII",
];
const WINDOWS_1252: &[&str] = &["WINDOWS-1252", "CP1252", "MS-ANSI"];
const LATIN9: &[&str] = &[
    "ISO-8859-15",
    "ISO_8859-15",
    "ISO8859-15",
    "ISO-IR-203",
    "LATIN-9",
    // glibc lo admite; el libiconv de macOS no.
    "LATIN9",
];

/// windows-1252 en 0x80–0x9F. Los cinco octetos sin asignar (`None`) son
/// inválidos, como en el iconv de glibc (servidores Linux) y en la tabla de
/// Unicode; el libiconv de macOS los lee como controles C1.
const WINDOWS_1252_HIGH: [Option<char>; 32] = [
    Some('\u{20AC}'),
    None,
    Some('\u{201A}'),
    Some('\u{0192}'),
    Some('\u{201E}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02C6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017D}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201C}'),
    Some('\u{201D}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02DC}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203A}'),
    Some('\u{0153}'),
    None,
    Some('\u{017E}'),
    Some('\u{0178}'),
];

/// Las ocho posiciones en que ISO-8859-15 difiere de ISO-8859-1.
const LATIN9_CHANGES: [(u8, char); 8] = [
    (0xA4, '\u{20AC}'),
    (0xA6, '\u{0160}'),
    (0xA8, '\u{0161}'),
    (0xB4, '\u{017D}'),
    (0xB8, '\u{017E}'),
    (0xBC, '\u{0152}'),
    (0xBD, '\u{0153}'),
    (0xBE, '\u{0178}'),
];

impl Charset {
    /// Codificación por nombre declarado; `None` si no está admitida.
    fn from_name(name: &str) -> Option<Self> {
        let upper = name.to_ascii_uppercase();
        let is = |list: &[&str]| list.contains(&upper.as_str());
        if is(UTF8) {
            Some(Self::Utf8)
        } else if is(LATIN1) {
            Some(Self::Latin1)
        } else if is(ASCII) {
            Some(Self::Ascii)
        } else if is(WINDOWS_1252) {
            Some(Self::Windows1252)
        } else if is(LATIN9) {
            Some(Self::Latin9)
        } else {
            None
        }
    }

    fn decode_byte(self, byte: u8) -> Option<char> {
        match self {
            Self::Utf8 => None,
            Self::Latin1 => Some(char::from(byte)),
            Self::Ascii => byte.is_ascii().then_some(char::from(byte)),
            Self::Windows1252 => match byte {
                0x80..=0x9F => WINDOWS_1252_HIGH
                    .get(usize::from(byte - 0x80))
                    .copied()
                    .flatten(),
                _ => Some(char::from(byte)),
            },
            Self::Latin9 => Some(
                LATIN9_CHANGES
                    .iter()
                    .find(|(b, _)| *b == byte)
                    .map_or(char::from(byte), |(_, c)| *c),
            ),
        }
    }

    /// Bytes de entrada → texto. UTF-8 válido se presta sin copiarlo.
    fn decode(self, bytes: &[u8]) -> Option<Cow<'_, str>> {
        if self == Self::Utf8 {
            return std::str::from_utf8(bytes).ok().map(Cow::Borrowed);
        }
        bytes
            .iter()
            .map(|b| self.decode_byte(*b))
            .collect::<Option<String>>()
            .map(Cow::Owned)
    }
}

/// `EncName ::= [A-Za-z] ([A-Za-z0-9._] | '-')*`; un nombre que no cumple la
/// gramática hace que el documento esté mal formado.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Valor de `encoding` en la declaración XML inicial, leída en ASCII (todas
/// las codificaciones admitidas lo son en ese tramo).
fn declared(bytes: &[u8]) -> Option<String> {
    let rest = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let rest = rest.strip_prefix(b"<?xml")?;
    if !rest.first().is_some_and(u8::is_ascii_whitespace) {
        return None;
    }
    let end = rest.windows(2).position(|w| w == b"?>")?;
    let decl = std::str::from_utf8(rest.get(..end)?).ok()?;
    let at = decl.find("encoding")?;
    let after = decl.get(at + "encoding".len()..)?.trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    let quote = after.chars().next().filter(|q| *q == '"' || *q == '\'')?;
    let value = after.get(1..)?;
    let close = value.find(quote)?;
    value.get(..close).map(str::to_owned)
}

/// Bytes de entrada → texto, según BOM o declaración `encoding`. Una entrada
/// UTF-8 (con o sin BOM) se devuelve prestada.
pub(crate) fn decode_input(bytes: &[u8]) -> Result<Cow<'_, str>, XmlParserError> {
    let invalid = |name: &str| XmlParserError::Encoding {
        name: name.to_owned(),
    };
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return Charset::Utf8.decode(rest).ok_or_else(|| invalid("UTF-8"));
    }
    let utf16 = if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        Some((rest, false))
    } else if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        Some((rest, true))
    } else if bytes.starts_with(b"<\0?\0") {
        Some((bytes, false))
    } else if bytes.starts_with(b"\0<\0?") {
        Some((bytes, true))
    } else {
        None
    };
    if let Some((rest, big_endian)) = utf16 {
        return decode_utf16(rest, big_endian)
            .map(Cow::Owned)
            .ok_or_else(|| invalid("UTF-16"));
    }
    let Some(name) = declared(bytes) else {
        return Charset::Utf8.decode(bytes).ok_or_else(|| invalid("UTF-8"));
    };
    if !valid_name(&name) {
        return Err(XmlParserError::Malformed {
            message: format!("nombre de codificación inválido: {name:?}"),
        });
    }
    Charset::from_name(&name)
        .and_then(|charset| charset.decode(bytes))
        .ok_or(XmlParserError::Encoding { name })
}

fn decode_utf16(bytes: &[u8], big_endian: bool) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units = bytes.chunks_exact(2).map(|pair| {
        let pair = [pair[0], pair[1]];
        if big_endian {
            u16::from_be_bytes(pair)
        } else {
            u16::from_le_bytes(pair)
        }
    });
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases() {
        assert_eq!(Charset::from_name("latin1"), Some(Charset::Latin1));
        assert_eq!(Charset::from_name("cp1252"), Some(Charset::Windows1252));
        assert_eq!(Charset::from_name("Latin-9"), Some(Charset::Latin9));
        assert_eq!(Charset::from_name("KOI8-R"), None);
        assert_eq!(Charset::Latin1.decode(b"\xE9").as_deref(), Some("é"));
        assert!(Charset::Ascii.decode(b"\xE9").is_none());
        assert!(Charset::Windows1252.decode(b"\x81").is_none());
        assert_eq!(Charset::Windows1252.decode(b"\x80").as_deref(), Some("€"));
        assert_eq!(Charset::Latin9.decode(b"\xA4").as_deref(), Some("€"));
    }

    #[test]
    fn detection() {
        let latin = b"<?xml version=\"1.0\" encoding='ISO-8859-1'?><r>\xE9</r>";
        assert_eq!(
            decode_input(latin).unwrap_or_default(),
            "<?xml version=\"1.0\" encoding='ISO-8859-1'?><r>é</r>"
        );
        assert_eq!(decode_input(b"\xEF\xBB\xBF<r/>").as_deref(), Ok("<r/>"));
        assert_eq!(decode_input(b"\xFF\xFE<\0r\0/\0>\0").as_deref(), Ok("<r/>"));
        assert_eq!(decode_input(b"\0<\0?\0x").as_deref(), Ok("<?x"));
        assert!(decode_input(b"\xFF\xFE<").is_err());
        assert!(decode_input(b"<r>\xFF</r>").is_err());
        assert!(matches!(
            decode_input(b"<?xml version=\"1.0\" encoding=\"KOI8-R\"?><r/>"),
            Err(XmlParserError::Encoding { .. })
        ));
        assert!(matches!(
            decode_input(b"<?xml version=\"1.0\" encoding=\"1x\"?><r/>"),
            Err(XmlParserError::Malformed { .. })
        ));
    }
}
