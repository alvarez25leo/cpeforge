//! Verificación independiente del oráculo de referencia: propiedades e invariantes de
//! fechas de Perú, códigos CDR, importes y serialización.
//!
//! Las expectativas se calculan aquí con aritmética propia (no con la base tz
//! ni con la referencia), para detectar errores que el oráculo compartiera.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::str::FromStr;

use cpeforge_core::model::response::CdrResponse;
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_core::response_code::{int_cast, is_accepted_code};
use cpeforge_core::{Decimal, Zoned};
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;

/// Fecha civil en Lima calculada con aritmética: UTC-5 fijo desde 1994-04-01.
fn lima_date_by_arithmetic(unix_seconds: i64) -> Date {
    let local_seconds = unix_seconds - 5 * 3600;
    let days = local_seconds.div_euclid(86_400);
    Date::constant(1970, 1, 1)
        .checked_add(jiff::Span::new().days(days))
        .unwrap()
}

fn ymd(date: Date) -> String {
    format!("{:04}{:02}{:02}", date.year(), date.month(), date.day())
}

#[test]
fn xml_id_uses_lima_date_for_every_hour_2023_to_2025() {
    let start = Timestamp::from_str("2023-01-01T00:00:00Z")
        .unwrap()
        .as_second();
    let end = Timestamp::from_str("2026-01-01T00:00:00Z")
        .unwrap()
        .as_second();
    let mut seconds = start;
    while seconds < end {
        // Se recorren horas y también el último segundo antes de cada hora.
        for instant in [seconds, seconds - 1] {
            let zoned = Timestamp::from_second(instant)
                .unwrap()
                .to_zoned(TimeZone::UTC);
            let expected = ymd(lima_date_by_arithmetic(instant));
            let summary = Summary {
                fec_resumen: Some(zoned.clone()),
                correlativo: Some("1".into()),
                ..Summary::default()
            };
            assert_eq!(
                summary.xml_id().unwrap(),
                format!("RC-{expected}-1"),
                "instante {instant}"
            );
            let voided = Voided {
                fec_comunicacion: Some(zoned),
                correlativo: Some("1".into()),
                ..Voided::default()
            };
            assert_eq!(voided.xml_id().unwrap(), format!("RA-{expected}-1"));
            assert_eq!(
                Reversion { voided }.xml_id().unwrap(),
                format!("RR-{expected}-1")
            );
        }
        seconds += 3600;
    }
}

#[test]
fn xml_id_does_not_depend_on_input_zone() {
    let instant = Timestamp::from_str("2024-02-01T04:59:59Z").unwrap();
    for tz in [
        "UTC",
        "America/Lima",
        "Asia/Tokyo",
        "Pacific/Kiritimati",
        "Pacific/Pago_Pago",
        "Europe/Madrid",
    ] {
        let zoned = instant.to_zoned(TimeZone::get(tz).unwrap());
        let summary = Summary {
            fec_resumen: Some(zoned),
            correlativo: Some("9".into()),
            ..Summary::default()
        };
        assert_eq!(summary.xml_id().unwrap(), "RC-20240131-9", "zona {tz}");
    }
}

#[test]
fn lima_summer_time_1994() {
    // Peru usó UTC-4 del 1 de enero al 1 de abril de 1994 (tzdata, zona America/Lima).
    let cases = [
        ("1994-02-15T04:30:00Z", "19940215"),
        ("1994-02-15T03:30:00Z", "19940214"),
        ("1994-04-01T04:30:00Z", "19940331"),
    ];
    for (instant, expected) in cases {
        let zoned = Timestamp::from_str(instant)
            .unwrap()
            .to_zoned(TimeZone::UTC);
        let summary = Summary {
            fec_resumen: Some(zoned),
            ..Summary::default()
        };
        assert_eq!(
            summary.xml_id().unwrap(),
            format!("RC-{expected}-"),
            "instante {instant}"
        );
    }
}

#[test]
fn accepted_codes_follow_the_numeric_rule() {
    for code in -50_000_i64..=50_000 {
        let expected = code == 0 || code >= 4000;
        let text = code.to_string();
        assert_eq!(is_accepted_code(Some(&text)), expected, "código {code}");
        assert_eq!(
            is_accepted_code(Some(&format!(" \t{text}xyz"))),
            expected,
            "código {code} con prefijo y sufijo"
        );
        let cdr = CdrResponse {
            code: Some(text),
            ..CdrResponse::default()
        };
        assert_eq!(cdr.is_accepted(), expected);
    }
}

#[test]
fn integer_strings_cast_exactly() {
    // Generador lineal congruente determinista: recorre todo el rango i64.
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..200_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let value = i64::from_ne_bytes(state.to_ne_bytes());
        assert_eq!(int_cast(&value.to_string()), value);
        // Con parte decimal la referencia pasa por `double`: `.999` solo se conserva
        // por debajo de 2^40 (separación entre doubles < 0.001).
        if value.unsigned_abs() < (1 << 40) {
            assert_eq!(
                int_cast(&format!("{value}.999")),
                value,
                "truncamiento hacia cero de {value}.999"
            );
        }
    }
    assert_eq!(int_cast(&i64::MAX.to_string()), i64::MAX);
    assert_eq!(int_cast(&i64::MIN.to_string()), i64::MIN);
    assert_eq!(int_cast("99999999999999999999999"), i64::MAX);
    assert_eq!(int_cast("-99999999999999999999999"), i64::MIN);
}

