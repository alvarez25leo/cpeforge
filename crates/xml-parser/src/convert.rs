//! Conversiones de texto que aplican los parsers: importes y cantidades,
//! comparación numérica tolerante, partes de un identificador `serie-número` y
//! fechas ISO. Sus reglas (prefijo numérico, saturación, desbordamiento de
//! fechas) son las del contrato de compatibilidad y están fijadas por los
//! casos de paridad.

use std::str::FromStr;

use jiff::civil::{Date, DateTime};
use jiff::tz::{Offset, TimeZone};
use jiff::{Span, Timestamp, Zoned};
use rust_decimal::Decimal;

use crate::error::XmlParserError;

/// Espacios que se ignoran al inicio de un número o de una fecha.
fn is_numeric_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0B}' | '\u{0C}')
}

/// Prefijo numérico: signo, mantisa (dígitos de la parte entera y de la
/// fraccionaria) y exponente decimal. `None` si no hay dígitos.
struct Numeric<'a> {
    negative: bool,
    integer: &'a str,
    fraction: &'a str,
    /// Hay punto decimal o exponente (no es un entero `strtol`).
    real: bool,
    exponent: i64,
}

fn numeric_prefix(text: &str) -> Option<Numeric<'_>> {
    let s = text.trim_start_matches(is_numeric_space);
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let int_len = s.bytes().take_while(u8::is_ascii_digit).count();
    let integer = &s[..int_len];
    let mut rest = &s[int_len..];
    let mut fraction = "";
    let mut real = false;
    if let Some(after) = rest.strip_prefix('.') {
        let len = after.bytes().take_while(u8::is_ascii_digit).count();
        fraction = &after[..len];
        if integer.is_empty() && fraction.is_empty() {
            return None;
        }
        rest = &after[len..];
        real = true;
    }
    if integer.is_empty() && fraction.is_empty() {
        return None;
    }
    let mut exponent = 0_i64;
    if let Some(after) = rest.strip_prefix(['e', 'E']) {
        let (sign, digits) = match after.as_bytes().first() {
            Some(b'-') => (-1, &after[1..]),
            Some(b'+') => (1, &after[1..]),
            _ => (1, after),
        };
        let len = digits.bytes().take_while(u8::is_ascii_digit).count();
        if len > 0 {
            // Exponentes enormes saturan: el resultado queda fuera de rango.
            let value = digits[..len].parse::<i64>().unwrap_or(i64::MAX / 2);
            exponent = sign * value.min(i64::MAX / 2);
            real = true;
        }
    }
    Some(Numeric {
        negative,
        integer,
        fraction,
        real,
        exponent,
    })
}

/// Máximo de cifras significativas antes de redondear (`Decimal` guarda 28).
const MAX_DIGITS: usize = 28;

