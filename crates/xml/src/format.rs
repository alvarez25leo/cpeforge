//! Formatos de números y tributos de las plantillas de la referencia.
//!
//! Las plantillas reciben importes `float` y los imprimen con tres reglas:
//!
//! - `{{ x }}`: conversión de la referencia de `float` a texto con `precision=14`
//!   ([`float_string`]);
//! - `|n_format(n)`: `number_format($x, n, '.', '')` ([`n_format`]);
//! - `|n_format_limit(n)`: el texto de la referencia si tiene como máximo `n` decimales y
//!   no usa exponente; si no, `n_format` ([`n_format_limit`]).
//!
//! Aquí se aplican sobre el [`Decimal`] exacto. Coinciden con la referencia mientras el
//! importe tenga hasta 15 cifras significativas; con más cifras la referencia ya habría
//! perdido precisión al convertirlo en `float`.

use std::fmt::Write as _;

use cpeforge_core::Decimal;
use cpeforge_core::response_code::int_cast;
use rust_decimal::RoundingStrategy;

/// Cifras significativas de la conversión `float` → texto con `precision=14`.
const PRECISION: usize = 14;

/// Texto que la referencia produce al imprimir el `float` equivalente (`echo $x` con
/// `precision=14`), por ejemplo `2`, `0.1`, `1.0E+14`, `1.0E-5` o `-0`.
///
/// Redondea a 14 cifras significativas (al par en empates), quita ceros
/// finales y usa notación exponencial si el exponente decimal es menor que
/// -4 o mayor o igual que 14, igual que `zend_gcvt`.
#[must_use]
pub fn float_string(value: Decimal) -> String {
    let mut out = String::new();
    write_float(&mut out, value);
    out
}

