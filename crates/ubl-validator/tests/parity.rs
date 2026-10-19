//! Comparación estricta con la referencia real fijado; la evidencia clasifica las
//! diferencias de libxml entre la referencia y la biblioteca Rust del sistema.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "arnés de prueba"
)]

use std::fs;
use std::path::{Path, PathBuf};

use cpeforge_ubl_validator::UblValidator;
use serde_json::Value;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Expande el marcador `{workspace}/` que los oráculos de la referencia escriben en lugar
/// del prefijo absoluto del checkout. Solo se aplica a los campos donde la referencia
/// incrusta la ruta del XSD (`error` y `errors[].message`); el resto del texto
/// se compara exacto.
fn expand_workspace(reference: &Value, root: &Path) -> String {
    let prefix = format!("{}/", root.display());
    reference.as_str().unwrap().replace("{workspace}/", &prefix)
}

#[test]
fn reference_parity_all_schemas() {
    let root = workspace().canonicalize().unwrap();
    let corpus = root.join("tests/reference/ubl-validator");
    let cases: Value =
        serde_json::from_slice(&fs::read(corpus.join("cases.json")).unwrap()).unwrap();
    let expected_path = std::env::var_os("UBL_REFERENCE_EXPECTED")
        .map(PathBuf::from)
        .unwrap_or_else(|| corpus.join("reference-expected.json"));
    let expected: Value = serde_json::from_slice(&fs::read(expected_path).unwrap()).unwrap();
    let entries = cases["cases"].as_array().unwrap();
    let outcomes = expected["results"].as_array().unwrap();
    assert_eq!(
        entries.len(),
        outcomes.len(),
        "faltan casos de referencia o Rust"
    );
    assert!(entries.len() >= 34);
    let mut roots = std::collections::HashSet::new();
    let mut differences = Vec::new();
    for (case, reference) in entries.iter().zip(outcomes) {
        let id = case["id"].as_str().unwrap();
        assert_eq!(reference["id"], id);
        let xml = fs::read(corpus.join("xml").join(case["file"].as_str().unwrap())).unwrap();
        let mut validator =
            UblValidator::new(root.join("target/reference/ubl-validator-v2.3.0/src/Ubl/../xsd"))
                .unwrap();
        assert_eq!(validator.get_error(), None);
        let result = match validator.validate(&xml) {
            Ok(result) => result,
            Err(error)
                if reference["exception"] == "ValueError"
                    && error.to_string() == reference["message"].as_str().unwrap() =>
            {
                continue;
            }
            Err(error) => {
                differences.push(format!(
                    "{id}: excepción Rust={error:?} la referencia={:?}",
                    reference["exception"]
                ));
                continue;
            }
        };
        assert_eq!(
            result.valid,
            case["expected_valid"].as_bool().unwrap(),
            "{id}"
        );
        if case["expected_valid"] == true {
            roots.insert(result.schema.clone().unwrap());
        }
        if result.valid != reference["valid"].as_bool().unwrap() {
            differences.push(format!("{id}: booleano"));
        }
        if result.version != reference["version"].as_str().unwrap() {
            differences.push(format!("{id}: versión"));
        }
        if result.schema.as_deref() != reference["schema"].as_str() {
            differences.push(format!("{id}: ruta"));
        }
        let reference_error = expand_workspace(&reference["error"], &root);
        if result.error != reference_error {
            differences.push(format!(
                "{id}: mensaje Rust={:?} la referencia={reference_error:?}",
                result.error
            ));
        }
        let actual: Vec<_> = result
            .errors
            .iter()
            .map(|e| {
                serde_json::json!({
                    "level":e.level,"code":e.code,"column":e.column,"line":e.line,
                    "message":e.message,"file":e.file,
                })
            })
            .collect();
        let reference_errors: Vec<_> = reference["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let mut e = e.clone();
                e["message"] = Value::String(expand_workspace(&e["message"], &root));
                e
            })
            .collect();
        if actual != reference_errors {
            differences.push(format!("{id}: campos XmlError"));
        }
    }
    assert_eq!(roots.len(), 16, "faltan esquemas raíz válidos");
    assert!(
        differences.is_empty(),
        "diferencias referencia↔Rust:\n{}",
        differences.join("\n")
    );
}

#[test]
fn reference_path_resolver_state() {
    let root = workspace().canonicalize().unwrap();
    let corpus = root.join("tests/reference/ubl-validator");
    let expected_path = std::env::var_os("UBL_REFERENCE_EXPECTED")
        .map(PathBuf::from)
        .unwrap_or_else(|| corpus.join("reference-expected.json"));
    let expected: Value = serde_json::from_slice(&fs::read(expected_path).unwrap()).unwrap();
    let state = &expected["state"];
    assert!(state["before"].is_null());
    let mut validator =
        UblValidator::new(root.join("target/reference/ubl-validator-v2.3.0/src/Ubl/../xsd"))
            .unwrap();
    assert_eq!(validator.get_error(), None);
    for (index, step) in state["steps"].as_array().unwrap().iter().enumerate() {
        if index == 3 {
            validator.set_version(Some("2.1")).unwrap();
        }
        let xml = fs::read(corpus.join("xml").join(step["file"].as_str().unwrap())).unwrap();
        let actual = validator.validate(&xml).unwrap();
        assert_eq!(actual.valid, step["valid"].as_bool().unwrap());
        assert_eq!(actual.error, expand_workspace(&step["error"], &root));
        assert_eq!(actual.version, step["cached_version"].as_str().unwrap());
    }
}
