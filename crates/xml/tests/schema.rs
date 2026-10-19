//! Verificación independiente con XSD.
//!
//! Valida con `xmllint` (libxml2), un validador distinto del generador, el XML
//! que produce Rust para cada caso y también el XML original de la referencia, usando
//! los esquemas UBL 2.3.0 fijados (preparados por
//! `scripts/prepare_ubl_validator_reference.sh` en `target/reference`).
//!
//! Adaptación documentada: igual que `XsdValidatorTrait` de las pruebas de la referencia,
//! a los documentos UBL 2.1 se les añade `<cbc:AccountID>1</cbc:AccountID>`
//! dentro del primer `ext:ExtensionContent` (vacío hasta que se firme), **en
//! una copia temporal**. El fixture original no se modifica.
//!
//! Cada caso debe validar, salvo los de `EXPECTED_INVALID`, que documentan por
//! qué la referencia genera ese XML no válido; para ellos se exige que falle. En
//! `REFERENCE_ONLY_INVALID` solo falla el XML de la referencia y se exige que el de Rust valide.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "../../core/tests/support/hydrate.rs"]
mod hydrate;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{Invoice, Note};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_xml::{
    Autoescape, DespatchBuilder, InvoiceBuilder, NoteBuilder, PerceptionBuilder, RenderOptions,
    RetentionBuilder, SummaryBuilder, VoidedBuilder,
};
use hydrate::{TestResult, deserialize, harness_error, to_serde};
use jiff::Timestamp;
use serde_json::Value;

/// Casos en los que solo el XML de la referencia es inválido: Rust corrige el defecto
/// (diferencia deliberada registrada en `differences.json`) y su XML debe
/// validar.
const REFERENCE_ONLY_INVALID: &[(&str, &str)] = &[
    (
        "inv20-special-default",
        "`]]>` en una descripción con |raw dentro de CDATA: la referencia genera XML mal formado; Rust divide la sección CDATA",
    ),
    (
        "inv21-special-default",
        "`]]>` en una descripción con |raw dentro de CDATA: la referencia genera XML mal formado; Rust divide la sección CDATA",
    ),
];

/// Casos cuyo XML (la referencia y Rust) no debe validar, con el motivo.
const EXPECTED_INVALID: &[(&str, &str)] = &[
    (
        "cr20-empty-default",
        "documento vacío: importes obligatorios vacíos",
    ),
    (
        "cr21-empty-default",
        "documento vacío: faltan elementos obligatorios",
    ),
    (
        "db20-empty-default",
        "documento vacío: importes obligatorios vacíos",
    ),
    (
        "db21-empty-default",
        "documento vacío: faltan elementos obligatorios",
    ),
    ("note-default-version", "nota sin datos"),
    ("desp-empty-default", "guía vacía"),
    ("desp-empty-cpeforge", "guía vacía"),
    ("desp2022-empty-default", "guía vacía"),
    ("desp2022-empty-cpeforge", "guía vacía"),
    ("inv-empty-default", "factura vacía"),
    ("inv-empty-cpeforge", "factura vacía"),
    ("inv21-empty-default", "factura vacía"),
    ("inv21-empty-cpeforge", "factura vacía"),
    (
        "inv21-empty-lists-default",
        "factura sin líneas (InvoiceLine es obligatorio)",
    ),
    ("summary-empty-details-default", "resumen sin líneas"),
    ("voided-empty-details-default", "baja sin líneas"),
    ("perception-empty-default", "percepción vacía"),
    ("perception-empty-cpeforge", "percepción vacía"),
    ("retention-empty-default", "retención vacía"),
    ("retention-empty-cpeforge", "retención vacía"),
    (
        "inv20-special-cpeforge",
        "sin autoescape un `&` produce XML mal formado",
    ),
    (
        "inv21-special-cpeforge",
        "sin autoescape un `&` produce XML mal formado",
    ),
    (
        "inv21-special-tests",
        "sin autoescape un `&` produce XML mal formado",
    ),
    (
        "cr21-special-cpeforge",
        "sin autoescape un `<` produce XML mal formado",
    ),
    (
        "inv20-amounts-default",
        "La referencia imprime cantidades pequeñas como 1.0E-6, no admitido por xsd:decimal",
    ),
    (
        "inv21-amounts-default",
        "La referencia imprime cantidades pequeñas como 1.0E-6, no admitido por xsd:decimal",
    ),
    (
        "inv21-charges-default",
        "cargo global sin factor: MultiplierFactorNumeric vacío",
    ),
    (
        "inv21-charges-tests",
        "cargo global sin factor: MultiplierFactorNumeric vacío",
    ),
    (
        "inv21-precision-default",
        "importe con más dígitos de los admitidos por el esquema",
    ),
    (
        "retention-full-default",
        "varios cac:Payment; el XSD de retención admite uno",
    ),
    (
        "retention-full-cpeforge",
        "varios cac:Payment; el XSD de retención admite uno",
    ),
    (
        "retention-full-tests",
        "varios cac:Payment; el XSD de retención admite uno",
    ),
    (
        "retention-no-provider-default",
        "varios cac:Payment; el XSD de retención admite uno",
    ),
    (
        "retention-no-provider-cpeforge",
        "varios cac:Payment; el XSD de retención admite uno",
    ),
];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn xsd_dir() -> TestResult<PathBuf> {
    let dir = std::env::var_os("CPEFORGE_XSD_DIR").map_or_else(
        || workspace().join("target/reference/ubl-validator-v2.3.0/src/xsd"),
        PathBuf::from,
    );
    if !dir.join("2.1/maindoc/UBL-Invoice-2.1.xsd").is_file() {
        return Err(harness_error(format!(
            "faltan los XSD en {}; ejecutar scripts/prepare_ubl_validator_reference.sh",
            dir.display()
        )));
    }
    Ok(dir)
}