/// Dígitos decimales de `value` en `buf`, sin ceros a la izquierda.
fn mantissa_digits(value: u128, buf: &mut [u8; 40]) -> &[u8] {
    let mut start = buf.len();
    let mut rest = value;
    loop {
        start -= 1;
        // `rest % 10` siempre cabe en un dígito.
        buf[start] = b'0' + u8::try_from(rest % 10).unwrap_or(0);
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    &buf[start..]
}

/// [`float_string`] escrito al final de `out`.
pub(crate) fn write_float(out: &mut String, value: Decimal) {
    write_float_with(out, value, true);
}

/// Como [`float_string`], pero siempre en notación posicional: mismas 14
/// cifras significativas y mismo redondeo, sin exponente (`1.0E-6` se escribe
/// `0.000001` y `1.0E+14`, `100000000000000`).
///
/// Es la conversión que usan las plantillas: `xsd:decimal` no admite
/// exponente, así que el texto de la referencia con exponente no valida
/// contra el esquema.
#[must_use]
pub fn xsd_float_string(value: Decimal) -> String {
    let mut out = String::new();
    write_xsd_float(&mut out, value);
    out
}

/// [`xsd_float_string`] escrito al final de `out`.
pub(crate) fn write_xsd_float(out: &mut String, value: Decimal) {
    write_float_with(out, value, false);
}

/// Conversión de `float` a texto; `exponent` decide si se usa la notación
/// exponencial de la referencia fuera del rango `[1e-4, 1e14)`.
fn write_float_with(out: &mut String, value: Decimal, exponent: bool) {
    if value.is_zero() {
        out.push_str(if value.is_sign_negative() { "-0" } else { "0" });
        return;
    }
    let mut buf = [0u8; 40];
    let all = mantissa_digits(value.mantissa().unsigned_abs(), &mut buf);
    let scale = i64::from(value.scale());
    let len = i64::try_from(all.len()).unwrap_or(i64::MAX);
    let mut decpt = len - scale;
    // Los dígitos redondeados a PRECISION cifras significativas.
    let mut rounded = [0u8; PRECISION];
    let mut count = all.len().min(PRECISION);
    rounded[..count].copy_from_slice(&all[..count]);
    if all.len() > PRECISION {
        let digits = all;
        let round_up = match digits[PRECISION].cmp(&b'5') {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => {
                digits[PRECISION + 1..].iter().any(|d| *d != b'0')
                    || (digits[PRECISION - 1] - b'0') % 2 == 1
            }
        };
        if round_up && increment(&mut rounded) {
            // 99…9 + 1 = 100…0: un dígito más, que se recorta a PRECISION.
            rounded[0] = b'1';
            rounded[1..].fill(b'0');
            decpt += 1;
        }
    }
    while count > 1 && rounded[count - 1] == b'0' {
        count -= 1;
    }
    let digits = &rounded[..count];

    if value.is_sign_negative() {
        out.push('-');
    }
    let ndigit = i64::try_from(PRECISION).unwrap_or(i64::MAX);
    // `zend_gcvt`: exponente si decpt < -3 (valores < 1e-4) o decpt > 14.
    if exponent && (decpt < -3 || decpt > ndigit) {
        let exponent = decpt - 1;
        out.push(char::from(digits[0]));
        out.push('.');
        if digits.len() == 1 {
            out.push('0');
        } else {
            out.extend(digits[1..].iter().map(|d| char::from(*d)));
        }
        out.push('E');
        out.push(if exponent < 0 { '-' } else { '+' });
        let _ = write!(out, "{}", exponent.unsigned_abs());
    } else if decpt < 0 {
        out.push_str("0.");
        for _ in 0..decpt.unsigned_abs() {
            out.push('0');
        }
        out.extend(digits.iter().map(|d| char::from(*d)));
    } else {
        let int_len = usize::try_from(decpt).unwrap_or(usize::MAX);
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

/// Suma uno a una secuencia de dígitos ASCII; devuelve `true` si desborda.
fn increment(digits: &mut [u8]) -> bool {
    for digit in digits.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return false;
        }
    }
    true
}

/// `FormatFilter::number` (`|n_format`): `number_format($x, $decimals, '.', '')`.
///
/// - `None` se formatea como `0`, igual que la referencia al recibir `null` (con un
///   aviso de deprecación que aquí no existe);
/// - redondeo «half up» (alejándose de cero) a `decimals` cifras, salvo para
///   valores cuyo producto por `10^decimals` alcanza `1e15`, que la referencia no redondea;
/// - nunca imprime `-0`: un resultado cero no lleva signo.
#[must_use]
pub fn n_format(value: Option<Decimal>, decimals: u32) -> String {
    let mut out = String::new();
    write_n_format(&mut out, value, decimals);
    out
}

/// [`n_format`] escrito al final de `out`.
pub(crate) fn write_n_format(out: &mut String, value: Option<Decimal>, decimals: u32) {
    let value = value.unwrap_or(Decimal::ZERO);
    let threshold = Decimal::from_i128_with_scale(1_000_000_000_000_000, decimals.min(28));
    let strategy = if value.abs() >= threshold {
        RoundingStrategy::MidpointNearestEven
    } else {
        RoundingStrategy::MidpointAwayFromZero
    };
    let mut rounded = value.round_dp_with_strategy(decimals, strategy);
    if rounded.is_zero() {
        rounded.set_sign_positive(true);
    }
    // Sin `{:.width$}`: `rust_decimal` entra en pánico si el texto con
    // relleno supera su búfer interno (p. ej. 1e23 con 10 decimales). Se
    // escribe el valor redondeado y se completan los ceros a mano.
    let _ = write!(out, "{rounded}");
    let scale = rounded.scale();
    if decimals > 0 {
        if scale == 0 {
            out.push('.');
        }
        for _ in scale..decimals {
            out.push('0');
        }
    }
}

/// `FormatFilter::numberLimit` (`|n_format_limit`).
///
/// Devuelve el texto de la referencia del número ([`float_string`]) si tiene como
/// máximo `decimals` decimales y no está en notación exponencial; en otro caso
/// aplica [`n_format`]. Con `None` devuelve una cadena vacía (la referencia devuelve
/// `null`).
#[must_use]
pub fn n_format_limit(value: Option<Decimal>, decimals: u32) -> String {
    let mut out = String::new();
    write_n_format_limit(&mut out, value, decimals);
    out
}

/// [`n_format_limit`] escrito al final de `out`.
pub(crate) fn write_n_format_limit(out: &mut String, value: Option<Decimal>, decimals: u32) {
    let Some(value) = value else {
        return;
    };
    let start = out.len();
    write_float(out, value);
    let text = &out[start..];
    let decimals_len = text.rfind('.').map_or(0, |pos| text.len() - pos - 1);
    let limit = usize::try_from(decimals).unwrap_or(usize::MAX);
    if decimals_len > limit || text.contains('E') {
        out.truncate(start);
        write_n_format(out, Some(value), decimals);
    }
}

/// Como [`n_format_limit`], pero sin ceros de relleno en valores desde
/// `1e14`: ahí la referencia pasa por notación exponencial y rellena con
/// `n_format`, por ejemplo `1000000000000000.0000000000` (26 dígitos), que
/// supera la precisión de `xsd:decimal` en libxml2 (24 dígitos) y no valida.
/// Aquí sale `1000000000000000`, el mismo valor redondeado a `decimals`
/// cifras. Con valores menores el texto es idéntico a [`n_format_limit`].
#[must_use]
pub fn xsd_n_format_limit(value: Option<Decimal>, decimals: u32) -> String {
    let mut out = String::new();
    write_xsd_n_format_limit(&mut out, value, decimals);
    out
}

/// [`xsd_n_format_limit`] escrito al final de `out`.
pub(crate) fn write_xsd_n_format_limit(out: &mut String, value: Option<Decimal>, decimals: u32) {
    let Some(value) = value else {
        return;
    };
    let start = out.len();
    write_float(out, value);
    let text = &out[start..];
    let decimals_len = text.rfind('.').map_or(0, |pos| text.len() - pos - 1);
    let limit = usize::try_from(decimals).unwrap_or(usize::MAX);
    let exponent = text.contains('E');
    if decimals_len > limit || exponent {
        out.truncate(start);
        write_n_format(out, Some(value), decimals);
        let large = value.abs() >= Decimal::from(100_000_000_000_000_i64);
        if exponent && large && out[start..].contains('.') {
            let trimmed = out.trim_end_matches('0').trim_end_matches('.').len();
            out.truncate(trimmed);
        }
    }
}

/// Tributo SUNAT (catálogo 05): código, código internacional y nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tributo {
    /// Código de tributo (`1000`, `9997`, …).
    pub id: &'static str,
    /// Código internacional (`VAT`, `EXC`, `FRE`, `OTH`).
    pub code: &'static str,
    /// Nombre (`IGV`, `ISC`, `EXO`, …).
    pub name: &'static str,
}

