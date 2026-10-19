//! Semántica de la referencia de `php:function`/`php:functionString` limitada a la única
//! función que registra `XslValidator`:
//! `CPEForge\Validator\Xml\XsltFunctions::matches(?string $pattern, ?string $input)`.
//!
//! Reproduce, con los resultados del oráculo (tests/reference/cpe-validator):
//! - el despacho de `ext/dom/xpath_callbacks.c` de la referencia (nombre como
//!   primer argumento, conversión de argumentos XPath y errores
//!   `Error`/`TypeError`/`ArgumentCountError`);
//! - la coerción débil de parámetros `?string` (bool → `"1"`/`""`, número →
//!   cadena con `precision=14`, conjunto de nodos → `array`, que falla);
//! - `str_replace('/', '\/', $pattern)` y `preg_match('/'.$pattern.'/', $input)`
//!   con PCRE2 en modo de bytes (sin `/u`), `PCRE2_EXTRA_ALLOW_LOOKAROUND_BSK`
//!   y los límites `pcre.backtrack_limit`/`pcre.recursion_limit`.
//!
//! El resultado vuelve a XPath como la referencia: `"1"`/`"0"` (cadena, porque la referencia
//! convierte el `int` con `zval_get_string`) o `false` (booleano) si
//! `preg_match` falla. Las diferencias del perfil seguro (patrón inválido o
//! límite de PCRE como error tipado) se aplican al final de la
//! transformación, no aquí.

use crate::ffi::{self, PcreResult, Session};

/// Nombre exacto con el que `XslValidator` registra la función (sensible a
/// mayúsculas, sin barra inicial).
pub const MATCHES_HANDLER: &str = "CPEForge\\Validator\\Xml\\XsltFunctions::matches";

/// Argumento XPath recibido por `php:function`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum XPathArg {
    String(String),
    Boolean(bool),
    Number(f64),
    /// Conjunto de nodos en `php:function` (la referencia lo convierte en `array`).
    NodeSet,
    /// Primer argumento que no es una cadena XPath.
    NotString,
}

/// Excepción que la referencia lanzaría al terminar `transformToXml()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtensionCall {
    pub(crate) class: &'static str,
    pub(crate) message: String,
}

/// Fallo de `preg_match` que la referencia notifica con un aviso y `false`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegexFailure {
    pub(crate) pattern: String,
    pub(crate) message: String,
}

/// Qué se deja en la pila XPath.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Push {
    /// `preg_match` devolvió 1 o 0 (cadena `"1"`/`"0"`).
    Matched(bool),
    /// `preg_match` devolvió `false`.
    False,
    /// La función de usuario lanzó: la referencia no apila nada.
    Nothing,
    /// Error del despacho: la referencia apila la cadena vacía.
    Sentinel,
}

fn throw(session: &mut Session, class: &'static str, message: String) {
    // zend_throw_error con una excepción pendiente la encadena como
    // `previous` y la nueva pasa a ser la visible.
    session.extension = Some(ExtensionCall { class, message });
}

