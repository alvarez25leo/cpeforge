//! Texto de importes y fechas tal como lo muestran las plantillas.
//!
//! # Por qué `f64`
//!
//! Las plantillas comparan importes (`doc.totalAnticipos > 0`), evalúan su
//! verdad y los formatean. Para eso cada [`Decimal`] entra en la plantilla
//! como el `f64` más cercano a su valor exacto ([`decimal_to_f64`]) y se
//! formatea con [`number_format`] y [`float_to_string`], cuyo texto está fijado
//! por el corpus de `tests/reference/report` en todo el rango, también con más
//! de 15 cifras significativas. `tests/amounts.rs` comprueba que, mientras
//! `|importe| · 10^decimales < 2^50` y el importe tenga como mucho 15 cifras,
//! el resultado es el del redondeo decimal exacto.
//!
//! No se hace aritmética de negocio con estos valores: solo se presentan.

#![allow(
    clippy::float_arithmetic,
    reason = "redondeo y presentación sobre f64; no hay aritmética de importes"
)]

use std::fmt::Write as _;

use jiff::Zoned;
use jiff::civil::Weekday;
use rust_decimal::Decimal;

/// Cifras significativas con las que se imprime un `f64`.
const PRECISION: usize = 14;

/// Potencias de diez exactas en `f64` (`10^0` … `10^22`).
const EXACT_POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// Mayor entero representable sin pérdida en `f64` (`2^53`).
const EXACT_INT: u128 = 1 << 53;

/// El `f64` más cercano al valor decimal exacto de `value` (redondeo al par
/// más cercano, como `strtod`).
///
/// Si la mantisa cabe en 53 bits y la escala no pasa de 22, mantisa y
/// `10^escala` son exactas en `f64` y una sola división IEEE da ese
/// redondeo correcto (vía rápida de Clinger, la misma que usa `strtod`).
/// El cero conserva el signo con el que se imprime (`-0.00` → `-0.0`). En
/// otro caso se convierte el texto decimal con `str::parse`, también
/// correctamente redondeado.
#[must_use]
pub fn decimal_to_f64(value: Decimal) -> f64 {
    let mantissa = value.mantissa();
    if mantissa == 0 {
        return if value.is_sign_negative() { -0.0 } else { 0.0 };
    }
    let scale = usize::try_from(value.scale()).unwrap_or(usize::MAX);
    if mantissa.unsigned_abs() <= EXACT_INT {
        if let Some(divisor) = EXACT_POW10.get(scale) {
            #[allow(
                clippy::cast_precision_loss,
                reason = "|mantisa| <= 2^53: conversión exacta"
            )]
            let numerator = mantissa as f64;
            return numerator / divisor;
        }
    }
    decimal_to_f64_text(value)
}

/// Conversión por el texto decimal y `str::parse`.
fn decimal_to_f64_text(value: Decimal) -> f64 {
    let mut buf = StackText::new();
    if write!(buf, "{value}").is_err() {
        return value.to_string().parse::<f64>().unwrap_or(0.0);
    }
    buf.as_str().parse::<f64>().unwrap_or(0.0)
}

/// Búfer de texto en pila para números cortos (sin reservar memoria).
struct StackText {
    bytes: [u8; 64],
    len: usize,
}

