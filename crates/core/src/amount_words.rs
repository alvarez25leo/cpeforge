//! Importe en letras para la leyenda 1000 («SON …» del comprobante).
//!
//! ```
//! use cpeforge_core::amount_words::{currency_name, to_invoice};
//! use cpeforge_core::Decimal;
//!
//! let total = Decimal::new(11_850, 2);
//! assert_eq!(
//!     to_invoice(total, "SOLES")?,
//!     "CIENTO DIECIOCHO CON 50/100 SOLES"
//! );
//! assert_eq!(currency_name("USD"), Some("DÓLARES AMERICANOS"));
//! # Ok::<(), cpeforge_core::amount_words::AmountWordsError>(())
//! ```
//!
//! Reglas: el importe se redondea a dos decimales (mitad lejos de cero), la
//! parte entera se escribe en mayúsculas con tildes y los céntimos como
//! `NN/100`. `UNO` se apocopa delante de `MIL`, `MILLÓN`/`MILLONES` y
//! `BILLÓN`/`BILLONES` (`VEINTIÚN MIL`, `TREINTA Y UN MILLONES`), pero se
//! conserva al final (`MIL UNO CON 00/100 SOLES`). Admite hasta 15 dígitos
//! enteros (menos de mil billones).

use std::fmt;

use rust_decimal::{Decimal, RoundingStrategy};

use crate::model::sale::Legend;

/// Código de la leyenda «monto en letras» (catálogo 52).
pub const LEGEND_CODE: &str = "1000";

/// Mayor parte entera admitida: 999 999 999 999 999.
const MAX_WHOLE: u64 = 999_999_999_999_999;

/// Importe que no se puede escribir en letras.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AmountWordsError {
    /// El importe es negativo.
    Negative,
    /// La parte entera tiene más de 15 dígitos.
    TooLarge,
    /// El código de moneda no está en [`currency_name`].
    UnknownCurrency(String),
}

impl fmt::Display for AmountWordsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Negative => f.write_str("importe negativo: no se escribe en letras"),
            Self::TooLarge => f.write_str("importe con más de 15 dígitos enteros"),
            Self::UnknownCurrency(code) => write!(f, "moneda sin nombre en letras: {code}"),
        }
    }
}

impl std::error::Error for AmountWordsError {}

/// Importe en letras: `"TRESCIENTOS TREINTA Y SEIS CON 00/100 SOLES"`.
///
/// `currency` es el nombre que se imprime al final y se pasa a mayúsculas.
/// Para obtenerlo desde el código ISO 4217 usar [`currency_name`].
///
/// # Errors
///
/// [`AmountWordsError::Negative`] si el importe es negativo y
/// [`AmountWordsError::TooLarge`] si la parte entera supera 15 dígitos.
pub fn to_invoice(amount: Decimal, currency: &str) -> Result<String, AmountWordsError> {
    if amount.is_sign_negative() && !amount.is_zero() {
        return Err(AmountWordsError::Negative);
    }
    let fixed = amount
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
        .abs();
    let text = format!("{fixed:.2}");
    let (whole, cents) = text.split_once('.').ok_or(AmountWordsError::TooLarge)?;
    let whole: u64 = whole.parse().map_err(|_| AmountWordsError::TooLarge)?;
    if whole > MAX_WHOLE {
        return Err(AmountWordsError::TooLarge);
    }
    Ok(format!(
        "{} CON {cents}/100 {}",
        integer_words(whole),
        currency.to_uppercase()
    ))
}

/// Nombre en letras de una moneda por su código ISO 4217 (`PEN`, `USD`,
/// `EUR`), tal como se escribe en la leyenda. `None` si no se conoce.
#[must_use]
pub fn currency_name(code: &str) -> Option<&'static str> {
    let code = code.trim();
    [
        ("PEN", "SOLES"),
        ("USD", "DÓLARES AMERICANOS"),
        ("EUR", "EUROS"),
    ]
    .into_iter()
    .find(|(iso, _)| iso.eq_ignore_ascii_case(code))
    .map(|(_, name)| name)
}

impl Legend {
    /// Leyenda 1000 con el importe en letras, a partir del código de moneda
    /// ISO 4217 del comprobante (`tipo_moneda`).
    ///
    /// ```
    /// use cpeforge_core::model::sale::Legend;
    /// use cpeforge_core::Decimal;
    ///
    /// let legend = Legend::amount_in_words(Decimal::new(336, 0), "PEN")?;
    /// assert_eq!(legend.code.as_deref(), Some("1000"));
    /// assert_eq!(
    ///     legend.value.as_deref(),
    ///     Some("TRESCIENTOS TREINTA Y SEIS CON 00/100 SOLES")
    /// );
    /// # Ok::<(), cpeforge_core::amount_words::AmountWordsError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Los de [`to_invoice`] y [`AmountWordsError::UnknownCurrency`] si la
    /// moneda no está en [`currency_name`]; para otra moneda, usar
    /// [`to_invoice`] con su nombre.
    pub fn amount_in_words(total: Decimal, currency_code: &str) -> Result<Self, AmountWordsError> {
        let name = currency_name(currency_code)
            .ok_or_else(|| AmountWordsError::UnknownCurrency(currency_code.to_owned()))?;
        Ok(Self {
            code: Some(LEGEND_CODE.to_owned()),
            value: Some(to_invoice(total, name)?),
        })
    }
}

