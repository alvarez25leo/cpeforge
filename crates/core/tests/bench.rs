//! Medición de serialización y de identificadores (no se ejecuta por defecto).
//!
//! ```bash
//! cargo test --release -p cpeforge-core --all-features --test bench -- --ignored --nocapture
//! ```
//!
//! Cada cifra es el promedio de N iteraciones tras una de calentamiento.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::hint::black_box;
use std::time::Instant;

use cpeforge_core::Document;
use cpeforge_core::model::response::BillResult;
use cpeforge_core::model::sale::Invoice;
use cpeforge_core::model::summary::Summary;
use serde_json::{Value, json};

fn invoice_json(lines: usize) -> String {
    let detail = json!({
        "codProducto": "P001", "unidad": "NIU", "cantidad": "2.000", "descripcion": "PRODUCTO",
        "mtoValorUnitario": "50.0000000001", "mtoBaseIgv": "100.00", "porcentajeIgv": "18.00",
        "igv": "18.00", "tipAfeIgv": "10", "totalImpuestos": "18.00", "mtoValorVenta": "100.00",
        "mtoPrecioUnitario": "59.00", "factorIcbper": "0.50", "atributos": []
    });
    let value = json!({
        "ublVersion": "2.1", "tipoDoc": "01", "serie": "F001", "correlativo": "123",
        "fechaEmision": "2024-01-31T23:30:00-05:00[America/Lima]",
        "company": {"ruc": "20000000001", "razonSocial": "EMPRESA"},
        "mtoOperGravadas": "100.00", "mtoIGV": "18.00", "mtoImpVenta": "118.00",
        "details": Value::Array(vec![detail; lines]),
    });
    value.to_string()
}

fn time(label: &str, iters: u32, mut f: impl FnMut() -> usize) {
    black_box(f());
    let started = Instant::now();
    for _ in 0..iters {
        black_box(f());
    }
    println!(
        "{label:<40} {:>9} ns/op",
        (started.elapsed() / iters).as_nanos()
    );
}

#[test]
#[ignore = "benchmark: ejecutar con --release --ignored"]
fn serde_and_identifiers() {
    for lines in [1usize, 1000] {
        let iters = if lines >= 1000 { 100 } else { 20_000 };
        let text = invoice_json(lines);
        let invoice: Invoice = serde_json::from_str(&text).unwrap();
        println!(
            "== factura de {lines} líneas ({} bytes JSON) ==",
            text.len()
        );
        time("deserializar", iters, || {
            serde_json::from_str::<Invoice>(&text)
                .unwrap()
                .base
                .details
                .map_or(0, |d| d.len())
        });
        time("serializar", iters, || {
            serde_json::to_string(&invoice).unwrap().len()
        });
    }
    let bill = BillResult {
        cdr_zip: Some(vec![7u8; 1 << 20]),
        ..BillResult::default()
    };
    time("serializar CDR de 1 MiB (Base64)", 200, || {
        serde_json::to_string(&bill).unwrap().len()
    });
    let summary: Summary = serde_json::from_value(json!({
        "correlativo": "001", "fecResumen": "2024-02-01T04:30:00+00:00[UTC]",
        "company": {"ruc": "20000000001"}
    }))
    .unwrap();
    time("Summary::name (RC + fecha de Perú)", 200_000, || {
        summary.name().unwrap().len()
    });
}
