//! Codificaciones declaradas en `<?xml … encoding="…"?>`.
//!
//! libxml2 convierte con iconv. Aquí se
//! reproducen las codificaciones de un byte habituales en documentos en
//! español: ISO-8859-1, US-ASCII, windows-1252 e ISO-8859-15, con sus alias
//! y sin distinguir mayúsculas. Donde el iconv de glibc (servidores Linux) y
//! el libiconv de macOS difieren, se sigue a glibc. Al serializar el documento
//! completo, como `saveXML()`, un carácter que la codificación no puede
//! representar se escribe como referencia decimal `&#N;`, igual que el
//! respaldo de `xmlCharEncOutput`.

use std::borrow::Cow;
use std::fmt::Write as _;

use crate::error::XmlError;

/// Codificación de entrada y salida de un documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Charset {
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
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        // Los alias son ASCII en mayúsculas: comparar sin distinguir
        // mayúsculas equivale a pasar el nombre a mayúsculas, sin copiarlo.
        let is = |list: &[&str]| list.iter().any(|alias| alias.eq_ignore_ascii_case(name));
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

    fn encode_char(self, c: char) -> Option<u8> {
        match self {
            Self::Utf8 => None,
            Self::Latin1 => u8::try_from(u32::from(c)).ok(),
            Self::Ascii => u8::try_from(u32::from(c)).ok().filter(u8::is_ascii),
            // Los caracteres reasignados están en 0x80–0xBF.
            Self::Windows1252 | Self::Latin9 => u8::try_from(u32::from(c))
                .ok()
                .filter(|b| self.decode_byte(*b) == Some(c))
                .or_else(|| (0x80..=0xBF).find(|b| self.decode_byte(*b) == Some(c))),
        }
    }

    /// Bytes de entrada → texto. En UTF-8 se valida y se presta, sin copiar.
    pub(crate) fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, XmlError> {
        let invalid = || XmlError::Malformed("bytes inválidos para la codificación".into());
        if self == Self::Utf8 {
            return std::str::from_utf8(bytes)
                .map(Cow::Borrowed)
                .map_err(|_| invalid());
        }
        bytes
            .iter()
            .map(|b| self.decode_byte(*b))
            .collect::<Option<String>>()
            .map(Cow::Owned)
            .ok_or_else(invalid)
    }

    /// Sustituye por `&#N;` los caracteres que la codificación no admite. En
    /// UTF-8 devuelve el mismo texto, sin copiarlo.
    pub(crate) fn escape_unencodable(self, text: String) -> String {
        if self == Self::Utf8 {
            return text;
        }
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            if self.encode_char(c).is_some() {
                out.push(c);
            } else {
                let _ = write!(out, "&#{};", u32::from(c));
            }
        }
        out
    }

    /// Texto (ya escapado con [`Self::escape_unencodable`]) → bytes. En UTF-8
    /// reutiliza el búfer del texto.
    pub(crate) fn encode(self, text: String) -> Vec<u8> {
        if self == Self::Utf8 {
            return text.into_bytes();
        }
        let mut out = Vec::with_capacity(text.len());
        for c in text.chars() {
            match self.encode_char(c) {
                Some(b) => out.push(b),
                None => out.extend_from_slice(format!("&#{};", u32::from(c)).as_bytes()),
            }
        }
        out
    }
}

/// `EncName ::= [A-Za-z] ([A-Za-z0-9._] | '-')*`; libxml2 rechaza el
/// documento si el nombre no cumple la gramática.
pub(crate) fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Valor de `encoding` en la declaración XML inicial, leída en ASCII (todas
/// las codificaciones admitidas lo son en ese tramo).
pub(crate) fn declared(bytes: &[u8]) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_and_roundtrip() {
        assert_eq!(Charset::from_name("latin1"), Some(Charset::Latin1));
        assert_eq!(Charset::from_name("cp1252"), Some(Charset::Windows1252));
        assert_eq!(Charset::from_name("Latin-9"), Some(Charset::Latin9));
        assert_eq!(Charset::from_name("ISO LATIN 1"), None);
        assert_eq!(Charset::Latin1.decode(b"\xE9").as_deref(), Ok("é"));
        assert!(Charset::Ascii.decode(b"\xE9").is_err());
        assert!(Charset::Windows1252.decode(b"\x81").is_err());
        assert_eq!(Charset::Windows1252.encode("\u{81}".into()), b"&#129;");
        assert_eq!(Charset::Windows1252.decode(b"\x80").as_deref(), Ok("€"));
        assert_eq!(Charset::Latin9.decode(b"\xA4").as_deref(), Ok("€"));
        assert_eq!(Charset::from_name("LATIN9"), Some(Charset::Latin9));
        let text = Charset::Latin1.escape_unencodable("é€\u{10000}".into());
        assert_eq!(text, "é&#8364;&#65536;");
        assert_eq!(Charset::Latin1.encode(text), b"\xE9&#8364;&#65536;");
        assert_eq!(Charset::Latin9.escape_unencodable("¤€".into()), "&#164;€");
    }

    #[test]
    fn declaration() {
        assert_eq!(
            declared(b"<?xml version=\"1.0\" encoding='ISO-8859-1'?><r/>").as_deref(),
            Some("ISO-8859-1")
        );
        assert_eq!(
            declared(b"<?xml version=\"1.0\"?><r encoding=\"x\"/>"),
            None
        );
        assert_eq!(declared(b"<r/>"), None);
        assert!(valid_name("ISO-8859-1") && valid_name("latin1"));
        assert!(
            !valid_name("ISO_8859-1:1987") && !valid_name("ISO LATIN 1") && !valid_name("1abc")
        );
    }
}