#[test]
fn decimals_keep_scale_and_sign() {
    for text in [
        "0",
        "0.00",
        "100.50",
        "-15.30",
        "0.000001",
        "79228162514264337593543950335",
        "-0.0000000000000000000000000001",
    ] {
        let decimal = Decimal::from_str(text).unwrap();
        assert_eq!(decimal.to_string(), text);
    }
    assert_eq!(
        Decimal::from_str("100.50").unwrap(),
        Decimal::from_str("100.5").unwrap()
    );
}

/// La zona de Perú se resuelve una vez y se reutiliza desde varios hilos.
#[test]
fn peru_time_zone_is_cached_and_thread_safe() {
    use cpeforge_core::time::{PERU_TIME_ZONE, peru_time_zone, to_peru_time};

    let handles: Vec<_> = (0..8)
        .map(|_| std::thread::spawn(|| peru_time_zone().unwrap()))
        .collect();
    for handle in handles {
        let tz = handle.join().unwrap();
        assert_eq!(tz.iana_name(), Some(PERU_TIME_ZONE));
    }
    let utc = Zoned::from_str("2024-02-01T04:30:00+00:00[UTC]").unwrap();
    let lima = to_peru_time(&utc).unwrap();
    assert_eq!(lima.to_string(), "2024-01-31T23:30:00-05:00[America/Lima]");
    assert_eq!(
        utc.time_zone().iana_name(),
        Some("UTC"),
        "no modifica la entrada"
    );
}

#[test]
fn zoned_keeps_named_zone_and_offset() {
    let zoned = Zoned::from_str("2024-01-31T23:30:00-05:00[America/Lima]").unwrap();
    assert_eq!(zoned.time_zone().iana_name(), Some("America/Lima"));
    assert_eq!(zoned.offset().seconds(), -18_000);
}

#[cfg(feature = "serde")]
mod serde_contract {
    use cpeforge_core::model::company::Address;
    use cpeforge_core::model::response::BillResult;
    use cpeforge_core::model::sale::{Charge, Invoice, Note};
    use cpeforge_core::{Decimal, Document};
    use serde_json::json;

    #[test]
    fn amounts_round_trip_as_exact_text() {
        let charge: Charge =
            serde_json::from_value(json!({"monto": "100.50", "factor": "-0.00", "montoBase": 7}))
                .unwrap();
        assert_eq!(charge.monto, Some(Decimal::new(10050, 2)));
        assert!(charge.factor.unwrap().is_sign_negative());
        let back = serde_json::to_value(&charge).unwrap();
        assert_eq!(back["monto"], "100.50");
        assert_eq!(back["factor"], "-0.00");
        assert_eq!(back["montoBase"], "7");
        assert_eq!(back["codTipo"], serde_json::Value::Null);
        let scientific: Charge = serde_json::from_value(json!({"monto": "1.5e3"})).unwrap();
        assert_eq!(scientific.monto, Some(Decimal::from(1500)));
    }

    #[test]
    fn lossy_or_invalid_amounts_are_rejected() {
        assert!(
            serde_json::from_value::<Charge>(json!({"monto": 100.5})).is_err(),
            "número JSON con decimales"
        );
        assert!(serde_json::from_value::<Charge>(json!({"monto": "abc"})).is_err());
        assert!(
            serde_json::from_value::<Charge>(json!({"monto": "0.12345678901234567890123456789"}))
                .is_err()
        );
        assert!(
            serde_json::from_value::<Charge>(json!({"montos": "1"})).is_err(),
            "campo desconocido"
        );
    }

    /// `rust_decimal` admite `_` como separador; un importe así es ambiguo
    /// (`"1_00"` valdría 100) y se rechaza en cualquier posición.
    #[test]
    fn underscore_separators_are_rejected() {
        for text in ["1_000", "1_00.50", "1.0_5", "1_0e2", "_1", "1_"] {
            let error = serde_json::from_value::<Charge>(json!({ "monto": text })).unwrap_err();
            assert!(error.to_string().contains("'_'"), "{text}: {error}");
        }
    }

    /// Un texto enorme no se copia entero en el mensaje de error.
    #[test]
    fn error_messages_do_not_echo_huge_inputs() {
        let huge = "9".repeat(1_000_000);
        let amount = serde_json::from_value::<Charge>(json!({ "monto": huge })).unwrap_err();
        let date = serde_json::from_value::<Invoice>(json!({ "fechaEmision": huge })).unwrap_err();
        for error in [amount.to_string(), date.to_string()] {
            assert!(error.len() < 400, "{} bytes", error.len());
            assert!(error.contains("1000000 bytes"), "{error}");
        }
        // Una entrada corta aparece completa, multibyte incluido.
        let short = serde_json::from_value::<Charge>(json!({ "monto": "ñ1" })).unwrap_err();
        assert!(short.to_string().contains("\"ñ1\""), "{short}");
    }

