//! Codificación de solicitudes y conversión tolerante de respuestas.
//!
//! Estas reglas fijan los bytes que se envían (rutas, formularios y JSON) y
//! cómo se leen los valores JSON recibidos. Forman parte del contrato de
//! interoperabilidad con la API GRE:
//!
//! - [`percent_encode`] y [`build_query`]: codificación RFC 3986 de segmentos
//!   de ruta y formularios; solo `A-Z a-z 0-9 - _ . ~` quedan literales y el
//!   espacio es `%20`, nunca `+`.
//! - [`encode_compact_json`] y [`encode_pretty_json`]: JSON compacto o con
//!   sangría de 4 espacios; escapan `/` como `\/` y todo carácter no ASCII
//!   como `\uXXXX` (UTF-16), de modo que el cuerpo es ASCII puro.
//! - [`decode_json_or_null`]: un cuerpo inválido, vacío o no UTF-8 es `null`.
//! - [`coerce_string`] y [`coerce_int`]: conversión permisiva de cualquier
//!   valor JSON a cadena o a entero de 64 bits.
//! - [`float_to_string`]: `float` a cadena con 14 cifras significativas
//!   (`precision=14`).

#![allow(
    clippy::float_arithmetic,
    reason = "conversiones de float con formato y truncamiento fijados"
)]

use std::fmt::Write as _;

use serde_json::{Map, Value};

/// Valor recibido que no admite la conversión pedida. `class` y `message`
/// se exponen tal cual en el [`crate::ApiError`] resultante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodeError {
    pub(crate) class: &'static str,
    pub(crate) message: String,
}

/// Codificación RFC 3986: todo salvo `A-Z a-z 0-9 - _ . ~` como `%XX` en
/// mayúsculas, byte a byte sobre UTF-8.
#[must_use]
pub(crate) fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Formulario `clave=valor` unido con `&`, ambos con [`percent_encode`], en
/// el orden recibido.
#[must_use]
pub(crate) fn build_query(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn escape_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || !c.is_ascii() => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Valor JSON que se serializa; los objetos conservan el orden de inserción.
#[derive(Debug, Clone, PartialEq)]
#[allow(
    unreachable_pub,
    reason = "visible solo por el trait sellado de los modelos"
)]
pub enum Json {
    Null,
    Str(String),
    Int(i64),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// JSON compacto, sin espacios.
pub(crate) fn encode_compact_json(value: &Json) -> String {
    let mut out = String::new();
    write_json(&mut out, value, None, 0);
    out
}

/// JSON con un elemento por línea, sangría de 4 espacios y `": "` entre
/// clave y valor; los contenedores vacíos quedan como `[]` y `{}`.
pub(crate) fn encode_pretty_json(value: &Json) -> String {
    let mut out = String::new();
    write_json(&mut out, value, Some(4), 0);
    out
}

fn write_json(out: &mut String, value: &Json, indent: Option<usize>, level: usize) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if let Some(n) = indent {
                    out.push('\n');
                    out.push_str(&" ".repeat(n * (level + 1)));
                }
                write_json(out, v, indent, level + 1);
            }
            if let Some(n) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(n * level));
            }
            out.push(']');
        }
        Json::Str(s) => escape_json_string(out, s),
        Json::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Json::Object(fields) => {
            if fields.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, v)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if let Some(n) = indent {
                    out.push('\n');
                    out.push_str(&" ".repeat(n * (level + 1)));
                }
                escape_json_string(out, k);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_json(out, v, indent, level + 1);
            }
            if let Some(n) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(n * level));
            }
            out.push('}');
        }
    }
}

/// Decodifica un cuerpo JSON; inválido, vacío o no UTF-8 es `null`.
#[must_use]
pub(crate) fn decode_json_or_null(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

/// `float` a cadena con `precision=14`: 14 cifras significativas, notación científica si el exponente decimal es menor que
/// -4 o mayor o igual que 14, y `.0` en la mantisa de un solo dígito.
#[must_use]
pub(crate) fn float_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NAN".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "INF" } else { "-INF" }.to_owned();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    // 14 cifras significativas en notación científica de Rust: "d.ddde±x".
    let sci = format!("{:.13e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let decpt = exp + 1;
    let sign = if x < 0.0 { "-" } else { "" };
    if !(-3..=14).contains(&decpt) {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        let e = decpt - 1;
        return format!(
            "{sign}{first}.{rest}E{}{}",
            if e < 0 { '-' } else { '+' },
            e.abs()
        );
    }
    let n = usize::try_from(decpt.max(0)).unwrap_or(0);
    let mut out = String::from(sign);
    if decpt <= 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat(usize::try_from(-decpt).unwrap_or(0)));
        out.push_str(digits);
    } else if digits.len() <= n {
        out.push_str(digits);
        out.push_str(&"0".repeat(n - digits.len()));
    } else {
        out.push_str(&digits[..n]);
        out.push('.');
        out.push_str(&digits[n..]);
    }
    out
}

