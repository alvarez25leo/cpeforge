//! Contratos independientes y casos de API del validador.

use cpeforge_core::model::{client::Client, company::Address, sale::Invoice};
use cpeforge_core::ports::ErrorCodeProvider;
use cpeforge_validator::{
    FieldRules, MessageTranslator, Metadata, Rule, RuleExtension, Validator, ValidatorError,
    Violation, loader_names,
};
use jiff::civil::Date;
use serde_json::{Value, json};
use std::collections::BTreeMap;

struct Provider;
impl ErrorCodeProvider for Provider {
    type Error = std::convert::Infallible;
    fn all(&self) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        Ok(None)
    }
    fn value(&self, code: Option<&str>) -> Result<Option<String>, Self::Error> {
        Ok(match code {
            Some("G001") => Some("Proveedor".to_owned()),
            Some("1003") => Some(String::new()),
            _ => None,
        })
    }
}

struct Listener;
impl RuleExtension for Listener {
    fn validate(&self, model_type: &str, _model: &Value) -> Vec<Violation> {
        if model_type == "Invoice" {
            vec![Violation::custom("fechaEmision", "Regla sintética")]
        } else {
            Vec::new()
        }
    }
}

struct PropertyListener;
impl RuleExtension for PropertyListener {
    fn extend_metadata(&self, model_type: &str, metadata: &mut Metadata) {
        if model_type == "Client" {
            metadata.fields.push(FieldRules {
                field: "email".to_owned(),
                rules: vec![Rule {
                    name: "NotBlank".to_owned(),
                    options: json!({"allowNull": false, "message": "Email obligatoria"}),
                }],
            });
        }
    }

    fn validate(&self, _model_type: &str, _model: &Value) -> Vec<Violation> {
        Vec::new()
    }
}

struct FixedDateListener {
    today: Date,
}
impl RuleExtension for FixedDateListener {
    fn validate(&self, model_type: &str, model: &Value) -> Vec<Violation> {
        if model_type != "Invoice" {
            return Vec::new();
        }
        let Some(day) = model
            .get("fechaEmision")
            .and_then(Value::as_str)
            .and_then(|text| text.get(..10))
        else {
            return Vec::new();
        };
        let Ok(issue) = Date::strptime("%Y-%m-%d", day) else {
            return Vec::new();
        };
        let Ok(delta) = self.today.since(issue) else {
            return Vec::new();
        };
        if delta.get_days().unsigned_abs() > 7 {
            vec![Violation::custom(
                "fechaEmision",
                "Fecha emision superó 7 dias",
            )]
        } else {
            Vec::new()
        }
    }
}

#[test]
fn metadata_y_cambio_de_version_coinciden_con_reference() -> Result<(), Box<dyn std::error::Error>>
{
    let mut validator = Validator::new();
    assert_eq!(loader_names("default")?.len(), 33);
    assert_eq!(loader_names("2.1")?.len(), 35);
    assert!(!validator.has_metadata_for("Charge")?);
    assert!(!validator.has_metadata_for("SinLoader")?);
    assert_eq!(validator.metadata_for("SinLoader")?.fields.len(), 0);
    let invoice = Invoice::default();
    let base: Vec<_> = validator
        .validate_model(&invoice)?
        .into_iter()
        .map(|v| (v.path, v.message))
        .collect();
    assert_eq!(base.len(), 11);
    assert_eq!(base[0].0, "tipoDoc");
    validator.set_version(Some("2.1"));
    let v21: Vec<_> = validator
        .validate_model(&invoice)?
        .into_iter()
        .map(|v| (v.path, v.message))
        .collect();
    assert_eq!(v21.len(), 12);
    assert_eq!(v21[0].0, "tipoOperacion");
    assert_eq!(validator.metadata_for("Invoice")?.loader, "v21");
    validator.set_version(Some("9.9"));
    assert_eq!(validator.metadata_for("Invoice")?.loader, "base");
    assert_eq!(validator.validate_model(&invoice)?.len(), base.len());
    validator.set_extension(Listener);
    let with_listener = validator.validate_model(&invoice)?;
    assert_eq!(with_listener[0].message, "Regla sintética");
    assert_eq!(with_listener[0].path, "fechaEmision");
    assert_eq!(validator.validate_model(&Client::default())?.len(), 3);
    Ok(())
}

#[test]
fn traductor_conserva_precedencia_y_claves_numericas() -> Result<(), Box<dyn std::error::Error>> {
    let mut translator = MessageTranslator::new(Provider);
    translator.add_resources([
        ("G001".to_owned(), "Recurso G001".to_owned()),
        ("1003".to_owned(), "Recurso 1003".to_owned()),
    ]);
    let params = BTreeMap::from([("{{ value }}".to_owned(), "ABC".to_owned())]);
    assert_eq!(translator.locale(), "en");
    assert_eq!(
        translator.trans("G001", &params, Some("validators"), Some("es_PE"))?,
        "Proveedor"
    );
    assert_eq!(translator.trans("1003", &params, None, None)?, "1003");
    assert_eq!(translator.trans("unknown", &params, None, None)?, "unknown");
    let mut validator = Validator::new();
    validator.set_provider(Provider);
    assert_eq!(
        validator.translate("G001", &params, None, None)?,
        "Proveedor"
    );
    assert_eq!(validator.translate("1003", &params, None, None)?, "1003");
    Ok(())
}

