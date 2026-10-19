//! Ejecutor Rust de los mismos casos JSON del oráculo de referencia.

use cpeforge_core::model::{
    client, company, despatch, perception, retention, sale, summary, voided,
};
use cpeforge_validator::{Validator, ValidatorError, Violation, check_setter_types};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::fs;

#[derive(Deserialize)]
struct Case {
    id: String,
    version: Option<String>,
    model: ModelSpec,
}

#[derive(Deserialize)]
struct ModelSpec {
    #[serde(rename = "type")]
    kind: String,
    fields: Value,
    #[serde(default)]
    raw: bool,
}

#[derive(Serialize)]
struct ResultRow {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    /// Lista de violaciones o, si son más de 1000, `{"$summary": …}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    violations: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorRow>,
}

#[derive(Serialize)]
struct ErrorRow {
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

/// Misma expansión que `validator_expand` del oráculo de referencia:
/// `{"$repeat": {"text"|"value", "count"}}` y `{"$nest": {"depth", "leaf"}}`.
fn expand(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(expand).collect()),
        Value::Object(mut object) if object.len() == 1 => {
            if let Some(Value::Object(spec)) = object.remove("$repeat") {
                let count = spec
                    .get("count")
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    .unwrap_or(0);
                return match spec.get("text").and_then(Value::as_str) {
                    Some(text) => Value::String(text.repeat(count)),
                    None => Value::Array(vec![
                        expand(
                            spec.get("value").cloned().unwrap_or(Value::Null)
                        );
                        count
                    ]),
                };
            }
            if let Some(Value::Object(spec)) = object.remove("$nest") {
                let depth = spec.get("depth").and_then(Value::as_u64).unwrap_or(0);
                let mut nested = expand(spec.get("leaf").cloned().unwrap_or(Value::Null));
                for _ in 0..depth {
                    nested = Value::Array(vec![nested]);
                }
                return nested;
            }
            Value::Object(object.into_iter().map(|(k, v)| (k, expand(v))).collect())
        }
        Value::Object(object) => {
            Value::Object(object.into_iter().map(|(k, v)| (k, expand(v))).collect())
        }
        other => other,
    }
}

/// Fallo de un caso: `TypeError` (setter o deserialización) o error del
/// validador (límites, reglas).
enum Failure {
    Type(String),
    Validator(ValidatorError),
}

fn normalize_input(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(normalize_input).collect()),
        Value::Object(mut object) => {
            if let Some(date) = object.remove("$date") {
                return date;
            }
            if let Some(Value::String(kind)) = object.remove("type") {
                if let Some(Value::Object(mut fields)) = object.remove("fields") {
                    if kind == "FormaPagoContado" {
                        fields
                            .entry("tipo")
                            .or_insert(Value::String("Contado".to_owned()));
                    }
                    if kind == "FormaPagoCredito" {
                        fields
                            .entry("tipo")
                            .or_insert(Value::String("Credito".to_owned()));
                    }
                    return normalize_input(Value::Object(fields));
                }
            }
            Value::Object(
                object
                    .into_iter()
                    .map(|(key, value)| (key, normalize_input(value)))
                    .collect(),
            )
        }
        other => other,
    }
}

fn evaluate<T: DeserializeOwned + Serialize>(
    validator: &Validator,
    kind: &str,
    fields: &Value,
) -> Result<Vec<Violation>, Failure> {
    let model: T =
        serde_json::from_value(fields.clone()).map_err(|error| Failure::Type(error.to_string()))?;
    validator
        .validate_named(kind, &model)
        .map_err(Failure::Validator)
}