/// Importe como [`Decimal`] exacto a partir del prefijo numérico del texto.
///
/// Admite espacios iniciales, signo, mantisa con punto y exponente; el resto
/// se ignora y sin prefijo el valor es `0`. El cero negativo conserva el
/// signo. El valor es exacto hasta 28 cifras (sin redondeo a binario) y uno
/// que no cabe en `Decimal` es un error.
///
/// # Errors
///
/// [`XmlParserError::Number`] si el valor queda fuera del rango de `Decimal`.
pub(crate) fn to_float(text: &str) -> Result<Decimal, XmlParserError> {
    let Some(n) = numeric_prefix(text) else {
        return Ok(Decimal::ZERO);
    };
    let out_of_range = || XmlParserError::Number {
        text: text.to_owned(),
    };
    // Cifras de la mantisa sin los ceros iniciales: la parte entera seguida
    // de la fraccionaria, sin concatenarlas.
    let int_zeros = n.integer.bytes().take_while(|b| *b == b'0').count();
    let (significant_int, significant_fraction) = if int_zeros < n.integer.len() {
        (&n.integer[int_zeros..], n.fraction)
    } else {
        let zeros = n.fraction.bytes().take_while(|b| *b == b'0').count();
        ("", &n.fraction[zeros..])
    };
    let significant_len = significant_int.len() + significant_fraction.len();
    let mut value = if significant_len == 0 {
        Decimal::ZERO
    } else {
        // Posición del punto decimal respecto del inicio de `significant`.
        let digits_len = n.integer.len() + n.fraction.len();
        let leading_zeros =
            i64::try_from(digits_len - significant_len).map_err(|_| out_of_range())?;
        let int_len = i64::try_from(n.integer.len()).map_err(|_| out_of_range())?;
        let point = int_len - leading_zeros + n.exponent;
        let sig_len = i64::try_from(significant_len).map_err(|_| out_of_range())?;
        if point > 29 {
            return Err(out_of_range());
        }
        // Escala tras colocar el punto; por debajo de 1e-28 no hay
        // representación distinta de cero.
        if point < -i64::try_from(MAX_DIGITS).map_err(|_| out_of_range())? {
            return Err(out_of_range());
        }
        if let Some(mut value) =
            exact_decimal(significant_int, significant_fraction, point, sig_len)
        {
            value.set_sign_negative(n.negative);
            return Ok(value);
        }
        let significant = format!("{significant_int}{significant_fraction}");
        let significant = significant.as_str();
        let mut text_value = String::new();
        if point <= 0 {
            text_value.push_str("0.");
            for _ in 0..(-point) {
                text_value.push('0');
            }
            text_value.push_str(significant);
        } else if point >= sig_len {
            text_value.push_str(significant);
            for _ in 0..(point - sig_len) {
                text_value.push('0');
            }
        } else {
            let split = usize::try_from(point).map_err(|_| out_of_range())?;
            text_value.push_str(&significant[..split]);
            text_value.push('.');
            text_value.push_str(&significant[split..]);
        }
        let value = Decimal::from_str(&text_value).map_err(|_| out_of_range())?;
        if value.is_zero() {
            return Err(out_of_range());
        }
        value
    };
    value.set_sign_negative(n.negative);
    Ok(value)
}

/// El valor `0.significant × 10^point` sin pasar por texto, cuando cabe
/// exacto en `Decimal` (hasta [`MAX_DIGITS`] cifras y escala 28): es el que
/// produce `Decimal::from_str` con el mismo número escrito sin exponente,
/// que solo redondea por encima de esos límites. `None` en otro caso.
fn exact_decimal(int: &str, fraction: &str, point: i64, sig_len: i64) -> Option<Decimal> {
    if usize::try_from(sig_len).ok()? > MAX_DIGITS {
        return None;
    }
    let mut mantissa = int
        .bytes()
        .chain(fraction.bytes())
        .fold(0_i128, |acc, digit| acc * 10 + i128::from(digit - b'0'));
    let scale = if point >= sig_len {
        for _ in 0..point - sig_len {
            mantissa = mantissa.checked_mul(10)?;
        }
        0
    } else {
        u32::try_from(sig_len - point).ok()?
    };
    Decimal::try_from_i128_with_scale(mantissa, scale).ok()
}

/// Entero a partir del prefijo numérico: con saturación en los extremos si
/// el prefijo es entero; si tiene punto o exponente, el valor decimal se
/// trunca hacia cero.
///
/// # Errors
///
/// [`XmlParserError::Number`] si un valor con decimales o exponente no cabe
/// en `i64`.
pub(crate) fn to_int(text: &str) -> Result<i64, XmlParserError> {
    let Some(n) = numeric_prefix(text) else {
        return Ok(0);
    };
    if !n.real {
        let digits = n.integer.trim_start_matches('0');
        let limit = if n.negative {
            i64::MIN.unsigned_abs()
        } else {
            i64::MAX.unsigned_abs()
        };
        // `strtol` satura en los extremos.
        let magnitude = if digits.is_empty() {
            0
        } else {
            digits.parse::<u64>().map_or(limit, |m| m.min(limit))
        };
        return Ok(if n.negative {
            0_i64.checked_sub_unsigned(magnitude).unwrap_or(i64::MIN)
        } else {
            i64::try_from(magnitude).unwrap_or(i64::MAX)
        });
    }
    let value = to_float(text)?;
    i64::try_from(value.trunc()).map_err(|_| XmlParserError::Number {
        text: text.to_owned(),
    })
}

