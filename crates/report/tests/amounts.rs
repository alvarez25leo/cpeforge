//! Importes en la frontera de la plantilla: `Decimal` → `double` de la referencia.
//!
//! Las plantillas formatean con `number_format` sobre el `double` que la referencia
//! obtiene del texto decimal (ver `cpeforge_report::format`). Estas pruebas
//! miden cuándo ese texto coincide con el redondeo decimal exacto
//! (`MidpointAwayFromZero`, el de `number_format`) y cuándo no, para la
//! decisión pendiente sobre `f64` registrada en la evidencia del paquete.
//! No cambian el comportamiento: lo caracterizan.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use cpeforge_core::Decimal;
use cpeforge_report::format::{Number, decimal_to_f64, float_to_string, number_format};
use rust_decimal::RoundingStrategy;

/// Generador determinista (xorshift64).
fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut state = seed;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    }
}

/// `number_format` con aritmética decimal exacta y redondeo half-up.
fn exact_format(value: Decimal, decimals: u32) -> String {
    let mut rounded =
        value.round_dp_with_strategy(decimals, RoundingStrategy::MidpointAwayFromZero);
    if rounded.is_zero() {
        // `number_format` no imprime `-0.00`.
        rounded.set_sign_positive(true);
    }
    let width = usize::try_from(decimals).unwrap();
    format!("{rounded:.width$}")
}

fn reference_format(value: Decimal, decimals: u32) -> String {
    number_format(Number::Float(decimal_to_f64(value)), i64::from(decimals))
}

/// Decimal aleatorio con `digits` cifras significativas y `scale` decimales.
fn random_decimal(next: &mut impl FnMut() -> u64, digits: u32, scale: u32) -> Decimal {
    let limit = 10u64.pow(digits);
    let mantissa = i64::try_from(next() % limit).unwrap();
    let sign = if next() % 4 == 0 { -1 } else { 1 };
    Decimal::new(sign * mantissa, scale)
}

/// `2^50`. El redondeo de la referencia compara el `double` del importe con el del
/// punto medio `(k + 0.5) / 10^d`. Si el importe tiene `d` decimales o menos,
/// ambos distan al menos `0.5 · 10^-d`; mientras `|x| · 10^d < 2^50`, esa
/// distancia supera dos ulp de `x` y los dos `double` no se confunden. Por
/// encima, la referencia puede sumar una unidad en la última cifra (ver
/// `reference_precision_edge_is_reproduced`). Con más decimales que `d` y como
/// mucho 15 cifras significativas, la distancia mínima `10^-escala` sigue
/// siendo de más de cuatro ulp.
const EXACT_SCALED_LIMIT: i64 = 1 << 50;

fn within_exact_range(value: Decimal, decimals: u32) -> bool {
    let scaled = value.abs() * Decimal::from(10u64.pow(decimals));
    scaled < Decimal::from(EXACT_SCALED_LIMIT)
}

#[test]
fn reference_formatting_equals_exact_decimal_while_scaled_value_is_below_2_pow_50() {
    let mut next = rng(0x5DEE_CE66_D1CE_4E5B);
    let mut checked = 0;
    for digits in 1..=15 {
        for scale in 0..=digits.min(10) {
            for _ in 0..1500 {
                let value = random_decimal(&mut next, digits, scale);
                for decimals in [0, 2, 3, 4] {
                    if !within_exact_range(value, decimals) {
                        continue;
                    }
                    assert_eq!(
                        reference_format(value, decimals),
                        exact_format(value, decimals),
                        "{value} con {decimals} decimales"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 500_000);
}

#[test]
fn bound_holds_densely_just_below_2_pow_50() {
    // Importes con 0 a 2 decimales (escala ≤ decimales mostrados) justo por
    // debajo del límite, formateados con 2 decimales. Tienen hasta 16 cifras
    // significativas: en esta rama el límite no depende de ellas.
    let mut next = rng(0x6A09_E667_F3BC_C908);
    let top = EXACT_SCALED_LIMIT / 100;
    for _ in 0..200_000 {
        let scale = u32::try_from(next() % 3).unwrap();
        let unit = 10i64.pow(scale);
        let below = i64::try_from(next() % 1_000_000_000).unwrap() + 1;
        let mantissa = (top - 1) * unit - below;
        let value = Decimal::new(mantissa, scale);
        assert!(within_exact_range(value, 2));
        assert_eq!(
            reference_format(value, 2),
            exact_format(value, 2),
            "{value}"
        );
    }
}

#[test]
fn exact_midpoints_round_half_up_like_decimal() {
    // Mitades exactas en decimal que no son exactas en binario.
    for (text, expected) in [
        ("1.005", "1.01"),
        ("2.675", "2.68"),
        ("0.285", "0.29"),
        ("-0.285", "-0.29"),
        ("1234567890.125", "1234567890.13"),
        ("999999999999.995", "1000000000000.00"),
        ("-0.004", "0.00"),
        ("-0.005", "-0.01"),
    ] {
        let value: Decimal = text.parse().unwrap();
        assert_eq!(reference_format(value, 2), expected, "{text}");
        assert_eq!(exact_format(value, 2), expected, "{text}");
    }
}

#[test]
fn reference_precision_edge_is_reproduced() {
    // Salida real de la referencia (`number_format($v, $d, '.', '')`):
    // cerca de `|v| · 10^d ≈ 2^51` el redondeo de la referencia puede sumar una unidad
    // a la última cifra. El crate reproduce a la referencia; con dos decimales no
    // ocurre por debajo de ≈ 11 billones (`2^50 / 100`), muy por encima de
    // cualquier importe real.
    for (text, decimals, reference) in [
        ("506978279587", 4, "506978279587.0001"),
        ("45035996273704.97", 2, "45035996273704.97"),
        ("45035996273704.96", 2, "45035996273704.97"),
        ("4503599627370.4", 3, "4503599627370.401"),
        ("99999999999.99", 2, "99999999999.99"),
        ("450359962737.0496", 4, "450359962737.0497"),
        ("414464166422.4", 4, "414464166422.4001"),
        ("41446416642.24", 4, "41446416642.2400"),
    ] {
        let value: Decimal = text.parse().unwrap();
        assert_eq!(reference_format(value, decimals), reference, "{text}");
    }
}

#[test]
fn above_15_significant_digits_reference_and_exact_decimal_diverge() {
    // Con más cifras de las que guarda un `double`, la referencia (y por tanto el HTML
    // de la referencia) pierde precisión. El crate reproduce a la referencia: el caso
    // `invoice-amounts-edge` del corpus fija este comportamiento.
    let value: Decimal = "12345678901234567.89".parse().unwrap();
    assert_eq!(reference_format(value, 2), "12345678901234568.00");
    assert_eq!(exact_format(value, 2), "12345678901234567.89");
}

#[test]
fn raw_float_printing_matches_normalized_decimal_up_to_14_digits() {
    // `{{ doc.mtoIGV }}` sin filtro imprime el `double` con `precision=14`.
    let mut next = rng(0x2545_F491_4F6C_DD1D);
    for digits in 1..=14 {
        for scale in 0..=digits.min(4) {
            for _ in 0..1000 {
                let value = random_decimal(&mut next, digits, scale);
                if value.is_zero() {
                    continue;
                }
                assert_eq!(
                    float_to_string(decimal_to_f64(value)),
                    value.normalize().to_string(),
                    "{value}"
                );
            }
        }
    }
}
