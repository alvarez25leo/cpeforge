//! Filtros para interpolar valores en
//! consultas XPath.

/// `XPath::ALPHANUMERIC`.
pub const ALPHANUMERIC: &str = "\\w\\d";
/// `XPath::NUMERIC`.
pub const NUMERIC: &str = "\\d";
/// `XPath::LETTERS`.
pub const LETTERS: &str = "\\w";
/// `XPath::EXTENDED_ALPHANUMERIC` (valor por defecto de [`filter_attr_name`]).
pub const EXTENDED_ALPHANUMERIC: &str = "\\w\\d\\s\\-_:\\.";
/// `XPath::SINGLE_QUOTE`.
pub const SINGLE_QUOTE: &str = "'";
/// `XPath::DOUBLE_QUOTE`.
pub const DOUBLE_QUOTE: &str = "\"";
/// `XPath::ALL_QUOTES` (valor por defecto de [`filter_attr_value`]).
pub const ALL_QUOTES: &str = "['\"]";

/// Clase de caracteres PCRE simple, sin modo UTF-8 (opera por bytes ASCII).
fn class_matches(class: &str, byte: u8) -> bool {
    let mut chars = class.chars().peekable();
    while let Some(c) = chars.next() {
        let hit = if c == '\\' {
            match chars.next() {
                Some('w') => byte.is_ascii_alphanumeric() || byte == b'_',
                Some('d') => byte.is_ascii_digit(),
                Some('s') => matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C),
                Some(other) => other.is_ascii() && byte == other as u8,
                None => false,
            }
        } else {
            c.is_ascii() && byte == c as u8
        };
        if hit {
            return true;
        }
    }
    false
}

/// `XPath::filterAttrValue`: elimina las comillas indicadas.
///
/// `quotes` es una clase PCRE (`['"]`) o un carácter literal (`'` o `"`).
#[must_use]
pub fn filter_attr_value(value: &str, quotes: &str) -> String {
    let class = quotes
        .strip_prefix('[')
        .and_then(|q| q.strip_suffix(']'))
        .unwrap_or(quotes);
    let bytes: Vec<u8> = value
        .bytes()
        .filter(|b| !class_matches(class, *b))
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `XPath::filterAttrName`: conserva solo los bytes de la clase permitida.
///
/// Sin modo UTF-8, los bytes no ASCII se eliminan uno a uno.
#[must_use]
pub fn filter_attr_name(name: &str, allow: &str) -> String {
    let bytes: Vec<u8> = name
        .bytes()
        .filter(|b| b.is_ascii() && class_matches(allow, *b))
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