/// Texto que es un número completo (se admiten espacios a ambos lados).
fn numeric_string(text: &str) -> Option<&str> {
    let t = text.trim_matches(is_numeric_space);
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    let int_len = body.bytes().take_while(u8::is_ascii_digit).count();
    let mut rest = &body[int_len..];
    let mut frac_len = 0;
    if let Some(after) = rest.strip_prefix('.') {
        frac_len = after.bytes().take_while(u8::is_ascii_digit).count();
        rest = &after[frac_len..];
    }
    if int_len == 0 && frac_len == 0 {
        return None;
    }
    if let Some(after) = rest.strip_prefix(['e', 'E']) {
        let after = after.strip_prefix(['+', '-']).unwrap_or(after);
        let len = after.bytes().take_while(u8::is_ascii_digit).count();
        if len == 0 {
            return None;
        }
        rest = &after[len..];
    }
    rest.is_empty().then_some(t)
}

/// Igualdad tolerante entre códigos: si ambos textos son numéricos se
/// comparan como números (enteros si caben en `i64`, si no como `f64`); en
/// otro caso, byte a byte. Así `"1001.0"` equivale a `"1001"`.
pub(crate) fn loose_eq(a: &str, b: &str) -> bool {
    match (numeric_string(a), numeric_string(b)) {
        (Some(x), Some(y)) => match (x.parse::<i64>(), y.parse::<i64>()) {
            (Ok(x), Ok(y)) => x == y,
            _ => match (x.parse::<f64>(), y.parse::<f64>()) {
                (Ok(x), Ok(y)) => x == y,
                _ => false,
            },
        },
        _ => a == b,
    }
}

/// Valor ausente a efectos de la lectura: cadena vacía o `"0"`.
pub(crate) fn is_empty(text: &str) -> bool {
    text.is_empty() || text == "0"
}

/// Quita en los extremos espacios, tabuladores, saltos, `\0` y `\x0B`.
pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches([' ', '\t', '\n', '\r', '\0', '\u{0B}'])
}

/// Parte `index` de un texto separado por `-` (`F001-123` → `F001`, `123`);
/// `None` si no existe.
pub(crate) fn explode_part(text: &str, index: usize) -> Option<String> {
    text.split('-').nth(index).map(str::to_owned)
}

/// Zona horaria y reloj con que se interpretan las fechas.
#[derive(Debug, Clone)]
pub(crate) struct DateContext {
    pub(crate) time_zone: TimeZone,
    pub(crate) now: Timestamp,
}

impl DateContext {
    /// Instante actual con precisión de microsegundos, en la zona configurada.
    pub(crate) fn now(&self) -> Zoned {
        let micros = self.now.as_microsecond();
        let ts = Timestamp::from_microsecond(micros).unwrap_or(self.now);
        ts.to_zoned(self.time_zone.clone())
    }

    /// Fecha de un comprobante, en el subconjunto de formatos ISO admitido: `AAAA-MM-DD`, con hora opcional (`T`, `t` o
    /// espacio; `HH:MM`, `HH:MM:SS`, fracción) y zona opcional (`Z`, `±HH`,
    /// `±HHMM`, `±HH:MM`). Admite espacios alrededor, desborda días
    /// y meses (`2024-02-30` → 1 de marzo, día `00`, hora `24`) y trunca la
    /// fracción a microsegundos. Sin zona, usa la del proceso; con zona, un
    /// desplazamiento fijo. Texto vacío: la hora actual.
    ///
    /// # Errors
    ///
    /// [`XmlParserError::Date`]: con `unsupported = false` si el texto no es
    /// una fecha (clase de referencia `DateMalformedStringException`); con
    /// `true` si tiene otra forma que este crate no interpreta (formatos
    /// relativos, nombres de mes, años de dos cifras…).
    pub(crate) fn parse(&self, text: &str) -> Result<Zoned, XmlParserError> {
        let trimmed = text.trim_matches(is_numeric_space);
        if trimmed.is_empty() {
            return Ok(self.now());
        }
        parse_iso(trimmed, &self.time_zone).ok_or_else(|| XmlParserError::Date {
            text: text.to_owned(),
            unsupported: !looks_invalid(trimmed),
        })
    }
}