/// Valor JSON a cadena: `null` y `false` son `""`, `true` es `"1"`, los
/// números enteros en decimal y los demás con [`float_to_string`], una lista
/// es `"Array"` y un objeto es un error.
pub(crate) fn coerce_string(value: &Value) -> Result<String, DecodeError> {
    Ok(match value {
        Value::Null => String::new(),
        Value::Bool(true) => "1".to_owned(),
        Value::Bool(false) => String::new(),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => i.to_string(),
            // Un entero fuera de i64 se trata como float.
            (None, Some(u)) => float_to_string(u as f64),
            _ => float_to_string(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => s.clone(),
        Value::Array(_) => "Array".to_owned(),
        Value::Object(_) => {
            return Err(DecodeError {
                class: "Error",
                message: "Object of class stdClass could not be converted to string".to_owned(),
            });
        }
    })
}

/// Entero al comienzo de una cadena: espacios iniciales, prefijo numérico (con
/// decimales y exponente), saturación en el rango de `int`; si no hay
/// prefijo numérico, `0`.
#[must_use]
pub(crate) fn int_from_str_prefix(s: &str) -> i64 {
    let t = s.trim_start_matches([' ', '\t', '\n', '\r', '\u{b}', '\u{c}']);
    let bytes = t.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let int_end = i;
    let mut is_float = false;
    if i < bytes.len() && bytes[i] == b'.' {
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > i + 1 || int_end > int_start {
            is_float = j > i + 1;
            i = j;
        }
    }
    if int_end == int_start && !is_float {
        return 0;
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
        if j > exp_start {
            is_float = true;
            i = j;
        }
    }
    if is_float {
        return float_to_int(t[..i].parse::<f64>().unwrap_or(0.0));
    }
    let digits = &t[..int_end];
    digits.parse::<i64>().unwrap_or_else(|_| {
        if digits.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// `float` a entero: trunca; fuera de rango, NaN o infinito da `0`.
#[must_use]
pub(crate) fn float_to_int(x: f64) -> i64 {
    if !x.is_finite() || !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&x) {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation, reason = "rango comprobado arriba")]
    let v = x.trunc() as i64;
    v
}

/// Valor JSON a entero: `null` y `false` son 0, `true` es 1, un número no
/// entero o fuera de `i64` usa [`float_to_int`], una cadena usa
/// [`int_from_str_prefix`], una lista vacía es 0 y cualquier otra lista u
/// objeto es 1.
pub(crate) fn coerce_int(value: &Value) -> i64 {
    match value {
        Value::Null | Value::Bool(false) => 0,
        Value::Bool(true) => 1,
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => i,
            (None, Some(u)) => float_to_int(u as f64),
            _ => float_to_int(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => int_from_str_prefix(s),
        Value::Array(a) => i64::from(!a.is_empty()),
        Value::Object(_) => 1,
    }
}

/// Valor «vacío»: `null`, `false`, número cero, `""`, `"0"` o lista vacía.
/// Un objeto nunca es vacío.
pub(crate) fn is_empty_value(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => true,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty() || s == "0",
        Value::Array(a) => a.is_empty(),
        Value::Object(_) => false,
        Value::Bool(true) => false,
    }
}

/// Propiedades de un valor que se lee como modelo: una cadena se vuelve a
/// decodificar como JSON; un arreglo se convierte en objeto con
/// claves numéricas; otro escalar no tiene propiedades.
pub(crate) fn model_source(value: &Value) -> Option<Map<String, Value>> {
    let decoded;
    let value = match value {
        Value::String(s) => {
            decoded = decode_json_or_null(s.as_bytes());
            &decoded
        }
        other => other,
    };
    object_source(value)
}

/// Propiedades de un valor ya decodificado: un objeto, o un arreglo convertido en objeto con
/// claves numéricas; cualquier otro valor no tiene propiedades.
pub(crate) fn object_source(value: &Value) -> Option<Map<String, Value>> {
    match value {
        Value::Object(m) => Some(m.clone()),
        Value::Array(items) => Some(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v.clone()))
                .collect(),
        ),
        _ => None,
    }
}

/// Campo presente y distinto de `null`.
pub(crate) fn present_field<'a>(
    source: Option<&'a Map<String, Value>>,
    field: &str,
) -> Option<&'a Value> {
    source.and_then(|m| m.get(field)).filter(|v| !v.is_null())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_with_14_significant_digits() {
        for (x, s) in [
            (1.5, "1.5"),
            (1.0, "1"),
            (0.1 + 0.2, "0.3"),
            (1e15, "1.0E+15"),
            (1e14, "1.0E+14"),
            (123_456_789_012_345.0, "1.2345678901234E+14"),
            (1_234_567_890_123_456_789.0, "1.2345678901235E+18"),
            (0.0001, "0.0001"),
            (0.00001, "1.0E-5"),
            (-2.5, "-2.5"),
            (1e100, "1.0E+100"),
            (3600.0, "3600"),
            (1.0e-7, "1.0E-7"),
        ] {
            assert_eq!(float_to_string(x), s, "{x}");
        }
    }

    #[test]
    fn int_from_numeric_prefix() {
        for (s, i) in [
            ("3600", 3600),
            (" 12", 12),
            ("12abc", 12),
            ("1e3", 1000),
            ("0x1A", 0),
            ("", 0),
            ("abc", 0),
            ("99999999999999999999", i64::MAX),
            ("-5", -5),
            ("12.7", 12),
            ("1.5e3x", 1500),
            ("\n7", 7),
        ] {
            assert_eq!(int_from_str_prefix(s), i, "{s:?}");
        }
    }

    #[test]
    fn ascii_json_and_rfc3986() {
        assert_eq!(percent_encode("a b/ñ~"), "a%20b%2F%C3%B1~");
        let v = Json::Object(vec![("u".into(), Json::Str("https://x/ñ😀".into()))]);
        assert_eq!(
            encode_compact_json(&v),
            r#"{"u":"https:\/\/x\/\u00f1\ud83d\ude00"}"#
        );
        assert_eq!(encode_pretty_json(&Json::Object(vec![])), "{}");
    }
}
