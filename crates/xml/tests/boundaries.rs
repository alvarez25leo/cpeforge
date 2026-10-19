//! Propiedades independientes del oráculo: formato numérico, `spaceless`,
//! escape y XML bien formado.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::str::FromStr;

use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, SaleDetail};
use cpeforge_xml::format::{float_string, n_format, n_format_limit};
use cpeforge_xml::{InvoiceBuilder, RenderOptions};

/// Generador congruente lineal determinista.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    fn decimal(&mut self) -> Decimal {
        let digits = self.next() % 99_999_999_999_999; // como máximo 14 cifras
        let scale = u32::try_from(self.next() % 15).unwrap();
        let mut value = Decimal::from_i128_with_scale(i128::from(digits), scale);
        if self.next() % 2 == 0 {
            value.set_sign_negative(true);
        }
        value
    }
}

#[test]
fn float_text_round_trips_for_14_digits() {
    let mut rng = Lcg(42);
    for _ in 0..100_000 {
        let value = rng.decimal();
        let text = float_string(value);
        let parsed = if text.contains('E') {
            Decimal::from_scientific(&text.replace("E+", "e").replace('E', "e")).unwrap()
        } else {
            Decimal::from_str(&text).unwrap()
        };
        assert_eq!(parsed, value, "{value} → {text}");
        assert!(
            !text.contains('E')
                || value.abs() >= Decimal::from(100_000_000_000_000_i64)
                || value.abs() < Decimal::new(1, 4),
            "{value} → {text}"
        );
    }
}

#[test]
fn n_format_has_exact_decimals_and_no_negative_zero() {
    let mut rng = Lcg(7);
    for _ in 0..100_000 {
        let value = rng.decimal();
        let decimals = u32::try_from(rng.next() % 7).unwrap();
        let text = n_format(Some(value), decimals);
        let fraction = text.split('.').nth(1).map_or(0, str::len);
        assert_eq!(
            fraction, decimals as usize,
            "{value} con {decimals}: {text}"
        );
        let parsed = Decimal::from_str(&text).unwrap();
        assert!(
            !(parsed.is_zero() && text.starts_with('-')),
            "{value}: {text}"
        );
        let diff = (parsed - value).abs();
        assert!(
            diff <= Decimal::new(5, decimals),
            "{value} con {decimals}: {text}"
        );
        let limited = n_format_limit(Some(value), decimals);
        assert!(
            !limited.contains('E')
                && limited.split('.').nth(1).map_or(0, str::len) <= decimals as usize,
            "{value}: {limited}"
        );
    }
}

fn invoice_with_text(text: &str) -> Invoice {
    let client = Client {
        tipo_doc: Some(text.into()),
        num_doc: Some(text.into()),
        rzn_social: Some("R".into()),
        ..Client::default()
    };
    Invoice {
        base: BaseSale {
            ubl_version: Some("2.1".into()),
            serie: Some(text.into()),
            correlativo: Some(text.into()),
            tipo_moneda: Some(text.into()),
            company: Some(Company {
                ruc: Some(text.into()),
                address: Some(Address {
                    distrito: Some(text.into()),
                    ..Address::default()
                }),
                ..Company::default()
            }),
            client: Some(client),
            legends: Some(vec![Legend {
                code: Some(text.into()),
                value: Some(text.into()),
            }]),
            details: Some(vec![SaleDetail {
                unidad: Some(text.into()),
                cod_producto: Some(text.into()),
                ..SaleDetail::default()
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: Some(text.into()),
        ..Invoice::default()
    }
}

#[test]
fn html_escaping_keeps_xml_well_formed() {
    let pieces = [
        "&",
        "<",
        ">",
        "\"",
        "'",
        "&amp;",
        "]]",
        "]]>",
        "<![CDATA[",
        "x",
        " ",
        "\t",
        "ñ",
        "😀",
        "\u{a0}",
    ];
    let builder = InvoiceBuilder::new(RenderOptions {
        now: Some("2024-01-01T00:00:00Z".parse().unwrap()),
        ..RenderOptions::default()
    });
    let mut rng = Lcg(3);
    for _ in 0..2_000 {
        let len = rng.next() % 6;
        let text: String = (0..len)
            .map(|_| pieces[usize::try_from(rng.next()).unwrap() % pieces.len()])
            .collect();
        let xml = builder.render(&invoice_with_text(&text)).unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap_or_else(|err| panic!("{text:?}: {err}"));
        let id = doc
            .descendants()
            .find(|n| n.tag_name().name() == "ID")
            .unwrap();
        let expected = format!("{text}-{text}");
        // `spaceless` solo altera texto formado únicamente por espacios entre etiquetas.
        if !expected.trim().is_empty() {
            assert_eq!(id.text().unwrap_or_default(), expected, "{text:?}");
        }
    }
}