#[test]
fn bordes_independientes_y_ruta_anidada() -> Result<(), Box<dyn std::error::Error>> {
    let validator = Validator::new();
    let mut address = Address {
        ubigueo: Some("123456".to_owned()),
        ..Address::default()
    };
    assert!(validator.validate_model(&address)?.is_empty());
    address.ubigueo = Some("1234567".to_owned());
    let errors = validator.validate_model(&address)?;
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, "ubigueo");
    assert_eq!(
        errors[0]
            .parameters
            .get("{{ value_length }}")
            .map(String::as_str),
        Some("7")
    );
    let raw = json!({"address": {"ubigueo": "1234567", "codigoPais": "PE", "codLocal": "0000"}, "tipoDoc": "1", "numDoc": "12345678", "rznSocial": "Sintético"});
    let nested = validator.validate_raw("Client", &raw)?;
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].path, "address.ubigueo");
    let required = validator.validate_raw("DespatchDetail", &json!({}))?;
    assert_eq!(
        required.iter().map(|v| v.path.as_str()).collect::<Vec<_>>(),
        vec!["cantidad"]
    );
    let numeric_string = validator.validate_raw("SalePerception", &json!({"mto": "3.5"}))?;
    assert!(
        numeric_string
            .iter()
            .any(|v| v.path == "mto" && v.message == "This value should be of type float.")
    );
    Ok(())
}

#[test]
fn debug_redacta_valores_y_parametros() {
    // `Violation` no se construye fuera del crate: se deserializa.
    let violation: Violation = serde_json::from_value(json!({
        "path": "ruc",
        "message": "Error",
        "template": "Error",
        "code": null,
        "parameters": {"{{ value }}": "SECRETO"},
        "invalid": "SECRETO",
    }))
    .unwrap();
    let debug = format!("{violation:?}");
    assert!(!debug.contains("SECRETO"));
    assert!(debug.contains("[REDACTED]"));
    for error in [
        ValidatorError::Model("SECRETO".to_owned()),
        ValidatorError::Provider("SECRETO".to_owned()),
        ValidatorError::RuleTable("SECRETO".to_owned()),
        ValidatorError::UnsupportedRule("SECRETO".to_owned()),
    ] {
        assert!(!format!("{error:?}").contains("SECRETO"));
        assert!(!error.to_string().contains("SECRETO"));
    }
}

#[test]
fn callback_de_fecha_con_reloj_fijo_como_reference() -> Result<(), Box<dyn std::error::Error>> {
    let probe: Value = serde_json::from_str(include_str!(
        "../../../tests/reference/validator/api-probe.json"
    ))?;
    let today = Date::strptime("%Y-%m-%d", "2025-01-10")?;
    let mut validator = Validator::new();
    validator.set_extension(FixedDateListener { today });
    for day in ["2025-01-01", "2025-01-07", "2025-01-20"] {
        let value = json!({"fechaEmision": format!("{day}T00:00:00-05:00")});
        let invoice: Invoice = serde_json::from_value(value)?;
        let actual: Vec<_> = validator
            .validate_model(&invoice)?
            .into_iter()
            .filter(|violation| violation.message == "Fecha emision superó 7 dias")
            .map(|violation| json!([violation.path, violation.message]))
            .collect();
        assert_eq!(json!(actual), probe["date_callback"][day]);
    }
    Ok(())
}

#[test]
fn entradas_no_confiables_tienen_limites_aun_sin_loader() {
    let validator = Validator::new();
    let large = json!({"campo_desconocido": "X".repeat(1_000_001)});
    assert_eq!(
        validator.validate_raw("SinLoader", &large),
        Err(ValidatorError::Limit("tamaño de entrada"))
    );
    let mut deep = Value::Null;
    for _ in 0..34 {
        deep = json!([deep]);
    }
    assert_eq!(
        validator.validate_raw("SinLoader", &deep),
        Err(ValidatorError::Limit("profundidad"))
    );
}

#[test]
fn listener_extiende_reglas_de_propiedad_sin_mutacion_global()
-> Result<(), Box<dyn std::error::Error>> {
    let probe: Value = serde_json::from_str(include_str!(
        "../../../tests/reference/validator/api-probe.json"
    ))?;
    let mut validator = Validator::new();
    let original = validator.metadata_for("Client")?;
    validator.set_extension(PropertyListener);
    let extended = validator.metadata_for("Client")?;
    assert_eq!(extended.fields.len(), original.fields.len() + 1);
    assert_eq!(
        extended.fields.last().map(|field| field.field.as_str()),
        Some("email")
    );
    assert_eq!(probe["property_extension"]["has_email"], json!(true));
    assert_eq!(validator.metadata_for("SinLoader")?.fields.len(), 0);
    let errors = validator.validate_model(&Client::default())?;
    assert_eq!(
        errors.last().map(|error| error.path.as_str()),
        Some("email")
    );
    assert_eq!(
        errors.last().map(|error| error.message.as_str()),
        Some("Email obligatoria")
    );
    let actual: Vec<_> = errors
        .iter()
        .map(|error| json!([error.path, error.message]))
        .collect();
    assert_eq!(json!(actual), probe["property_extension"]["violations"]);
    Ok(())
}