/// Parte entera en letras (0 a 999 999 999 999 999).
fn integer_words(n: u64) -> String {
    if n == 0 {
        return "CERO".to_owned();
    }
    let billones = n / 1_000_000_000_000;
    let millones = (n / 1_000_000) % 1_000_000;
    let miles = (n / 1_000) % 1_000;
    let unidades = n % 1_000;
    let mut parts: Vec<String> = Vec::new();
    match billones {
        0 => {}
        1 => parts.push("UN BILLÓN".to_owned()),
        b => parts.push(format!("{} BILLONES", below_thousand(b, true))),
    }
    match millones {
        0 => {}
        1 => parts.push("UN MILLÓN".to_owned()),
        m => parts.push(format!("{} MILLONES", below_million(m, true))),
    }
    if miles > 0 || unidades > 0 {
        parts.push(below_million(miles * 1_000 + unidades, false));
    }
    parts.join(" ")
}

/// 1 a 999 999; `apocope` escribe `UN` en lugar de `UNO` al final.
fn below_million(n: u64, apocope: bool) -> String {
    let miles = n / 1_000;
    let resto = n % 1_000;
    let mut parts: Vec<String> = Vec::new();
    match miles {
        0 => {}
        1 => parts.push("MIL".to_owned()),
        m => parts.push(format!("{} MIL", below_thousand(m, true))),
    }
    if resto > 0 {
        parts.push(below_thousand(resto, apocope));
    }
    parts.join(" ")
}

/// 1 a 999; `apocope` escribe `UN`/`VEINTIÚN` en lugar de `UNO`/`VEINTIUNO`.
fn below_thousand(n: u64, apocope: bool) -> String {
    if n == 100 {
        return "CIEN".to_owned();
    }
    match (n / 100, n % 100) {
        (0, resto) => below_hundred(resto, apocope),
        (centenas, 0) => hundreds_word(centenas).to_owned(),
        (centenas, resto) => format!(
            "{} {}",
            hundreds_word(centenas),
            below_hundred(resto, apocope)
        ),
    }
}

/// 1 a 99.
fn below_hundred(n: u64, apocope: bool) -> String {
    if n < 30 {
        return match (n, apocope) {
            (1, true) => "UN".to_owned(),
            (21, true) => "VEINTIÚN".to_owned(),
            _ => units_word(n).to_owned(),
        };
    }
    let decena = tens_word(n / 10);
    match n % 10 {
        0 => decena.to_owned(),
        1 if apocope => format!("{decena} Y UN"),
        u => format!("{decena} Y {}", units_word(u)),
    }
}

/// 1 a 29.
fn units_word(n: u64) -> &'static str {
    match n {
        1 => "UNO",
        2 => "DOS",
        3 => "TRES",
        4 => "CUATRO",
        5 => "CINCO",
        6 => "SEIS",
        7 => "SIETE",
        8 => "OCHO",
        9 => "NUEVE",
        10 => "DIEZ",
        11 => "ONCE",
        12 => "DOCE",
        13 => "TRECE",
        14 => "CATORCE",
        15 => "QUINCE",
        16 => "DIECISÉIS",
        17 => "DIECISIETE",
        18 => "DIECIOCHO",
        19 => "DIECINUEVE",
        20 => "VEINTE",
        21 => "VEINTIUNO",
        22 => "VEINTIDÓS",
        23 => "VEINTITRÉS",
        24 => "VEINTICUATRO",
        25 => "VEINTICINCO",
        26 => "VEINTISÉIS",
        27 => "VEINTISIETE",
        28 => "VEINTIOCHO",
        29 => "VEINTINUEVE",
        _ => "",
    }
}

/// Decenas de 3 a 9.
fn tens_word(n: u64) -> &'static str {
    match n {
        3 => "TREINTA",
        4 => "CUARENTA",
        5 => "CINCUENTA",
        6 => "SESENTA",
        7 => "SETENTA",
        8 => "OCHENTA",
        9 => "NOVENTA",
        _ => "",
    }
}