impl StackText {
    const fn new() -> Self {
        Self {
            bytes: [0; 64],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(self.bytes.get(..self.len).unwrap_or_default()).unwrap_or_default()
    }
}

impl std::fmt::Write for StackText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(std::fmt::Error)?;
        let slot = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        slot.copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// Texto de un `f64` con 14 cifras significativas y sin ceros finales:
/// `18`, `0.1`, `-0`, `INF`, `NAN`. Usa notación `1.0E+20` si el exponente
/// decimal es mayor que 14 o menor que −4.
#[must_use]
pub fn float_to_string(value: f64) -> String {
    let mut out = String::new();
    write_float(&mut out, value);
    out
}

/// Límite de la vía rápida de [`write_float`]: con 14 cifras significativas,
/// un entero de hasta 14 cifras se imprime con todas ellas y sin exponente.
const PLAIN_INT_LIMIT: f64 = 1e14;

/// [`float_to_string`] escrito al final de `out`.
pub(crate) fn write_float(out: &mut String, value: f64) {
    if value.is_nan() {
        out.push_str("NAN");
        return;
    }
    if value.is_infinite() {
        out.push_str(if value < 0.0 { "-INF" } else { "INF" });
        return;
    }
    if value != 0.0 && value.fract() == 0.0 && value.abs() < PLAIN_INT_LIMIT {
        // Entero exacto de 1 a 14 cifras: se imprimen sus dígitos.
        #[allow(clippy::cast_possible_truncation, reason = "|v| < 1e14, entero")]
        let int = value as i64;
        let _ = write!(out, "{int}");
        return;
    }
    write_float_digits(out, value);
}

/// Algoritmo general de [`write_float`] para valores finitos.
fn write_float_digits(out: &mut String, value: f64) {
    if value.is_sign_negative() {
        out.push('-');
    }
    if value == 0.0 {
        out.push('0');
        return;
    }
    // 14 cifras significativas correctamente redondeadas (`{:.13e}`); después
    // se quitan los ceros finales.
    let mut sci = StackText::new();
    let _ = write!(sci, "{:.*e}", PRECISION - 1, value.abs());
    let sci = sci.as_str();
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((sci, "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let mut buf = [0u8; PRECISION];
    let mut len = 0;
    for (slot, digit) in buf
        .iter_mut()
        .zip(mantissa.bytes().filter(u8::is_ascii_digit))
    {
        *slot = digit;
        len += 1;
    }
    while len > 1 && buf.get(len - 1) == Some(&b'0') {
        len -= 1;
    }
    let digits = buf.get(..len).unwrap_or_default();
    let decpt = exponent + 1;
    let ndigit = i64::try_from(PRECISION).unwrap_or(14);
    if (decpt >= 0 && decpt > ndigit) || decpt < -3 {
        let shown = decpt - 1;
        out.push(char::from(digits[0]));
        out.push('.');
        if digits.len() == 1 {
            out.push('0');
        } else {
            out.extend(digits[1..].iter().map(|d| char::from(*d)));
        }
        out.push('E');
        out.push(if shown < 0 { '-' } else { '+' });
        let _ = write!(out, "{}", shown.unsigned_abs());
    } else if decpt < 0 {
        out.push_str("0.");
        for _ in 0..decpt.unsigned_abs() {
            out.push('0');
        }
        out.extend(digits.iter().map(|d| char::from(*d)));
    } else {
        let int_len = usize::try_from(decpt).unwrap_or(0);
        for index in 0..int_len {
            out.push(digits.get(index).map_or('0', |d| char::from(*d)));
        }
        if digits.len() > int_len {
            if int_len == 0 {
                out.push('0');
            }
            out.push('.');
            out.extend(digits[int_len..].iter().map(|d| char::from(*d)));
        }
    }
}

/// `10^power`: multiplicación exacta hasta `10^22` y `powi` por encima.
fn intpow10(power: u32) -> f64 {
    if power > 22 {
        return 10f64.powi(i32::try_from(power).unwrap_or(i32::MAX));
    }
    let mut out = 1.0;
    for _ in 0..power {
        out *= 10.0;
    }
    out
}

/// Redondea `value` a `places` decimales (o a decenas, centenas… si es
/// negativo) con las mitades hacia fuera del cero.
///
/// Toma la parte entera de `value · 10^places`, la corrige si el siguiente
/// entero representa exactamente `value`, y sube una unidad si `|value|`
/// alcanza el punto medio `(k ± 0.5) / 10^places` calculado en `f64`. Por eso
/// una mitad decimal como `1.005` sube aunque su `f64` quede por debajo.
/// Con `|k| ≥ 10^16` devuelve `value` sin cambios.
#[must_use]
pub fn round_half_up(value: f64, places: i32) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let places = places.max(i32::MIN + 1);
    let exponent = intpow10(places.unsigned_abs());
    let scaled = if places > 0 {
        value * exponent
    } else {
        value / exponent
    };
    let (mut tmp, tmp2) = if value >= 0.0 {
        let t = scaled.floor();
        (t, t + 1.0)
    } else {
        let t = scaled.ceil();
        (t, t - 1.0)
    };
    let back = if places > 0 {
        tmp2 / exponent
    } else {
        tmp2 * exponent
    };
    if back == value {
        tmp = tmp2;
    }
    if tmp.abs() >= 1e16 {
        return value;
    }
    // Mitad hacia fuera del cero, comparando con el punto medio en `f64`.
    let half = 0.5f64.copysign(tmp);
    let edge = if places > 0 {
        ((tmp + half) / exponent).abs()
    } else {
        ((tmp + half) * exponent).abs()
    };
    if value.abs() >= edge {
        tmp += 1.0f64.copysign(tmp);
    }
    if places.unsigned_abs() < 23 {
        if places > 0 {
            tmp / exponent
        } else {
            tmp * exponent
        }
    } else {
        // Fuera de las potencias exactas: se recompone por texto.
        format!("{tmp:15.6}e{}", -places)
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .unwrap_or(value)
    }
}

/// Número de una plantilla: entero o `f64`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    /// Entero de 64 bits.
    Int(i64),
    /// Número en coma flotante.
    Float(f64),
}

/// `value` con `decimals` decimales, punto decimal y sin separador de miles,
/// redondeado con [`round_half_up`]; nunca imprime `-0`.
///
/// Los enteros, y los `f64` de magnitud ≥ 2^52 que caben en un `i64`, se
/// formatean con aritmética entera exacta. Con `decimals` negativo redondea a
/// decenas, centenas… y no imprime decimales.
#[must_use]
pub fn number_format(value: Number, decimals: i64) -> String {
    match value {
        Number::Int(n) => number_format_long(n, decimals),
        Number::Float(f) => {
            // A partir de 2^52 todo `f64` es entero; debe caber en `i64`.
            let fits = (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&f);
            if f.abs() >= 4_503_599_627_370_496.0 && fits {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "comprobado que cabe en i64"
                )]
                let n = f as i64;
                return number_format_long(n, decimals);
            }
            number_format_double(f, decimals)
        }
    }
}

