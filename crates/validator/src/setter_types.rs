//! Comprobación de tipos de los setters de la referencia al hidratar un modelo desde JSON.
//!
//! En la referencia, con `declare(strict_types=1)`, `$modelo->setCampo($valor)` lanza
//! `TypeError` si el valor no es del tipo declarado. [`check_setter_types`]
//! reproduce esa comprobación y su mensaje sobre la misma representación JSON
//! que hidrata el oráculo (`{"type": …, "fields": …}` para objetos,
//! `{"$date": …}` para fechas, listas para arreglos). Las firmas salen de
//! `setters.json`, generado por reflexión de la referencia
//! (`scripts/build_validator_setters.py`).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::LazyLock;

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Table {
    models: BTreeMap<String, Model>,
}

#[derive(Deserialize)]
struct Model {
    class: String,
    is_a: Vec<String>,
    /// Setter → (clase que lo declara, parámetro, tipo declarado).
    setters: BTreeMap<String, (String, String, Option<String>)>,
}

static TABLE: LazyLock<Option<Table>> =
    LazyLock::new(|| serde_json::from_str(include_str!("setters.json")).ok());

/// `TypeError` de un setter con tipos estrictos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetterTypeError {
    message: String,
}

impl SetterTypeError {
    /// Mensaje de la referencia sin la ubicación (`, called in … on line N`), por
    /// ejemplo `CPEForge\Model\Company\Address::setCodLocal(): Argument #1
    /// ($codLocal) must be of type ?string, false given`.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SetterTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SetterTypeError {}

/// Valor de la referencia que resulta de hidratar un valor JSON.
enum Hydrated<'a> {
    Null,
    Bool(bool),
    Int,
    Float,
    String,
    Array,
    DateTime,
    Object(&'a Model),
}

impl Hydrated<'_> {
    /// Nombre que la referencia usa en `…, X given`.
    fn given(&self) -> &str {
        match self {
            Self::Null => "null",
            Self::Bool(true) => "true",
            Self::Bool(false) => "false",
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Array => "array",
            Self::DateTime => "DateTime",
            Self::Object(model) => &model.class,
        }
    }

    /// ¿Admite la referencia (modo estricto) este valor para el tipo declarado?
    fn accepted_by(&self, declared: Option<&str>) -> bool {
        let Some(declared) = declared else {
            return true;
        };
        let (nullable, base) = match declared.strip_prefix('?') {
            Some(base) => (true, base),
            None => (false, declared),
        };
        match self {
            Self::Null => nullable || base == "mixed",
            _ if base == "mixed" => true,
            Self::Bool(_) => base == "bool",
            Self::Int => matches!(base, "int" | "float"),
            Self::Float => base == "float",
            Self::String => base == "string",
            Self::Array => matches!(base, "array" | "iterable"),
            Self::DateTime => matches!(base, "DateTime" | "DateTimeInterface" | "object"),
            Self::Object(model) => base == "object" || model.is_a.iter().any(|c| c == base),
        }
    }
}

fn model<'a>(table: &'a Table, kind: &str) -> Option<&'a Model> {
    table.models.get(kind)
}

/// Hidrata `value` como lo hace el oráculo de referencia, comprobando antes los
/// objetos anidados (la referencia los construye antes de llamar al setter externo).
fn hydrate<'a>(table: &'a Table, value: &Value) -> Result<Hydrated<'a>, SetterTypeError> {
    Ok(match value {
        Value::Null => Hydrated::Null,
        Value::Bool(b) => Hydrated::Bool(*b),
        Value::Number(n) if n.is_f64() => Hydrated::Float,
        Value::Number(_) => Hydrated::Int,
        Value::String(_) => Hydrated::String,
        Value::Array(items) => {
            for item in items {
                if item.get("type").is_some_and(Value::is_string) {
                    hydrate(table, item)?;
                }
            }
            Hydrated::Array
        }
        Value::Object(map) => match map.get("type").and_then(Value::as_str) {
            Some(kind) => {
                check_fields(table, kind, map.get("fields").unwrap_or(&Value::Null))?;
                match model(table, kind) {
                    Some(model) => Hydrated::Object(model),
                    None => Hydrated::Array,
                }
            }
            None if map.contains_key("$date") => Hydrated::DateTime,
            None => Hydrated::Array,
        },
    })
}