/// Raíz y versión UBL (`2.0` si no hay `cbc:UBLVersionID`, como el trait de la referencia).
fn root_and_version(xml: &str) -> Option<(String, String)> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();
    let version = root
        .descendants()
        .find(|n| n.tag_name().name() == "UBLVersionID")
        .and_then(|n| n.text())
        .unwrap_or("2.0")
        .to_owned();
    Some((root.tag_name().name().to_owned(), version))
}

/// `true` si `xmllint` valida la copia adaptada del XML.
fn validate(xml: &str, name: &str, xsd: &Path) -> TestResult<bool> {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("xsd");
    fs::create_dir_all(&tmp)?;
    let Some((root, version)) = root_and_version(xml) else {
        return Ok(false); // mal formado
    };
    let copy = if version == "2.1" {
        xml.replacen(
            "<ext:ExtensionContent/>",
            "<ext:ExtensionContent><cbc:AccountID>1</cbc:AccountID></ext:ExtensionContent>",
            1,
        )
    } else {
        xml.to_owned()
    };
    let path = tmp.join(format!("{name}.xml"));
    fs::write(&path, copy)?;
    let schema = xsd
        .join(&version)
        .join("maindoc")
        .join(format!("UBL-{root}-{version}.xsd"));
    if !schema.is_file() {
        return Err(harness_error(format!("no hay XSD para {root} {version}")));
    }
    let output = Command::new("xmllint")
        .arg("--noout")
        .arg("--nonet")
        .arg("--schema")
        .arg(&schema)
        .arg(&path)
        .output()
        .map_err(|err| harness_error(format!("no se pudo ejecutar xmllint: {err}")))?;
    Ok(output.status.success())
}

fn render(case: &Value, now: i64) -> TestResult<Option<String>> {
    let mut options = RenderOptions {
        now: Some(Timestamp::from_second(now)?),
        ..RenderOptions::default()
    };
    if case["options"]["autoescape"] == Value::Bool(false) {
        options.autoescape = Autoescape::Disabled;
    }
    options.strict_variables = case["options"]["strict_variables"] == Value::Bool(true);
    let value = to_serde(&case["input"])?.map_err(|err| harness_error(format!("{err}")))?;
    macro_rules! go {
        ($ty:ty, $b:expr, $m:ident) => {
            Ok($b
                .$m(&deserialize::<$ty>(value).map_err(|e| harness_error(format!("{e}")))?)
                .ok())
        };
    }
    match (
        case["builder"].as_str().unwrap_or_default(),
        case["input"]["$class"].as_str().unwrap_or_default(),
    ) {
        ("InvoiceBuilder", _) => go!(Invoice, InvoiceBuilder::new(options), render),
        ("NoteBuilder", _) => go!(Note, NoteBuilder::new(options), render),
        ("DespatchBuilder", _) => go!(Despatch, DespatchBuilder::new(options), render),
        ("SummaryBuilder", _) => go!(Summary, SummaryBuilder::new(options), render),
        ("VoidedBuilder", "Reversion") => {
            go!(Reversion, VoidedBuilder::new(options), render_reversion)
        }
        ("VoidedBuilder", _) => go!(Voided, VoidedBuilder::new(options), render),
        ("PerceptionBuilder", _) => go!(Perception, PerceptionBuilder::new(options), render),
        ("RetentionBuilder", _) => go!(Retention, RetentionBuilder::new(options), render),
        (other, _) => Err(harness_error(format!("builder desconocido {other}"))),
    }
}