fn number_format_long(num: i64, dec: i64) -> String {
    let negative = num < 0;
    let mut abs = num.unsigned_abs();
    let mut is_negative = negative;
    if dec < 0 {
        if dec < -19 {
            abs = 0;
        } else {
            let power = 10u64.pow(u32::try_from(-dec).unwrap_or(0));
            let rest = abs % power;
            abs /= power;
            abs = if rest >= power / 2 {
                abs.saturating_mul(power).saturating_add(power)
            } else {
                abs.saturating_mul(power)
            };
        }
        if abs == 0 {
            is_negative = false;
        }
    }
    let mut out = String::new();
    if is_negative {
        out.push('-');
    }
    let _ = write!(out, "{abs}");
    if dec > 0 {
        out.push('.');
        for _ in 0..dec {
            out.push('0');
        }
    }
    out
}

fn number_format_double(value: f64, decimals: i64) -> String {
    let places =
        i32::try_from(decimals.clamp(i64::from(i32::MIN), i64::from(i32::MAX))).unwrap_or(0);
    let mut negative = value < 0.0;
    let rounded = round_half_up(value.abs(), places);
    let dec = usize::try_from(decimals.max(0)).unwrap_or(0);
    if rounded.is_nan() {
        return "nan".to_owned();
    }
    if rounded.is_infinite() {
        return "inf".to_owned();
    }
    if negative && rounded == 0.0 {
        negative = false;
    }
    let mut out = String::with_capacity(dec + 24);
    if negative {
        out.push('-');
    }
    let _ = write!(out, "{rounded:.dec$}");
    out
}

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn weekday_index(day: Weekday) -> usize {
    usize::try_from(day.to_monday_zero_offset()).unwrap_or(0)
}

/// Texto de `date` en su propia zona horaria según `format`.
///
/// `format` usa los códigos de una letra de `date()` de la referencia, el formato que
/// conocen las plantillas (`d D j l N S w z W F m M n t L o X x Y y a A B g G
/// h H i s u v e I O P p T Z c r U`); `\` copia el carácter siguiente y
/// cualquier otro carácter se copia tal cual. Nombres de días y meses en
/// inglés.
#[must_use]
pub fn format_date(date: &Zoned, format: &str) -> String {
    let mut out = String::new();
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }
        write_date_char(&mut out, date, c);
    }
    out
}

fn offset_seconds(date: &Zoned) -> i32 {
    date.offset().seconds()
}

fn write_offset(out: &mut String, seconds: i32, colon: bool) {
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.unsigned_abs();
    let (hours, minutes) = (abs / 3600, (abs % 3600) / 60);
    let _ = if colon {
        write!(out, "{sign}{hours:02}:{minutes:02}")
    } else {
        write!(out, "{sign}{hours:02}{minutes:02}")
    };
}

fn write_year(out: &mut String, year: i64, force_sign: bool) {
    let sign = if year < 0 {
        "-"
    } else if force_sign {
        "+"
    } else {
        ""
    };
    let _ = write!(out, "{sign}{:04}", year.unsigned_abs());
}