    /// La serialización sin `String` intermedio produce exactamente el mismo
    /// texto que `Display` y Base64 estándar, en los extremos del rango.
    #[test]
    fn serialized_text_matches_display() {
        use base64::Engine as _;
        use base64::engine::general_purpose::STANDARD;
        use cpeforge_core::Zoned;

        for text in [
            "0",
            "-0.00",
            "79228162514264337593543950335",
            "-79228162514264337593543950335",
            "0.0000000000000000000000000001",
            "18.50",
        ] {
            let charge: Charge = serde_json::from_value(json!({ "monto": text })).unwrap();
            assert_eq!(serde_json::to_value(&charge).unwrap()["monto"], text);
        }
        for text in [
            "2024-01-31T23:30:00-05:00[America/Lima]",
            "-009999-01-02T01:59:59+00:00[UTC]",
            "9999-12-30T22:00:00+00:00[UTC]",
        ] {
            let zoned: Zoned = text.parse().unwrap();
            let invoice: Invoice = serde_json::from_value(json!({ "fechaEmision": text })).unwrap();
            assert_eq!(
                serde_json::to_value(&invoice).unwrap()["fechaEmision"],
                zoned.to_string()
            );
        }
        let bytes: Vec<u8> = (0..=255u8).cycle().take(70_001).collect();
        let bill = BillResult {
            cdr_zip: Some(bytes.clone()),
            ..BillResult::default()
        };
        let value = serde_json::to_value(&bill).unwrap();
        assert_eq!(value["cdrZip"], STANDARD.encode(&bytes));
        let back: BillResult = serde_json::from_value(value).unwrap();
        assert_eq!(back.cdr_zip, Some(bytes));
    }

    #[test]
    fn reference_names_and_defaults() {
        let note: Note = serde_json::from_value(json!({
            "numDocfectado": "F001-1", "mtoIGV": "18", "company": {"ruc": "20000000001"}, "tipoDoc": "07", "serie": "FC01", "correlativo": "1"
        }))
        .unwrap();
        assert_eq!(note.num_doc_afectado.as_deref(), Some("F001-1"));
        assert_eq!(note.base.mto_igv, Some(Decimal::from(18)));
        assert_eq!(note.base.ubl_version.as_deref(), Some("2.0"));
        assert_eq!(note.name().unwrap(), "20000000001-07-FC01-1");
        let address: Address = serde_json::from_value(json!({"codigoPais": null})).unwrap();
        assert_eq!(
            address.codigo_pais, None,
            "null explícito sustituye el valor por defecto"
        );
        assert_eq!(
            address.cod_local.as_deref(),
            Some("0000"),
            "ausencia conserva el valor por defecto"
        );
    }

    #[test]
    fn full_models_round_trip() {
        let invoice: Invoice = serde_json::from_value(json!({
            "serie": "F001", "fechaEmision": "2024-01-31T23:30:00-05:00", "details": [{"cantidad": "2.000", "atributos": []}],
            "formaPago": {"tipo": "Contado"}, "guias": [{"tipoDoc": "09", "nroDoc": "T001-1"}]
        }))
        .unwrap();
        let text = serde_json::to_string(&invoice).unwrap();
        let back: Invoice = serde_json::from_str(&text).unwrap();
        assert_eq!(back, invoice);
        let date = invoice.base.fecha_emision.as_ref().unwrap();
        assert_eq!(date.offset().seconds(), -18_000);
        assert_eq!(
            serde_json::to_value(&invoice).unwrap()["fechaEmision"],
            "2024-01-31T23:30:00-05:00[-05:00]"
        );

        let bill: BillResult =
            serde_json::from_value(json!({"success": true, "cdrZip": "UEsDBAD/"})).unwrap();
        assert_eq!(
            bill.cdr_zip.as_deref(),
            Some(&[b'P', b'K', 3, 4, 0, 0xff][..])
        );
        assert_eq!(serde_json::to_value(&bill).unwrap()["cdrZip"], "UEsDBAD/");
    }
}

/// `(int) $string` comparado con los resultados reales de la referencia
/// (el oráculo externo (`core`)).
#[test]
fn int_cast_matches_reference_oracle() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/reference/core/int-cast/expected.json");
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        expected["reference_int_size"], 8,
        "el oráculo debe ser de 64 bits"
    );
    let results = expected["results"].as_array().unwrap();
    assert!(results.len() >= 70, "faltan entradas del oráculo");
    for result in results {
        let input = result["input"].as_str().unwrap();
        assert_eq!(
            Some(int_cast(input)),
            result["int"].as_i64(),
            "entrada {input:?}"
        );
    }
}