/// Centenas de 1 a 9 (100 exacto es `CIEN`, ver [`below_thousand`]).
fn hundreds_word(n: u64) -> &'static str {
    match n {
        1 => "CIENTO",
        2 => "DOSCIENTOS",
        3 => "TRESCIENTOS",
        4 => "CUATROCIENTOS",
        5 => "QUINIENTOS",
        6 => "SEISCIENTOS",
        7 => "SETECIENTOS",
        8 => "OCHOCIENTOS",
        9 => "NOVECIENTOS",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn huge_amounts_are_rejected_without_panic() {
        assert_eq!(
            to_invoice(Decimal::MAX, "SOLES"),
            Err(AmountWordsError::TooLarge)
        );
    }

    use super::*;
    use std::str::FromStr;

    fn words(amount: &str) -> String {
        Decimal::from_str(amount)
            .ok()
            .and_then(|d| to_invoice(d, "SOLES").ok())
            .unwrap_or_default()
    }

    #[test]
    fn writes_invoice_amounts() {
        let cases = [
            ("0", "CERO CON 00/100 SOLES"),
            ("0.50", "CERO CON 50/100 SOLES"),
            ("1", "UNO CON 00/100 SOLES"),
            ("15.15", "QUINCE CON 15/100 SOLES"),
            ("16", "DIECISÉIS CON 00/100 SOLES"),
            ("21", "VEINTIUNO CON 00/100 SOLES"),
            ("22", "VEINTIDÓS CON 00/100 SOLES"),
            ("30", "TREINTA CON 00/100 SOLES"),
            ("31", "TREINTA Y UNO CON 00/100 SOLES"),
            ("100", "CIEN CON 00/100 SOLES"),
            ("101", "CIENTO UNO CON 00/100 SOLES"),
            ("118.50", "CIENTO DIECIOCHO CON 50/100 SOLES"),
            ("336", "TRESCIENTOS TREINTA Y SEIS CON 00/100 SOLES"),
            ("1000", "MIL CON 00/100 SOLES"),
            ("1001", "MIL UNO CON 00/100 SOLES"),
            ("2500.10", "DOS MIL QUINIENTOS CON 10/100 SOLES"),
            ("21000", "VEINTIÚN MIL CON 00/100 SOLES"),
            ("31000", "TREINTA Y UN MIL CON 00/100 SOLES"),
            ("100000", "CIEN MIL CON 00/100 SOLES"),
            ("101000", "CIENTO UN MIL CON 00/100 SOLES"),
            ("200000", "DOSCIENTOS MIL CON 00/100 SOLES"),
            ("1000000", "UN MILLÓN CON 00/100 SOLES"),
            ("1000001", "UN MILLÓN UNO CON 00/100 SOLES"),
            ("2000000", "DOS MILLONES CON 00/100 SOLES"),
            ("21000000", "VEINTIÚN MILLONES CON 00/100 SOLES"),
            ("1001000000", "MIL UN MILLONES CON 00/100 SOLES"),
            (
                "1234567.89",
                "UN MILLÓN DOSCIENTOS TREINTA Y CUATRO MIL QUINIENTOS SESENTA Y SIETE CON 89/100 SOLES",
            ),
            (
                "999999999.99",
                "NOVECIENTOS NOVENTA Y NUEVE MILLONES NOVECIENTOS NOVENTA Y NUEVE MIL NOVECIENTOS NOVENTA Y NUEVE CON 99/100 SOLES",
            ),
            ("1000000000000", "UN BILLÓN CON 00/100 SOLES"),
            ("2000000000001", "DOS BILLONES UNO CON 00/100 SOLES"),
        ];
        for (amount, expected) in cases {
            assert_eq!(words(amount), expected, "{amount}");
        }
    }

    #[test]
    fn rounds_half_away_from_zero() {
        assert_eq!(words("10.005"), "DIEZ CON 01/100 SOLES");
        assert_eq!(words("10.004"), "DIEZ CON 00/100 SOLES");
        assert_eq!(words("0.999"), "UNO CON 00/100 SOLES");
    }

    #[test]
    fn rejects_negative_and_too_large() {
        assert_eq!(
            to_invoice(Decimal::from(-5), "SOLES"),
            Err(AmountWordsError::Negative)
        );
        assert!(to_invoice(Decimal::from(-0), "SOLES").is_ok());
        let max = Decimal::from(MAX_WHOLE);
        assert!(to_invoice(max, "SOLES").is_ok());
        assert_eq!(
            to_invoice(max + Decimal::ONE, "SOLES"),
            Err(AmountWordsError::TooLarge)
        );
    }

    #[test]
    fn currency_is_uppercased_and_resolved_by_code() {
        assert_eq!(
            to_invoice(Decimal::from(5), "dólares americanos").unwrap_or_default(),
            "CINCO CON 00/100 DÓLARES AMERICANOS"
        );
        assert_eq!(currency_name(" pen "), Some("SOLES"));
        assert_eq!(currency_name("XYZ"), None);
        let legend = Legend::amount_in_words(Decimal::new(1050, 2), "USD");
        assert_eq!(
            legend,
            Ok(Legend {
                code: Some("1000".into()),
                value: Some("DIEZ CON 50/100 DÓLARES AMERICANOS".into()),
            })
        );
        assert_eq!(
            Legend::amount_in_words(Decimal::ONE, "XYZ"),
            Err(AmountWordsError::UnknownCurrency("XYZ".into()))
        );
    }
}
