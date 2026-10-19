//! Interpretación de códigos de respuesta SUNAT.
//!
//! `CdrResponse::isAccepted()` de la referencia convierte el código con
//! `(int)` y considera aceptado `0` o cualquier valor `>= 4000` (observaciones).
//! La conversión `(int)` de la referencia tiene reglas propias (prefijo numérico,
//! saturación, `0` para texto); [`int_cast`] las reproduce para que la
//! decisión de aceptación sea idéntica.

/// Resultado de `isAccepted()` para un código CDR.
///
/// `None` y los textos no numéricos valen `0` y, por tanto, se consideran
/// aceptados, igual que en la referencia. Es una regla de compatibilidad, no una
/// decisión tributaria segura: ver [`crate::model::response::CdrResponse::is_accepted`].
#[must_use]
pub fn is_accepted_code(code: Option<&str>) -> bool {
    let value = code.map_or(0, int_cast);
    value == 0 || value >= 4000
}

/// Conversión de cadena a entero equivalente a `(int) $string` en la referencia
/// (plataforma de 64 bits).
///
/// Reglas reproducidas:
/// - se omiten espacios iniciales ASCII (`" \t\n\r\x0B\x0C"`);
/// - se toma el prefijo numérico más largo (`[+-]? (dígitos [. dígitos*] | . dígitos) ([eE][+-]? dígitos)?`)
///   y se ignora el resto; sin prefijo el resultado es `0`;
/// - un entero que cabe en `i64` se devuelve exacto;
/// - con parte decimal, exponente o desbordamiento se interpreta como `double`
///   y se trunca hacia cero, saturando a `i64::MIN`/`i64::MAX`; infinito vale `0`.
#[must_use]
pub fn int_cast(text: &str) -> i64 {
    let bytes = text.as_bytes();
    let mut start = 0;
    while start < bytes.len() && matches!(bytes[start], b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
    {
        start += 1;
    }
    let mut end = start;
    if end < bytes.len() && matches!(bytes[end], b'+' | b'-') {
        end += 1;
    }
    let int_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    let int_digits = end - int_start;
    let mut is_double = false;
    let mut frac_digits = 0;
    if end < bytes.len() && bytes[end] == b'.' {
        let mut probe = end + 1;
        while probe < bytes.len() && bytes[probe].is_ascii_digit() {
            probe += 1;
        }
        frac_digits = probe - end - 1;
        if int_digits > 0 || frac_digits > 0 {
            is_double = true;
            end = probe;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return 0;
    }
    if end < bytes.len() && matches!(bytes[end], b'e' | b'E') {
        let mut probe = end + 1;
        if probe < bytes.len() && matches!(bytes[probe], b'+' | b'-') {
            probe += 1;
        }
        let exp_start = probe;
        while probe < bytes.len() && bytes[probe].is_ascii_digit() {
            probe += 1;
        }
        if probe > exp_start {
            is_double = true;
            end = probe;
        }
    }
    // El prefijo solo contiene ASCII, así que el corte es un límite de carácter válido.
    let number = &text[start..end];
    if !is_double {
        if let Ok(value) = number.parse::<i64>() {
            return value;
        }
    }
    double_to_int(number)
}

/// `zend_dval_to_lval_cap`: trunca hacia cero, satura fuera de rango y
/// devuelve `0` para valores no finitos.
///
/// Se usa `f64` deliberadamente: se replica la semántica binaria de la referencia para
/// un código de respuesta, no se calcula ningún importe.
#[allow(clippy::float_arithmetic)]
fn double_to_int(number: &str) -> i64 {
    let Ok(value) = number.parse::<f64>() else {
        return 0;
    };
    if !value.is_finite() {
        return 0;
    }
    // 2^63 es exactamente representable; `i64::MAX as f64` también vale 2^63.
    const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;
    if value >= TWO_POW_63 {
        i64::MAX
    } else if value < -TWO_POW_63 {
        i64::MIN
    } else {
        // Dentro de [-2^63, 2^63) la conversión con `as` trunca sin saturar.
        #[allow(clippy::cast_possible_truncation)]
        let truncated = value.trunc() as i64;
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_matches_probe_values() {
        // Valores obtenidos con la referencia `(int) $s` (ver tests/reference/core).
        let cases: &[(&str, i64)] = &[
            ("9223372036854775808", i64::MAX),
            ("1e19", i64::MAX),
            ("-1e19", i64::MIN),
            ("-9223372036854775809", i64::MIN),
            ("-9223372036854775808", i64::MIN),
            ("1e400", 0),
            ("-1e400", 0),
            ("9223372036854775807.5", i64::MAX),
            ("9.2233720368547748e18", 9_223_372_036_854_774_784),
            (" 1", 1),
            ("\u{a0}1", 0),
            ("1abc", 1),
            ("1e", 1),
            ("1e+", 1),
            ("+-1", 0),
            (".", 0),
            ("1.", 1),
            ("-.5e1", -5),
            ("0x1A", 0),
            ("1_000", 1),
            ("\u{661}", 0),
            ("1e-400", 0),
            ("-0.9", 0),
            ("  -3999.99", -3999),
            ("0.4e4", 4000),
            ("\u{0}1", 0),
            ("", 0),
            ("-", 0),
        ];
        for (text, expected) in cases {
            assert_eq!(int_cast(text), *expected, "entrada {text:?}");
        }
    }

    #[test]
    fn accepted_codes() {
        assert!(is_accepted_code(None));
        assert!(is_accepted_code(Some("0")));
        assert!(is_accepted_code(Some("4000")));
        assert!(!is_accepted_code(Some("3999")));
        assert!(!is_accepted_code(Some("-1")));
        assert!(is_accepted_code(Some("abc")));
    }
}