/// Despacha una llamada `php:function` (o `functionString`).
pub(crate) fn dispatch(session: &mut Session, _string_mode: bool, args: &[XPathArg]) -> Push {
    let Some(first) = args.first() else {
        throw(
            session,
            "Error",
            "Function name must be passed as the first argument".to_owned(),
        );
        return Push::Sentinel;
    };
    let XPathArg::String(name) = first else {
        throw(
            session,
            "TypeError",
            "Handler name must be a string".to_owned(),
        );
        return Push::Sentinel;
    };
    if name != MATCHES_HANDLER {
        throw(
            session,
            "Error",
            format!("No callback handler \"{name}\" registered"),
        );
        return Push::Sentinel;
    }
    // Con una excepción pendiente zend_call_function no ejecuta la función.
    if session.extension.is_some() {
        return Push::Nothing;
    }
    let params = &args[1..];
    let mut coerced: [String; 2] = [String::new(), String::new()];
    for (index, param) in ["pattern", "input"].iter().enumerate() {
        let Some(arg) = params.get(index) else {
            throw(
                session,
                "ArgumentCountError",
                format!(
                    "Too few arguments to function {MATCHES_HANDLER}(), {} passed and exactly 2 expected",
                    params.len()
                ),
            );
            return Push::Nothing;
        };
        coerced[index] = match arg {
            XPathArg::String(text) => text.clone(),
            XPathArg::Boolean(true) => "1".to_owned(),
            XPathArg::Boolean(false) => String::new(),
            XPathArg::Number(number) => float_to_string(*number),
            XPathArg::NodeSet | XPathArg::NotString => {
                throw(
                    session,
                    "TypeError",
                    format!(
                        "{MATCHES_HANDLER}(): Argument #{} (${param}) must be of type ?string, array given",
                        index + 1
                    ),
                );
                return Push::Nothing;
            }
        };
    }
    let [pattern, input] = coerced;
    match matches(&pattern, &input, session.match_limit, session.depth_limit) {
        Ok(found) => Push::Matched(found),
        Err(MatchFailure::Limit) => {
            session.regex_limits = session.regex_limits.saturating_add(1);
            Push::False
        }
        Err(MatchFailure::Invalid(message)) => {
            session.regex_errors.push(RegexFailure { pattern, message });
            Push::False
        }
    }
}

/// Por qué `preg_match` devolvió `false`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MatchFailure {
    /// Aviso de la referencia (delimitador, modificador o compilación).
    Invalid(String),
    /// Límite de retroceso/recursión (sin aviso en la referencia).
    Limit,
}

/// `XsltFunctions::matches`: `preg_match('/'.str_replace('/', '\/', $p).'/', $input)`.
pub(crate) fn matches(
    pattern: &str,
    input: &str,
    match_limit: u32,
    depth_limit: u32,
) -> Result<bool, MatchFailure> {
    let regex = format!("/{}/", pattern.replace('/', "\\/"));
    let bytes = regex.as_bytes();
    // pcre_get_compiled_regex_cache_ex: busca el delimitador final sin escapar.
    let end = bytes.len();
    let mut pos = 1;
    while pos < end {
        if bytes[pos] == b'\\' && pos + 1 < end {
            pos += 1;
        } else if bytes[pos] == b'/' {
            break;
        }
        pos += 1;
    }
    if pos >= end {
        return Err(MatchFailure::Invalid(
            "No ending delimiter '/' found".to_owned(),
        ));
    }
    // Tras un delimitador anticipado, el '/' añadido al final nunca es un
    // modificador válido: la referencia avisa del primero que no reconoce.
    for &c in &bytes[pos + 1..] {
        if !matches!(
            c,
            b'i' | b'm'
                | b'n'
                | b's'
                | b'x'
                | b'A'
                | b'D'
                | b'r'
                | b'S'
                | b'X'
                | b'U'
                | b'u'
                | b'J'
                | b' '
                | b'\n'
                | b'\r'
        ) {
            return Err(MatchFailure::Invalid(if c == 0 {
                "NUL byte is not a valid modifier".to_owned()
            } else {
                format!("Unknown modifier '{}'", char::from(c))
            }));
        }
    }
    match ffi::pcre_match(&bytes[1..pos], input.as_bytes(), match_limit, depth_limit) {
        PcreResult::Matched(found) => Ok(found),
        PcreResult::CompileError { message, offset } => Err(MatchFailure::Invalid(format!(
            "Compilation failed: {message} at offset {offset}"
        ))),
        PcreResult::Limit => Err(MatchFailure::Limit),
        PcreResult::MatchError(code) => {
            Err(MatchFailure::Invalid(format!("Internal PCRE error {code}")))
        }
    }
}

