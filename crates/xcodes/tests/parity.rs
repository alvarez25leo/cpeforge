//! Comparación exhaustiva contra la salida de la referencia fijado.

use cpeforge_core::ports::ErrorCodeProvider;
use cpeforge_validator::Validator;
use cpeforge_xcodes::{ENTRY_COUNT, XmlErrorCodeProvider};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

const EXPECTED: &str = include_str!("../../../tests/reference/xcodes/expected.json");
const DIFFERENCES: &str = include_str!("../../../tests/reference/xcodes/differences.json");
const EXTRA_IDS: [&str; 14] = [
    "null",
    "empty",
    "space",
    "leading-zero",
    "extra-zero",
    "canonical",
    "unknown",
    "alpha",
    "unicode",
    "newline",
    "long",
    "xpath-no-match",
    "xpath-many",
    "xpath-injection-0100",
];

#[test]
fn reference_catalog_and_all_queries() -> Result<(), Box<dyn Error>> {
    let expected: Value = serde_json::from_str(EXPECTED)?;
    let registry: Value = serde_json::from_str(DIFFERENCES)?;
    let differences = registry["differences"].as_array().ok_or("sin registro")?;
    let mut registered = BTreeSet::new();
    for entry in differences {
        perfil_seguro::validate(entry)?;
        registered.insert(entry["id"].as_str().ok_or("diferencia sin ID")?);
    }
    assert_eq!(
        registered.len(),
        differences.len(),
        "ID de diferencia duplicado"
    );
    let reference_rows = expected["all"]
        .as_array()
        .ok_or("sin getAll de la referencia")?;
    let reference_queries = expected["values"]
        .as_array()
        .ok_or("sin getValue de la referencia")?;
    assert_eq!(reference_rows.len(), ENTRY_COUNT);
    assert_eq!(reference_queries.len(), ENTRY_COUNT + 14);
    let extras = reference_queries[ENTRY_COUNT..]
        .iter()
        .map(|query| query["id"].as_str().ok_or("extra sin ID"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    assert_eq!(extras, EXTRA_IDS.into_iter().collect());
    for (row, query) in reference_rows.iter().zip(reference_queries) {
        let code = row["xml_code"].as_str().ok_or("fila sin código")?;
        assert_eq!(query["id"].as_str(), Some(format!("all/{code}").as_str()));
        assert_eq!(query["code"].as_str(), Some(code));
    }

    let provider = XmlErrorCodeProvider::new()?;
    let all = provider.all()?.ok_or("all() nulo")?;
    assert_eq!(all.len(), ENTRY_COUNT);
    assert_eq!(provider.iter().len(), ENTRY_COUNT);
    for (index, ((code, message), row)) in all.iter().zip(reference_rows).enumerate() {
        assert_eq!(
            row["xml_code"].as_str(),
            Some(code.as_str()),
            "código en {index}"
        );
        assert_eq!(
            row["message"].as_str(),
            Some(message.as_str()),
            "mensaje en {index}"
        );
        let canonical = code
            .parse::<i64>()
            .is_ok_and(|value| value.to_string() == *code);
        if canonical {
            assert_eq!(row["reference_key_type"], "integer", "tipo en {index}");
            assert_eq!(
                row["reference_key"].as_i64(),
                code.parse::<i64>().ok(),
                "clave de la referencia en {index}"
            );
        } else {
            assert_eq!(row["reference_key_type"], "string", "tipo en {index}");
            assert_eq!(
                row["reference_key"].as_str(),
                Some(code.as_str()),
                "clave de la referencia en {index}"
            );
        }
    }

    let mut observed = BTreeSet::new();
    let mut seen_queries = BTreeSet::new();
    for query in reference_queries {
        let id = query["id"].as_str().ok_or("consulta sin ID")?;
        assert!(seen_queries.insert(id), "ID de consulta duplicado: {id}");
        let code = query["code"].as_str();
        let reference = query["value"]
            .as_str()
            .ok_or("valor de la referencia no textual")?;
        let rust = provider.value(code)?.ok_or("valor Rust nulo")?;
        if rust != reference {
            assert!(registered.contains(id), "diferencia no registrada: {id}");
            assert_eq!(id, "xpath-injection-0100");
            assert_eq!(rust, "");
            assert_eq!(reference, provider.get("0100"));
            observed.insert(id);
        } else {
            assert!(!registered.contains(id), "registro obsoleto: {id}");
        }
    }
    assert_eq!(observed, registered, "diferencias faltantes u obsoletas");
    Ok(())
}

#[test]
fn validator_consumes_catalog_without_changing_local_g001() -> Result<(), Box<dyn Error>> {
    let provider = XmlErrorCodeProvider::new()?;
    let mut validator = Validator::new();
    validator.set_provider(provider);
    let parameters = BTreeMap::new();
    assert_eq!(
        validator.translate("0100", &parameters, None, None)?,
        provider.get("0100")
    );
    assert_eq!(
        validator.translate("9999", &parameters, None, None)?,
        "9999"
    );
    assert_eq!(
        validator.translate("G001", &parameters, None, None)?,
        "El texto no cumple con el formato establecido"
    );
    Ok(())
}
