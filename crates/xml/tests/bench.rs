//! Medición de rendimiento del render (no se ejecuta por defecto).
//!
//! ```bash
//! cargo test --release -p cpeforge-xml --all-features --test bench -- --ignored --nocapture
//! ```
//!
//! Usa los casos sintéticos de `tests/reference/xml/cases` y comprobantes
//! grandes derivados de ellos (líneas repetidas). Cada cifra es el promedio de
//! N iteraciones tras una de calentamiento; el modelo se construye antes de
//! medir, así que solo se mide el render.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "../../core/tests/support/hydrate.rs"]
mod hydrate;

use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::sale::{Invoice, Note};
use cpeforge_core::model::summary::Summary;
use cpeforge_xml::{DespatchBuilder, InvoiceBuilder, NoteBuilder, RenderOptions, SummaryBuilder};
use hydrate::{deserialize, to_serde};
use jiff::Timestamp;
use serde_json::Value;

fn case(family: &str, id: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/reference/xml/cases")
        .join(format!("{family}.json"));
    let cases: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("sin caso {id}"))["input"]
        .clone()
}

/// Repite las líneas `details` hasta tener `lines`.
fn scaled(mut input: Value, lines: usize) -> Value {
    let details = input["details"].as_array().unwrap().clone();
    input["details"] = Value::Array(details.iter().cycle().take(lines).cloned().collect());
    input
}

fn model<T: serde::de::DeserializeOwned>(input: &Value) -> T {
    deserialize(to_serde(input).unwrap().unwrap()).unwrap()
}

#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "solo informe"
)]
fn time(label: &str, iters: u32, mut f: impl FnMut() -> usize) {
    let bytes = black_box(f());
    let started = Instant::now();
    for _ in 0..iters {
        black_box(f());
    }
    let per = started.elapsed() / iters;
    let mib_s = (bytes as f64 / 1_048_576.0) / per.as_secs_f64();
    println!(
        "{label:<36} {:>9} µs/op  {:>8} B  {mib_s:>7.1} MiB/s",
        per.as_micros(),
        bytes
    );
}

#[test]
#[ignore = "benchmark: ejecutar con --release --ignored"]
fn render_throughput() {
    let options = RenderOptions {
        now: Some(Timestamp::from_second(1_706_745_600).unwrap()),
        ..RenderOptions::default()
    };
    let invoice = InvoiceBuilder::new(options.clone());
    let note = NoteBuilder::new(options.clone());
    let despatch = DespatchBuilder::new(options.clone());
    let summary = SummaryBuilder::new(options);

    for lines in [1usize, 100, 1000] {
        println!("== {lines} líneas ==");
        let iters = if lines >= 1000 { 50 } else { 500 };
        let doc: Invoice = model(&scaled(case("invoice", "inv21-full-default"), lines));
        time("factura UBL 2.1", iters, || {
            invoice.render(&doc).unwrap().len()
        });
        let doc: Invoice = model(&scaled(case("invoice", "inv20-full-default"), lines));
        time("factura UBL 2.0", iters, || {
            invoice.render(&doc).unwrap().len()
        });
        let doc: Note = model(&scaled(case("note", "cr21-full-default"), lines));
        time("nota de crédito 2.1", iters, || {
            note.render(&doc).unwrap().len()
        });
        let doc: Despatch = model(&scaled(case("despatch", "desp2022-full-default"), lines));
        time("guía 2022", iters, || despatch.render(&doc).unwrap().len());
        let doc: Summary = model(&scaled(case("summary", "summary-full-default"), lines));
        time("resumen diario", iters, || {
            summary.render(&doc).unwrap().len()
        });
    }
}

/// Un único render de una factura de 5000 líneas, para medir el pico de
/// memoria del proceso (`/usr/bin/time -l`).
#[test]
#[ignore = "benchmark: ejecutar con --release --ignored"]
fn render_peak_memory() {
    // El modelo se amplía en Rust (sin JSON) para que el pico lo dé el render.
    let mut doc: Invoice = model(&case("invoice", "inv21-full-default"));
    let details = doc.base.details.take().unwrap();
    doc.base.details = Some(details.iter().cycle().take(5000).cloned().collect());
    let options = RenderOptions {
        now: Some(Timestamp::from_second(1_706_745_600).unwrap()),
        ..RenderOptions::default()
    };
    let xml = InvoiceBuilder::new(options).render(&doc).unwrap();
    println!("XML: {} bytes", xml.len());
}