/// Conversión `float → string` de la referencia con `precision=14` (`zend_gcvt`).
pub(crate) fn float_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NAN".to_owned();
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-INF".to_owned()
        } else {
            "INF".to_owned()
        };
    }
    const NDIGIT: i32 = 14;
    let negative = value.is_sign_negative();
    // zend_dtoa(value, mode 2, 14): dígitos significativos sin ceros finales
    // y posición del punto decimal.
    let (digits, decpt): (Vec<u8>, i32) = if value == 0.0 {
        (vec![b'0'], 1)
    } else {
        let formatted = format!("{:.13e}", value.abs());
        let (mantissa, exponent) = formatted
            .split_once('e')
            .unwrap_or((formatted.as_str(), "0"));
        let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
        while digits.len() > 1 && digits.last() == Some(&b'0') {
            digits.pop();
        }
        (digits, exponent.parse::<i32>().unwrap_or(0) + 1)
    };
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if !(-3..=NDIGIT).contains(&decpt) {
        let exponent = decpt - 1;
        out.push(char::from(digits[0]));
        out.push('.');
        if digits.len() == 1 {
            out.push('0');
        } else {
            out.extend(digits[1..].iter().map(|&d| char::from(d)));
        }
        out.push('E');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.unsigned_abs().to_string());
    } else if decpt < 0 {
        out.push_str("0.");
        for _ in decpt..0 {
            out.push('0');
        }
        out.extend(digits.iter().map(|&d| char::from(d)));
    } else {
        let point = usize::try_from(decpt).unwrap_or(0);
        for i in 0..point {
            out.push(digits.get(i).map_or('0', |&d| char::from(d)));
        }
        if digits.len() > point {
            if point == 0 {
                out.push('0');
            }
            out.push('.');
            out.extend(digits[point..].iter().map(|&d| char::from(d)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_formatting_uses_precision_14() {
        let cases: &[(f64, &str)] = &[
            (1.0, "1"),
            (0.1, "0.1"),
            (1.0 / 3.0, "0.33333333333333"),
            (2.0 / 3.0, "0.66666666666667"),
            (1e14, "1.0E+14"),
            (1e13, "10000000000000"),
            (1e15, "1.0E+15"),
            (123_456_789_012_345_678.0, "1.2345678901235E+17"),
            (0.0001, "0.0001"),
            (0.00001, "1.0E-5"),
            (0.000_012_345, "1.2345E-5"),
            (-0.0, "-0"),
            (0.0, "0"),
            (-1.5, "-1.5"),
            (0.5, "0.5"),
            (12_345_678_901_234.5, "12345678901234"),
            (99_999_999_999_999.5, "1.0E+14"),
            (0.1 + 0.2, "0.3"),
            (f64::NAN, "NAN"),
            (f64::INFINITY, "INF"),
            (f64::NEG_INFINITY, "-INF"),
            (1e100, "1.0E+100"),
            (1.5e-300, "1.5E-300"),
        ];
        for (value, expected) in cases {
            assert_eq!(float_to_string(*value), *expected, "{value:e}");
        }
    }

    #[test]
    fn delimiter_and_modifier_errors_are_reported() {
        let m = |p: &str, s: &str| matches(p, s, 1_000_000, 100_000);
        assert_eq!(m("a/b", "a/b"), Ok(true));
        assert_eq!(
            m("a\\/i", "A"),
            Err(MatchFailure::Invalid("Unknown modifier '/'".into()))
        );
        assert_eq!(
            m("a\\", "a"),
            Err(MatchFailure::Invalid(
                "No ending delimiter '/' found".into()
            ))
        );
        assert_eq!(
            m("x\\/ is", "x"),
            Err(MatchFailure::Invalid("Unknown modifier '/'".into()))
        );
        assert_eq!(
            m("x\\/e", "x"),
            Err(MatchFailure::Invalid("Unknown modifier 'e'".into()))
        );
        assert_eq!(
            m("(", "a"),
            Err(MatchFailure::Invalid(
                "Compilation failed: missing closing parenthesis at offset 1".into()
            ))
        );
        assert_eq!(m("^.$", "ñ"), Ok(false));
        assert_eq!(m("^..$", "ñ"), Ok(true));
        assert_eq!(m("", "abc"), Ok(true));
        assert_eq!(
            m("^(a+)+$", &format!("{}b", "a".repeat(36))),
            Err(MatchFailure::Limit)
        );
    }
}