/// Textos que no son una fecha en ningún formato: empiezan por un patrón de
/// fecha imposible (mes > 12, día > 31) o por un carácter que ningún formato
/// admite.
fn looks_invalid(text: &str) -> bool {
    if let Some((y, m, d, _)) = scan_date(text) {
        let _ = y;
        return m > 12 || d > 31;
    }
    // `15/01/2024` se interpreta como `mm/dd/yyyy` y falla con mes 15; un
    // número inicial seguido de `/` con mes imposible es un error.
    let mut parts = text.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(m), Some(_), Some(_)) => m.parse::<u32>().is_ok_and(|m| m > 12),
        _ => false,
    }
}

/// `AAAA-M[M]-D[D]` al inicio: (año, mes, día, bytes consumidos).
fn scan_date(text: &str) -> Option<(i16, u32, u32, usize)> {
    let b = text.as_bytes();
    let digits = |from: usize, min: usize, max: usize| -> Option<(u32, usize)> {
        let len = b
            .get(from..)?
            .iter()
            .take(max)
            .take_while(|c| c.is_ascii_digit())
            .count();
        if len < min {
            return None;
        }
        let value = text.get(from..from + len)?.parse().ok()?;
        Some((value, from + len))
    };
    let (year, at) = digits(0, 4, 4)?;
    if b.get(at) != Some(&b'-') {
        return None;
    }
    let (month, at) = digits(at + 1, 1, 2)?;
    if b.get(at) != Some(&b'-') {
        return None;
    }
    let (day, at) = digits(at + 1, 1, 2)?;
    if b.get(at).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    Some((i16::try_from(year).ok()?, month, day, at))
}

fn parse_iso(text: &str, zone: &TimeZone) -> Option<Zoned> {
    let (year, month, day, mut at) = scan_date(text)?;
    if month > 12 || day > 31 {
        return None;
    }
    let b = text.as_bytes();
    let (mut hour, mut minute, mut second, mut micro) = (0_i64, 0_i64, 0_i64, 0_i64);
    // Hora opcional.
    if matches!(b.get(at), Some(b'T' | b't' | b' ')) {
        let mut start = at + 1;
        while b.get(start) == Some(&b' ') {
            start += 1;
        }
        if let Some((h, m, s, f, end)) = scan_time(text, start) {
            (hour, minute, second, micro, at) = (h, m, s, f, end);
        } else if b.get(at) != Some(&b' ') {
            return None;
        }
    }
    // Zona opcional.
    let mut rest = &text[at..];
    rest = rest.trim_start_matches(' ');
    let offset = if rest.is_empty() {
        None
    } else {
        Some(scan_offset(rest)?)
    };
    let base = Date::new(year, 1, 1).ok()?;
    let date = base
        .checked_add(Span::new().months(i64::from(month) - 1))
        .ok()?
        .checked_add(Span::new().days(i64::from(day) - 1))
        .ok()?;
    let datetime: DateTime = date
        .to_datetime(jiff::civil::Time::midnight())
        .checked_add(
            Span::new()
                .hours(hour)
                .minutes(minute)
                .seconds(second)
                .microseconds(micro),
        )
        .ok()?;
    let tz = match offset {
        Some(seconds) => TimeZone::fixed(Offset::from_seconds(seconds).ok()?),
        None => zone.clone(),
    };
    datetime.to_zoned(tz).ok()
}