fn run_case(case: Case) -> ResultRow {
    let mut validator = Validator::new();
    validator.set_version(case.version.as_deref());
    let kind = case.model.kind;
    let fields = expand(case.model.fields);
    // Como `validator_hydrate` de la referencia: los setters con tipos estrictos se
    // ejecutan antes de validar; el oráculo de referencia sustituye la ubicación del
    // TypeError por ", called by oracle".
    if !case.model.raw {
        if let Err(error) = check_setter_types(&kind, &fields) {
            return ResultRow {
                id: case.id,
                kind,
                violations: None,
                error: Some(ErrorRow {
                    kind: "TypeError".to_owned(),
                    message: format!("{error}, called by oracle"),
                }),
            };
        }
    }
    let fields = normalize_input(fields);
    let fields = if matches!(kind.as_str(), "FormaPagoContado" | "FormaPagoCredito") {
        let mut object = match fields {
            Value::Object(object) => object,
            _ => Map::new(),
        };
        object.entry("tipo").or_insert(Value::String(
            if kind == "FormaPagoContado" {
                "Contado"
            } else {
                "Credito"
            }
            .to_owned(),
        ));
        Value::Object(object)
    } else {
        fields
    };
    macro_rules! check {
        ($type:ty) => {
            evaluate::<$type>(&validator, &kind, &fields)
        };
    }
    let result = if case.model.raw {
        validator
            .validate_raw(&kind, &fields)
            .map_err(Failure::Validator)
    } else {
        match kind.as_str() {
            "Address" => check!(company::Address),
            "Client" => check!(client::Client),
            "Company" => check!(company::Company),
            "Cuota" => check!(sale::Cuota),
            "DespatchDetail" => check!(despatch::DespatchDetail),
            "Despatch" => check!(despatch::Despatch),
            "Detraction" => check!(sale::Detraction),
            "Direction" => check!(despatch::Direction),
            "Document" => check!(sale::RelatedDocument),
            "EmbededDespatch" => check!(sale::EmbededDespatch),
            "Exchange" => check!(retention::Exchange),
            "FormaPagoContado" | "FormaPagoCredito" | "PaymentTerms" => check!(sale::PaymentTerms),
            "Invoice" => check!(sale::Invoice),
            "Legend" => check!(sale::Legend),
            "Note" => check!(sale::Note),
            "Payment" => check!(retention::Payment),
            "PerceptionDetail" => check!(perception::PerceptionDetail),
            "Perception" => check!(perception::Perception),
            "Prepayment" => check!(sale::Prepayment),
            "RetentionDetail" => check!(retention::RetentionDetail),
            "Retention" => check!(retention::Retention),
            "Reversion" => check!(voided::Reversion),
            "SaleDetail" => check!(sale::SaleDetail),
            "SalePerception" => check!(sale::SalePerception),
            "Shipment" => check!(despatch::Shipment),
            "SummaryDetail" => check!(summary::SummaryDetail),
            "Summary" => check!(summary::Summary),
            "SummaryPerception" => check!(summary::SummaryPerception),
            "Transportist" => check!(despatch::Transportist),
            "VoidedDetail" => check!(voided::VoidedDetail),
            "Voided" => check!(voided::Voided),
            "Charge" => check!(sale::Charge),
            "DetailAttribute" => check!(sale::DetailAttribute),
            _ => Err(Failure::Type(format!("tipo no soportado: {kind}"))),
        }
    };
    match result {
        Ok(violations) => ResultRow {
            id: case.id,
            kind,
            violations: Some(violations_value(&violations)),
            error: None,
        },
        Err(failure) => {
            let (error_kind, message) = match failure {
                Failure::Type(message) => ("TypeError", message),
                Failure::Validator(error @ ValidatorError::Limit(_)) => {
                    ("ValidatorError::Limit", error.to_string())
                }
                Failure::Validator(error) => ("TypeError", error.to_string()),
            };
            ResultRow {
                id: case.id,
                kind,
                violations: None,
                error: Some(ErrorRow {
                    kind: error_kind.to_owned(),
                    message,
                }),
            }
        }
    }
}

/// Igual que el oráculo de referencia: más de 1000 violaciones se resumen con su
/// cantidad, la primera y la última.
fn violations_value(violations: &[Violation]) -> Value {
    let list = serde_json::to_value(violations).unwrap_or(Value::Null);
    match list.as_array().filter(|items| items.len() > 1000) {
        Some(items) => serde_json::json!({"$summary": {
            "count": items.len(),
            "first": items.first(),
            "last": items.last(),
        }}),
        None => list,
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("se requiere cases.json")?;
    let cases: Vec<Case> = serde_json::from_str(&fs::read_to_string(path)?)?;
    let rows: Vec<_> = cases.into_iter().map(run_case).collect();
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("validator oracle: {error}");
        std::process::exit(2);
    }
}
