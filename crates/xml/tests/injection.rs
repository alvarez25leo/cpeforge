//! Regresión de seguridad para todos los comprobantes: un texto con
//! delimitadores XML no puede romper el documento ni insertar elementos.
//!
//! Toma cada caso sintético con las opciones por defecto (escape `html`),
//! añade `]]><x/>&<"'` a todos sus textos libres y exige que el XML siga bien
//! formado y conserve el texto. Cubre tanto los valores escapados como los que
//! van en `CDATA` sin escape.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "../../core/tests/support/hydrate.rs"]
mod hydrate;

use std::fs;
use std::path::PathBuf;

use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{Invoice, Note};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_xml::{
    DespatchBuilder, InvoiceBuilder, NoteBuilder, PerceptionBuilder, RenderOptions,
    RetentionBuilder, SummaryBuilder, VoidedBuilder, XmlError,
};
use hydrate::{deserialize, to_serde};
use jiff::Timestamp;
use serde_json::Value;

const PAYLOAD: &str = "]]><x/>&<\"'";

/// Campos de texto libre que las plantillas imprimen tal cual (sin códigos de
/// catálogo, versiones ni identificadores que eligen la plantilla).
const FREE_TEXT: &[&str] = &[
    "razonSocial",
    "nombreComercial",
    "rznSocial",
    "direccion",
    "descripcion",
    "observacion",
    "desMotivo",
    "desMotivoBaja",
    "desTraslado",
    "value",
    "urbanizacion",
    "departamento",
    "provincia",
    "distrito",
    "nombre",
    "name",
    "email",
    "telephone",
];

fn inject(value: &mut Value) -> usize {
    match value {
        Value::Object(map) => map
            .iter_mut()
            .map(|(key, child)| match child {
                Value::String(text) if FREE_TEXT.contains(&key.as_str()) => {
                    text.push_str(PAYLOAD);
                    1
                }
                _ => inject(child),
            })
            .sum(),
        Value::Array(items) => items.iter_mut().map(inject).sum(),
        _ => 0,
    }
}

fn render(builder: &str, class: &str, input: Value) -> Option<Result<String, XmlError>> {
    let options = RenderOptions {
        now: Some(Timestamp::from_second(1_706_745_600).unwrap()),
        ..RenderOptions::default()
    };
    let value = to_serde(&input).unwrap().ok()?;
    macro_rules! go {
        ($ty:ty, $b:expr, $m:ident) => {
            deserialize::<$ty>(value).ok().map(|doc| $b.$m(&doc))
        };
    }
    match (builder, class) {
        ("InvoiceBuilder", "Invoice") => go!(Invoice, InvoiceBuilder::new(options), render),
        ("NoteBuilder", "Note") => go!(Note, NoteBuilder::new(options), render),
        ("DespatchBuilder", "Despatch") => go!(Despatch, DespatchBuilder::new(options), render),
        ("SummaryBuilder", "Summary") => go!(Summary, SummaryBuilder::new(options), render),
        ("VoidedBuilder", "Voided") => go!(Voided, VoidedBuilder::new(options), render),
        ("VoidedBuilder", "Reversion") => {
            go!(Reversion, VoidedBuilder::new(options), render_reversion)
        }
        ("PerceptionBuilder", "Perception") => {
            go!(Perception, PerceptionBuilder::new(options), render)
        }
        ("RetentionBuilder", "Retention") => go!(Retention, RetentionBuilder::new(options), render),
        _ => None,
    }
}

#[test]
fn delimiters_in_free_text_never_break_the_document() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/xml/cases");
    let mut checked = 0;
    let mut problems = Vec::new();
    let mut families: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    families.sort();
    for file in families {
        let cases: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        for case in cases["cases"].as_array().unwrap() {
            if case["options"].as_object().is_some_and(|o| !o.is_empty()) {
                continue; // sin autoescape el XML mal formado es el contrato
            }
            let id = case["id"].as_str().unwrap();
            let mut input = case["input"].clone();
            let injected = inject(&mut input);
            if injected == 0 {
                continue;
            }
            let builder = case["builder"].as_str().unwrap();
            let class = input["$class"].as_str().unwrap_or_default().to_owned();
            let Some(Ok(xml)) = render(builder, &class, input) else {
                continue; // caso de error del oráculo: no hay XML
            };
            match roxmltree::Document::parse(&xml) {
                Ok(doc) => {
                    if doc.descendants().any(|n| n.has_tag_name("x")) {
                        problems.push(format!("{id}: se insertó un elemento <x/>"));
                    }
                    let text: String = doc.descendants().filter_map(|n| n.text()).collect();
                    if !text.contains(PAYLOAD) {
                        problems.push(format!("{id}: el texto no se conservó"));
                    }
                    checked += 1;
                }
                Err(err) => problems.push(format!("{id}: XML mal formado: {err}")),
            }
        }
    }
    println!("casos con texto hostil: {checked}");
    assert!(checked > 40, "cobertura insuficiente: {checked}");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