const TRIBUTOS: [Tributo; 8] = [
    Tributo {
        id: "1000",
        code: "VAT",
        name: "IGV",
    },
    Tributo {
        id: "1016",
        code: "VAT",
        name: "IVAP",
    },
    Tributo {
        id: "2000",
        code: "EXC",
        name: "ISC",
    },
    Tributo {
        id: "9995",
        code: "FRE",
        name: "EXP",
    },
    Tributo {
        id: "9996",
        code: "FRE",
        name: "GRA",
    },
    Tributo {
        id: "9997",
        code: "VAT",
        name: "EXO",
    },
    Tributo {
        id: "9998",
        code: "FRE",
        name: "INA",
    },
    Tributo {
        id: "9999",
        code: "OTH",
        name: "OTROS",
    },
];

/// `TributoFunction::getByTributo`: el tributo de un código exacto.
#[must_use]
pub fn tributo(code: Option<&str>) -> Option<Tributo> {
    let code = code?;
    TRIBUTOS.iter().copied().find(|tributo| tributo.id == code)
}

/// `TributoFunction::getByAfectacion`: tributo según el tipo de afectación
/// del IGV (catálogo 07).
///
/// El código se convierte con `(int)` de la referencia; `10` es IGV, `17` IVAP, `20`
/// exonerado, `30` inafecto, `40` exportación y cualquier otro valor
/// (incluidos `null` y texto) gratuito `9996`.
#[must_use]
pub fn tributo_by_afectacion(afectacion: Option<&str>) -> Tributo {
    let id = match afectacion.map_or(0, int_cast) {
        10 => "1000",
        17 => "1016",
        20 => "9997",
        30 => "9998",
        40 => "9995",
        _ => "9996",
    };
    tributo(Some(id)).unwrap_or(TRIBUTOS[4])
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn d(text: &str) -> Decimal {
        Decimal::from_str(text).unwrap()
    }

    #[test]
    fn float_strings() {
        for (input, expected) in [
            ("2.000", "2"),
            ("0.1", "0.1"),
            ("100000000000000", "1.0E+14"),
            ("99999999999999", "99999999999999"),
            ("0.0001", "0.0001"),
            ("0.00001", "1.0E-5"),
            ("123456789012345", "1.2345678901234E+14"),
            ("-1.5", "-1.5"),
            ("1000000000000000000000", "1.0E+21"),
            ("99999999999999.5", "1.0E+14"),
        ] {
            assert_eq!(float_string(d(input)), expected, "{input}");
        }
        let mut zero = d("0.00");
        zero.set_sign_negative(true);
        assert_eq!(float_string(zero), "-0");
    }

    #[test]
    fn xsd_numbers_never_use_exponent() {
        for (input, expected) in [
            ("0.000001", "0.000001"),
            ("0.00001", "0.00001"),
            ("-0.00001", "-0.00001"),
            ("100000000000000", "100000000000000"),
            ("123456789012345", "123456789012340"),
            ("1000000000000000000000", "1000000000000000000000"),
            ("99999999999999.5", "100000000000000"),
            ("0.1", "0.1"),
            ("2.000", "2"),
        ] {
            assert_eq!(xsd_float_string(d(input)), expected, "{input}");
        }
        // Mismo valor que la referencia: solo cambia la notación.
        for input in ["0.00001", "123456789012345", "0.0000012345678901234567"] {
            let reference = float_string(d(input)).replace("E+", "e").replace('E', "e");
            assert_eq!(
                Decimal::from_scientific(&reference).unwrap(),
                d(&xsd_float_string(d(input))),
                "{input}"
            );
        }
    }

    #[test]
    fn xsd_number_limit_has_no_padding_from_1e14() {
        assert_eq!(
            xsd_n_format_limit(Some(d("1000000000000000")), 10),
            "1000000000000000"
        );
        assert_eq!(
            xsd_n_format_limit(Some(d("123456789012345.671875")), 10),
            "123456789012345.671875"
        );
        assert_eq!(
            n_format_limit(Some(d("1000000000000000")), 10),
            "1000000000000000.0000000000"
        );
        // Por debajo de 1e14, idéntico a la referencia (incluido el relleno).
        for (input, decimals) in [("0.000001", 10), ("0.123456", 5), ("59.0", 10), ("1.5", 2)] {
            assert_eq!(
                xsd_n_format_limit(Some(d(input)), decimals),
                n_format_limit(Some(d(input)), decimals),
                "{input}"
            );
        }
        assert_eq!(xsd_n_format_limit(None, 10), "");
    }

    #[test]
    fn number_format() {
        // Importes enormes: sin pánico y con el relleno completo.
        assert_eq!(
            n_format(Some(d("100000000000000000000000")), 10),
            "100000000000000000000000.0000000000"
        );
        assert_eq!(n_format(Some(d("0.5")), 0), "1");
        assert_eq!(n_format(Some(d("12")), 3), "12.000");
        assert_eq!(n_format(Some(d("1.10")), 2), "1.10");
        assert_eq!(n_format(Some(d("1.005")), 2), "1.01");
        assert_eq!(n_format(Some(d("-0.004")), 2), "0.00");
        assert_eq!(n_format(Some(d("-0.005")), 2), "-0.01");
        assert_eq!(n_format(None, 2), "0.00");
        assert_eq!(n_format(Some(d("2.5")), 0), "3");
        assert_eq!(n_format_limit(Some(d("0.123456")), 5), "0.12346");
        assert_eq!(n_format_limit(Some(d("59.0")), 10), "59");
        assert_eq!(n_format_limit(None, 10), "");
    }

    #[test]
    fn tributos() {
        assert_eq!(tributo_by_afectacion(Some("10")).id, "1000");
        assert_eq!(tributo_by_afectacion(Some("10abc")).id, "1000");
        assert_eq!(tributo_by_afectacion(None).id, "9996");
        assert_eq!(tributo(Some("2")), None);
    }
}
