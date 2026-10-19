//! Comprueba que cada símbolo y método público del inventario de la referencia de
//! la referencia tiene un equivalente Rust registrado.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use cpeforge_core::time::PERU_TIME_ZONE;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn reference(file: &str) -> Result<Value, Box<dyn Error>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/reference/core")
        .join(file);
    Ok(serde_json::from_str(&fs::read_to_string(&path)?)?)
}

fn object(value: &Value) -> Result<&serde_json::Map<String, Value>, Box<dyn Error>> {
    value
        .as_object()
        .ok_or_else(|| "se esperaba un objeto JSON".into())
}

#[test]
fn inventory_is_from_pinned_reference() -> TestResult {
    let inventory = reference("inventory.json")?;
    let sale = reference("expected/sale.json")?;
    assert_eq!(
        inventory["reference"]["baseline_commit"], sale["reference"]["commit"],
        "inventario y fixtures deben venir de la misma referencia"
    );
    assert_eq!(
        object(&inventory["classes"])?.len(),
        61,
        "v5.3.0 tiene 61 archivos de la referencia en packages/core/src/Core"
    );
    Ok(())
}

#[test]
fn every_reference_symbol_and_method_is_mapped() -> TestResult {
    let inventory = reference("inventory.json")?;
    let map = reference("api-map.json")?;
    let classes = object(&inventory["classes"])?;
    let mapped = object(&map["classes"])?;

    let reference: BTreeSet<&String> = classes.keys().collect();
    let rust: BTreeSet<&String> = mapped.keys().collect();
    assert_eq!(reference, rust, "símbolos sin mapear o mapeos obsoletos");

    let mut problems = Vec::new();
    for (name, class) in classes {
        let methods = object(&mapped[name]["methods"])?;
        let mut expected = BTreeSet::new();
        let mut all_methods = BTreeSet::new();
        for method in class["methods"].as_array().into_iter().flatten() {
            let method_name = method["name"].as_str().unwrap_or_default();
            all_methods.insert(method_name.to_owned());
            // Interfaces (sin `kind`) y métodos con lógica propia exigen mapeo explícito;
            // getters y setters triviales se verifican por regla en tests/parity.rs.
            let kind = method["kind"].as_str();
            if kind.is_none() || kind == Some("custom") {
                expected.insert(method_name.to_owned());
            }
        }
        let declared: BTreeSet<String> = methods.keys().cloned().collect();
        for missing in expected.difference(&declared) {
            problems.push(format!("{name}::{missing} sin equivalente Rust"));
        }
        for stale in declared.difference(&all_methods) {
            problems.push(format!(
                "{name}::{stale} mapeado pero ausente en el inventario"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    Ok(())
}

#[test]
fn every_property_has_trivial_getter_and_setter() -> TestResult {
    let inventory = reference("inventory.json")?;
    for (name, class) in object(&inventory["classes"])? {
        let mut getters = BTreeSet::new();
        let mut setters = BTreeSet::new();
        for method in class["methods"].as_array().into_iter().flatten() {
            match method["kind"].as_str() {
                Some("getter") => {
                    getters.insert(method["property"].as_str().unwrap_or_default().to_owned())
                }
                Some("setter") => {
                    setters.insert(method["property"].as_str().unwrap_or_default().to_owned())
                }
                _ => false,
            };
        }
        assert_eq!(getters, setters, "{name}: getters y setters no coinciden");
        if name.ends_with("BasicToken") {
            // Propiedades tipadas inicializadas por el constructor.
            continue;
        }
        for property in class["properties"].as_array().into_iter().flatten() {
            let prop = property["name"].as_str().unwrap_or_default();
            assert!(getters.contains(prop), "{name}::${prop} sin getter trivial");
        }
    }
    Ok(())
}

#[test]
fn peru_time_zone_matches_reference_constant() -> TestResult {
    let inventory = reference("inventory.json")?;
    assert_eq!(
        inventory["classes"]["CPEForge\\Model\\TimeZonePe"]["constants"]["DEFAULT"],
        PERU_TIME_ZONE
    );
    Ok(())
}