#[test]
fn xml_validates_against_xsd() -> TestResult {
    let xsd = xsd_dir()?;
    let reference = workspace().join("tests/reference/xml");
    let mut problems = Vec::new();
    let (mut valid, mut invalid) = (0, 0);
    let mut seen_invalid = Vec::new();
    let mut files: Vec<PathBuf> = fs::read_dir(reference.join("cases"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    files.sort();
    for file in files {
        let family = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        let cases: Value = serde_json::from_str(&fs::read_to_string(&file)?)?;
        let expected: Value = serde_json::from_str(&fs::read_to_string(
            reference.join("expected").join(format!("{family}.json")),
        )?)?;
        for case in cases["cases"].as_array().into_iter().flatten() {
            let id = case["id"].as_str().unwrap_or_default();
            let expected_case = &expected["results"][id];
            if expected_case.get("sha256").is_none() {
                continue; // caso de error: no hay XML que validar
            }
            let reference_xml = fs::read_to_string(
                reference
                    .join("expected")
                    .join(&family)
                    .join(format!("{id}.xml")),
            )?;
            let rust_xml = render(case, expected_case["now"].as_i64().unwrap_or_default())?
                .ok_or_else(|| harness_error(format!("{id}: Rust no generó XML")))?;
            let both = EXPECTED_INVALID.iter().find(|(case_id, _)| *case_id == id);
            let reference_only = REFERENCE_ONLY_INVALID
                .iter()
                .find(|(case_id, _)| *case_id == id);
            for (origin, xml) in [("reference", &reference_xml), ("rust", &rust_xml)] {
                let expectation = both.or(if origin == "reference" {
                    reference_only
                } else {
                    None
                });
                let ok = validate(xml, &format!("{id}-{origin}"), &xsd)?;
                match (ok, expectation) {
                    (true, None) => valid += 1,
                    (false, Some(_)) => invalid += 1,
                    (false, None) => {
                        problems.push(format!("{id} [{origin}]: no valida contra el XSD"))
                    }
                    (true, Some((_, reason))) => problems.push(format!(
                        "{id} [{origin}]: valida, pero estaba clasificado inválido ({reason})"
                    )),
                }
            }
            if both.is_some() || reference_only.is_some() {
                seen_invalid.push(id.to_owned());
            }
        }
    }
    for (id, _) in EXPECTED_INVALID.iter().chain(REFERENCE_ONLY_INVALID) {
        if !seen_invalid.iter().any(|seen| seen == id) {
            problems.push(format!(
                "clasificación obsoleta: {id} no existe o no generó XML"
            ));
        }
    }
    println!("XSD: {valid} validaciones correctas, {invalid} inválidas esperadas");
    assert!(valid > 200, "cobertura XSD insuficiente");
    if problems.is_empty() {
        Ok(())
    } else {
        Err(harness_error(problems.join("\n")))
    }
}

/// Frontera con `xmldsig`: el XML sin firma deja libre el contenedor de la
/// firma y referencia `#CPEFORGE-SIGN`, y los fixtures conservan bytes intactos.
#[test]
fn unsigned_xml_reserves_signature_slot() -> TestResult {
    let reference = workspace().join("tests/reference/xml/expected");
    let mut checked = 0;
    for family in fs::read_dir(&reference)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
    {
        for file in fs::read_dir(family.path())?.filter_map(Result::ok) {
            let xml = fs::read_to_string(file.path())?;
            if roxmltree::Document::parse(&xml).is_err() {
                continue;
            }
            checked += 1;
            assert!(
                xml.starts_with(r#"<?xml version="1.0" encoding="utf-8"?>"#),
                "{}",
                file.path().display()
            );
            assert_eq!(
                xml.matches("<ext:ExtensionContent/>").count(),
                1,
                "un único contenedor vacío para la firma: {}",
                file.path().display()
            );
            assert!(
                xml.contains("<cbc:URI>#CPEFORGE-SIGN</cbc:URI>"),
                "{}",
                file.path().display()
            );
            assert!(
                !xml.contains('\n'),
                "spaceless deja una sola línea: {}",
                file.path().display()
            );
        }
    }
    assert!(checked >= 140, "solo {checked} XML bien formados");
    Ok(())
}