/// `HH:MM[:SS[.fracción]]` con hora 0–24, minutos 0–59 y segundos 0–60.
fn scan_time(text: &str, from: usize) -> Option<(i64, i64, i64, i64, usize)> {
    let b = text.as_bytes();
    let two = |at: usize, min: usize| -> Option<(i64, usize)> {
        let len = b
            .get(at..)?
            .iter()
            .take(2)
            .take_while(|c| c.is_ascii_digit())
            .count();
        if len < min {
            return None;
        }
        Some((text.get(at..at + len)?.parse().ok()?, at + len))
    };
    let (hour, at) = two(from, 1)?;
    if b.get(at) != Some(&b':') {
        return None;
    }
    let (minute, mut at) = two(at + 1, 2)?;
    let mut second = 0;
    let mut micro = 0;
    if b.get(at) == Some(&b':') {
        let (s, end) = two(at + 1, 2)?;
        second = s;
        at = end;
        if matches!(b.get(at), Some(b'.' | b',')) {
            let len = b
                .get(at + 1..)?
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count();
            if len == 0 {
                return None;
            }
            let frac = text.get(at + 1..at + 1 + len.min(6))?;
            micro = format!("{frac:0<6}").parse().ok()?;
            at += 1 + len;
        }
    }
    if hour > 24 || minute > 59 || second > 60 || b.get(at).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    Some((hour, minute, second, micro, at))
}