fn abbreviation(date: &Zoned) -> String {
    let info = date.time_zone().to_offset_info(date.timestamp());
    let abbr = info.abbreviation();
    if date.time_zone().iana_name().is_none() || abbr.is_empty() {
        let mut out = String::new();
        write_offset(&mut out, offset_seconds(date), true);
        return out;
    }
    abbr.to_owned()
}

fn write_date_char(out: &mut String, date: &Zoned, c: char) {
    let civil = date.date();
    let year = i64::from(civil.year());
    let month = usize::try_from(civil.month()).unwrap_or(1);
    let day = civil.day();
    let hour = date.hour();
    let weekday = weekday_index(civil.weekday());
    let hour12 = match hour % 12 {
        0 => 12,
        h => h,
    };
    let _ = match c {
        'd' => write!(out, "{day:02}"),
        'D' => write!(out, "{}", &DAYS[weekday][..3]),
        'j' => write!(out, "{day}"),
        'l' => write!(out, "{}", DAYS[weekday]),
        'N' => write!(out, "{}", weekday + 1),
        'S' => write!(
            out,
            "{}",
            match day {
                1 | 21 | 31 => "st",
                2 | 22 => "nd",
                3 | 23 => "rd",
                _ => "th",
            }
        ),
        'w' => write!(out, "{}", (weekday + 1) % 7),
        'z' => write!(out, "{}", civil.day_of_year() - 1),
        'W' => write!(out, "{:02}", civil.iso_week_date().week()),
        'F' => write!(out, "{}", MONTHS[month - 1]),
        'm' => write!(out, "{month:02}"),
        'M' => write!(out, "{}", &MONTHS[month - 1][..3]),
        'n' => write!(out, "{month}"),
        't' => write!(out, "{}", civil.days_in_month()),
        'L' => write!(out, "{}", u8::from(civil.in_leap_year())),
        // `o` no rellena con ceros, a diferencia de `Y`.
        'o' => write!(out, "{}", civil.iso_week_date().year()),
        'X' => {
            write_year(out, year, true);
            Ok(())
        }
        'x' => {
            write_year(out, year, !(0..10_000).contains(&year));
            Ok(())
        }
        'Y' => {
            write_year(out, year, false);
            Ok(())
        }
        'y' => write!(out, "{:02}", year.rem_euclid(100)),
        'a' => write!(out, "{}", if hour < 12 { "am" } else { "pm" }),
        'A' => write!(out, "{}", if hour < 12 { "AM" } else { "PM" }),
        'B' => {
            let sse = date.timestamp().as_second();
            let mut beat = ((sse.rem_euclid(86_400)) + 3600) * 10;
            if beat < 0 {
                beat += 864_000;
            }
            write!(out, "{:03}", (beat / 864) % 1000)
        }
        'g' => write!(out, "{hour12}"),
        'G' => write!(out, "{hour}"),
        'h' => write!(out, "{hour12:02}"),
        'H' => write!(out, "{hour:02}"),
        'i' => write!(out, "{:02}", date.minute()),
        's' => write!(out, "{:02}", date.second()),
        'u' => write!(out, "{:06}", date.subsec_nanosecond() / 1000),
        'v' => write!(out, "{:03}", date.subsec_nanosecond() / 1_000_000),
        'e' => {
            match date.time_zone().iana_name() {
                Some(name) => out.push_str(name),
                None => write_offset(out, offset_seconds(date), true),
            }
            Ok(())
        }
        'I' => write!(
            out,
            "{}",
            u8::from(
                date.time_zone()
                    .to_offset_info(date.timestamp())
                    .dst()
                    .is_dst()
            )
        ),
        'O' => {
            write_offset(out, offset_seconds(date), false);
            Ok(())
        }
        'P' => {
            write_offset(out, offset_seconds(date), true);
            Ok(())
        }
        'p' => {
            if offset_seconds(date) == 0 {
                out.push('Z');
            } else {
                write_offset(out, offset_seconds(date), true);
            }
            Ok(())
        }
        'T' => write!(out, "{}", abbreviation(date)),
        'Z' => write!(out, "{}", offset_seconds(date)),
        'c' => {
            out.push_str(&format_date(date, "Y-m-d\\TH:i:sP"));
            Ok(())
        }
        'r' => {
            out.push_str(&format_date(date, "D, d M Y H:i:s O"));
            Ok(())
        }
        'U' => write!(out, "{}", date.timestamp().as_second()),
        other => {
            out.push(other);
            Ok(())
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_text_examples() {
        assert_eq!(float_to_string(18.0), "18");
        assert_eq!(float_to_string(0.1), "0.1");
        assert_eq!(float_to_string(0.1 + 0.2), "0.3");
        assert_eq!(float_to_string(1e20), "1.0E+20");
        assert_eq!(float_to_string(1e14), "1.0E+14");
        assert_eq!(
            float_to_string(123_456_789_012_345.0),
            "1.2345678901234E+14"
        );
        assert_eq!(float_to_string(0.0001), "0.0001");
        assert_eq!(float_to_string(0.00001), "1.0E-5");
        assert_eq!(float_to_string(-0.0), "-0");
        assert_eq!(float_to_string(-2.5), "-2.5");
    }

    /// Generador determinista (xorshift64) para las pruebas de equivalencia.
    fn rng(seed: u64) -> impl FnMut() -> u64 {
        let mut state = seed;
        move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        }
    }

    #[test]
    fn fast_decimal_conversion_equals_strtod() {
        let mut next = rng(0x9E37_79B9_7F4A_7C15);
        let check = |d: Decimal| {
            let fast = decimal_to_f64(d);
            let text: f64 = d.to_string().parse().unwrap();
            assert_eq!(fast.to_bits(), text.to_bits(), "{d}");
        };
        for scale in 0..=28u32 {
            for _ in 0..4000 {
                // Mantisas de 1 a 96 bits: cubren la vía rápida y la de texto.
                let bits = next() % 96 + 1;
                let raw = ((u128::from(next()) << 64) | u128::from(next())) >> (128 - bits);
                let negative = next() % 2 == 0;
                let lo = u32::try_from(raw & 0xFFFF_FFFF).unwrap();
                let mid = u32::try_from((raw >> 32) & 0xFFFF_FFFF).unwrap();
                let hi = u32::try_from((raw >> 64) & 0xFFFF_FFFF).unwrap();
                check(Decimal::from_parts(lo, mid, hi, negative, scale));
            }
        }
        for text in [
            "0",
            "-0",
            "0.00",
            "-0.00",
            "0.1",
            "0.285",
            "1.005",
            "2.675",
            "118.00",
            "9007199254740992",
            "9007199254740993",
            "-9007199254740993.5",
            "12345678901234567.89",
            "0.0000000000000000000000000001",
            "79228162514264337593543950335",
            "-79228162514264337593543950335",
        ] {
            check(text.parse().unwrap());
        }
        for scale in [0, 2, 10, 28] {
            for negative in [false, true] {
                check(Decimal::from_parts(0, 0, 0, negative, scale));
            }
        }
    }

    #[test]
    fn fast_float_text_equals_general_algorithm() {
        let general = |v: f64| {
            let mut out = String::new();
            write_float_digits(&mut out, v);
            out
        };
        let mut next = rng(0xD1B5_4A32_D192_ED03);
        let mut values: Vec<f64> = (1..=200).map(f64::from).collect();
        values.extend([
            99_999_999_999_999.0,
            10_000_000_000_000.0,
            4_503_599_627_370_496.0,
        ]);
        for _ in 0..20_000 {
            #[allow(clippy::cast_precision_loss, reason = "valores de prueba")]
            let v = (next() % 100_000_000_000_000) as f64;
            values.push(v);
        }
        for v in values {
            for v in [v, -v] {
                if v != 0.0 {
                    assert_eq!(float_to_string(v), general(v), "{v}");
                }
            }
        }
        // Fuera de la vía rápida.
        assert_eq!(float_to_string(1e14), "1.0E+14");
        assert_eq!(float_to_string(2.927), "2.927");
    }

    #[test]
    fn number_format_examples() {
        let f = |v: f64, d: i64| number_format(Number::Float(v), d);
        assert_eq!(f(1.005, 2), "1.01");
        assert_eq!(f(2.675, 2), "2.68");
        assert_eq!(f(0.285, 2), "0.29");
        assert_eq!(f(-0.004, 2), "0.00");
        assert_eq!(f(1.0, 2), "1.00");
        assert_eq!(f(1234.5, 0), "1235");
        assert_eq!(f(1250.0, -2), "1300");
        assert_eq!(number_format(Number::Int(-1250), -2), "-1300");
        assert_eq!(number_format(Number::Int(7), 2), "7.00");
    }
}