fn check_fields(table: &Table, kind: &str, fields: &Value) -> Result<(), SetterTypeError> {
    let (Some(model), Some(fields)) = (model(table, kind), fields.as_object()) else {
        return Ok(());
    };
    for (field, value) in fields {
        let mut chars = field.chars();
        let setter = match chars.next() {
            Some(first) => format!("set{}{}", first.to_uppercase(), chars.as_str()),
            None => continue,
        };
        let Some((owner, param, declared)) = model.setters.get(&setter) else {
            continue;
        };
        let hydrated = hydrate(table, value)?;
        if !hydrated.accepted_by(declared.as_deref()) {
            return Err(SetterTypeError {
                message: format!(
                    "{owner}::{setter}(): Argument #1 (${param}) must be of type {}, {} given",
                    declared.as_deref().unwrap_or("mixed"),
                    hydrated.given()
                ),
            });
        }
    }
    Ok(())
}

/// Comprueba, como los setters de la referencia con `strict_types`, los campos JSON de un
/// modelo `kind` (nombre corto: `Invoice`, `Address`, …) antes de hidratarlo.
///
/// Los campos se recorren en el orden del `Map` de `serde_json` (alfabético):
/// con varios campos inválidos la referencia informa el primero en el orden del JSON,
/// que puede ser otro. Un modelo o setter desconocido no se comprueba aquí.
///
/// # Errors
///
/// [`SetterTypeError`] con el mensaje que daría la referencia para el primer valor que
/// el setter no admite.
pub fn check_setter_types(kind: &str, fields: &Value) -> Result<(), SetterTypeError> {
    match TABLE.as_ref() {
        Some(table) => check_fields(table, kind, fields),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(kind: &str, fields: Value) -> Option<String> {
        check_setter_types(kind, &fields)
            .err()
            .map(|e| e.message().to_owned())
    }

    #[test]
    fn table_loads() {
        assert!(TABLE.is_some());
    }

    #[test]
    fn strict_scalars() {
        assert_eq!(
            message("Address", json!({"codLocal": false})).as_deref(),
            Some(
                "CPEForge\\Model\\Company\\Address::setCodLocal(): Argument #1 ($codLocal) must be of type ?string, false given"
            )
        );
        assert!(message("Address", json!({"codLocal": "0000", "ubigueo": null})).is_none());
        // int → float está permitido incluso en modo estricto.
        assert!(message("Payment", json!({"importe": 10})).is_none());
        assert!(
            message("Payment", json!({"importe": "10"}))
                .is_some_and(|m| m.ends_with("must be of type ?float, string given"))
        );
    }

    #[test]
    fn inherited_setter_names_declaring_class() {
        assert!(
            message("FormaPagoContado", json!({"tipo": false}))
                .is_some_and(|m| m.starts_with("CPEForge\\Model\\Sale\\PaymentTerms::setTipo()"))
        );
    }

    #[test]
    fn nested_objects_dates_and_lists() {
        assert!(message("Invoice", json!({"fechaEmision": {"$date": "2024-01-01"}})).is_none());
        assert!(
            message("Invoice", json!({"fechaEmision": "2024-01-01"}))
                .is_some_and(|m| m.ends_with("must be of type ?DateTimeInterface, string given"))
        );
        assert!(
            message(
                "Invoice",
                json!({"company": {"type": "Company", "fields": {"address": {"type": "Address", "fields": {"codLocal": 0}}}}})
            )
            .is_some_and(|m| m.starts_with("CPEForge\\Model\\Company\\Address::setCodLocal()")
                && m.ends_with("int given"))
        );
        assert!(
            message("Invoice", json!({"company": {"type": "Address", "fields": {}}}))
                .is_some_and(|m| m.ends_with(
                    "must be of type ?CPEForge\\Model\\Company\\Company, CPEForge\\Model\\Company\\Address given"
                ))
        );
    }
}