/// `Z`, `±H[H]`, `±HHMM` o `±HH:MM` → segundos; el texto debe acabar ahí.
fn scan_offset(text: &str) -> Option<i32> {
    if text.eq_ignore_ascii_case("z") {
        return Some(0);
    }
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    let (hours, minutes) = if let Some((h, m)) = digits.split_once(':') {
        (h, m)
    } else if digits.len() > 2 {
        digits.split_at(digits.len() - 2)
    } else {
        (digits, "0")
    };
    let all_digits =
        |s: &str| !s.is_empty() && s.len() <= 2 && s.bytes().all(|c| c.is_ascii_digit());
    if !all_digits(hours) || !all_digits(minutes) {
        return None;
    }
    let hours: i32 = hours.parse().ok()?;
    let minutes: i32 = minutes.parse().ok()?;
    if hours > 99 || minutes > 59 {
        return None;
    }
    Some(sign * (hours * 3600 + minutes * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(text: &str) -> String {
        to_float(text).map_or_else(|_| "ERR".into(), |d| d.to_string())
    }

    #[test]
    fn float_casts() {
        assert_eq!(f("118.00"), "118.00");
        assert_eq!(f(" 12.5abc"), "12.5");
        assert_eq!(f("\n\t 5.5\n"), "5.5");
        assert_eq!(f("1,234.50"), "1");
        assert_eq!(f("1e3"), "1000");
        assert_eq!(f("2.5E+2x"), "250");
        assert_eq!(f(""), "0");
        assert_eq!(f("abc"), "0");
        assert_eq!(f("0x1A"), "0");
        assert_eq!(f("INF"), "0");
        assert_eq!(f("+7"), "7");
        assert_eq!(f(".5"), "0.5");
        assert_eq!(f("5."), "5");
        assert_eq!(f("1e"), "1");
        assert_eq!(f("-"), "0");
        assert_eq!(f("12345678901234567.89"), "12345678901234567.89");
        assert_eq!(f("1e400"), "ERR");
        assert_eq!(f("1e-40"), "ERR");
        assert_eq!(f("1e28"), "10000000000000000000000000000");
        assert_eq!(
            f("0.0000000000000000000000000001"),
            "0.0000000000000000000000000001"
        );
        let neg_zero = to_float("-0.00").unwrap_or_default();
        assert!(neg_zero.is_zero() && neg_zero.is_sign_negative());
    }

    #[test]
    fn int_casts() {
        let i = |t: &str| to_int(t).ok();
        assert_eq!(i("3"), Some(3));
        assert_eq!(i("3.9 bultos"), Some(3));
        assert_eq!(i(" 1e3"), Some(1000));
        assert_eq!(i("-4"), Some(-4));
        assert_eq!(i("abc"), Some(0));
        assert_eq!(i(""), Some(0));
        assert_eq!(i("99999999999999999999"), Some(i64::MAX));
        assert_eq!(i("-99999999999999999999"), Some(i64::MIN));
        assert_eq!(i("-9.9"), Some(-9));
        assert_eq!(i("1e30"), None);
    }

    #[test]
    fn loose_equality() {
        assert!(loose_eq("1001.0", "1001"));
        assert!(loose_eq("1", "01"));
        assert!(loose_eq(" 1 ", "01"));
        assert!(loose_eq("1e3", "1000"));
        assert!(!loose_eq("1001a", "1001"));
        assert!(!loose_eq("ra", "RA"));
        assert!(loose_eq("IGV", "IGV"));
        assert!(!loose_eq("", "0"));
        assert!(loose_eq("0x", "0x"));
        assert!(is_empty("0") && is_empty("") && !is_empty("0.0"));
        assert_eq!(trim("\t a \n\0"), "a");
    }

    #[test]
    fn explode() {
        assert_eq!(explode_part("F001-123", 1).as_deref(), Some("123"));
        assert_eq!(explode_part("F001", 1), None);
        assert_eq!(explode_part("", 0).as_deref(), Some(""));
    }

    #[test]
    fn dates() {
        let lima = TimeZone::get("America/Lima").unwrap_or(TimeZone::UTC);
        let ctx = DateContext {
            time_zone: lima,
            now: Timestamp::from_second(1_700_000_000).unwrap_or(Timestamp::UNIX_EPOCH),
        };
        let p = |t: &str| {
            ctx.parse(t)
                .map(|z| z.to_string())
                .map_err(|e| e.error_class().map(str::to_owned))
        };
        assert_eq!(
            p("2024-01-15").as_deref(),
            Ok("2024-01-15T00:00:00-05:00[America/Lima]")
        );
        assert_eq!(
            p(" 2024-02-30 ").as_deref(),
            Ok("2024-03-01T00:00:00-05:00[America/Lima]")
        );
        assert_eq!(
            p("2024-01-00").as_deref(),
            Ok("2023-12-31T00:00:00-05:00[America/Lima]")
        );
        assert_eq!(
            p("2024-01-15T24:00:00").as_deref(),
            Ok("2024-01-16T00:00:00-05:00[America/Lima]")
        );
        assert_eq!(
            p("2024-01-15T10:20:30-05:00").as_deref(),
            Ok("2024-01-15T10:20:30-05:00[-05:00]")
        );
        assert_eq!(
            p("2024-01-15T10:20:30+0530").as_deref(),
            Ok("2024-01-15T10:20:30+05:30[+05:30]")
        );
        assert_eq!(
            p("2024-01-15-05:00").as_deref(),
            Ok("2024-01-15T00:00:00-05:00[-05:00]")
        );
        assert_eq!(
            p("2024-01-15T10:20:30.123456789").as_deref(),
            Ok("2024-01-15T10:20:30.123456-05:00[America/Lima]")
        );
        assert_eq!(
            p("2024-01-15 10:20").as_deref(),
            Ok("2024-01-15T10:20:00-05:00[America/Lima]")
        );
        assert_eq!(
            p("").as_deref(),
            Ok("2023-11-14T17:13:20-05:00[America/Lima]")
        );
        assert_eq!(
            p("2024-13-01"),
            Err(Some("DateMalformedStringException".into()))
        );
        assert_eq!(
            p("2024-01-32"),
            Err(Some("DateMalformedStringException".into()))
        );
        assert_eq!(
            p("15/01/2024"),
            Err(Some("DateMalformedStringException".into()))
        );
        assert_eq!(p("tomorrow"), Err(None));
        assert_eq!(p("24-01-15"), Err(None));
    }
}
