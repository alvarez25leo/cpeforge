//! Reglas de retención y percepción: número de pago mayor que 0
//! (`GreaterThan`), indicador de emisión excepcional de dos caracteres
//! (`Length`) y un solo pago o cobro por comprobante relacionado (`Count`,
//! `cac:Payment` admite una sola aparición por documento relacionado en el
//! XSD de SUNAT).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use cpeforge_core::model::retention::{Payment, Retention, RetentionDetail};
use cpeforge_validator::{Metadata, Rule, RuleExtension, Validator, ValidatorError, Violation};
use serde_json::{Value, json};

const CODE_EXACT_LENGTH: &str = "4b6f5c76-22b4-409d-af16-fbe823ba9332";
const CODE_GREATER_THAN: &str = "778b7ae0-84d3-481a-9dec-35fdb64b1d78";
const CODE_TOO_MANY: &str = "756b1212-697c-468d-a9ad-50dd783bb169";
const CODE_TOO_FEW: &str = "bef8e338-6ae5-4caf-b8e2-50e7b0579e69";
const CODE_EXACT_COUNT: &str = "9fe5d43f-3784-4ece-a0e1-473fc02dadbc";

fn s(value: &str) -> Option<String> {
    Some(value.to_owned())
}

fn lima() -> Option<cpeforge_core::Zoned> {
    Some("2026-11-03T10:00:00-05:00[America/Lima]".parse().unwrap())
}

fn company() -> Company {
    Company {
        ruc: s("20000000001"),
        razon_social: s("AGENTE DEMO S.A.C."),
        address: Some(Address {
            ubigueo: s("150101"),
            cod_local: s("0000"),
            ..Address::default()
        }),
        ..Company::default()
    }
}

fn provider() -> Client {
    Client {
        tipo_doc: s("6"),
        num_doc: s("20000000002"),
        rzn_social: s("PROVEEDOR DEMO"),
        ..Client::default()
    }
}

fn payment(numero: Option<i64>) -> Payment {
    Payment {
        numero,
        moneda: s("PEN"),
        importe: Some(Decimal::ONE_HUNDRED),
        fecha: lima(),
    }
}

fn perception(ind_excepcional: Option<&str>, cobros: Vec<Payment>) -> Perception {
    Perception {
        serie: s("P001"),
        correlativo: s("1"),
        fecha_emision: lima(),
        company: Some(company()),
        proveedor: Some(provider()),
        regimen: s("01"),
        tasa: Some(Decimal::TWO),
        imp_percibido: Some(Decimal::TWO),
        imp_cobrado: Some(Decimal::ONE_HUNDRED),
        ind_excepcional: ind_excepcional.map(str::to_owned),
        details: Some(vec![PerceptionDetail {
            tipo_doc: s("01"),
            num_doc: s("F001-1"),
            fecha_emision: lima(),
            imp_total: Some(Decimal::ONE_HUNDRED),
            moneda: s("PEN"),
            cobros: Some(cobros),
            fecha_percepcion: lima(),
            imp_percibido: Some(Decimal::TWO),
            imp_cobrar: Some(Decimal::ONE_HUNDRED),
            ..PerceptionDetail::default()
        }]),
        ..Perception::default()
    }
}

fn retention(pagos: Vec<Payment>) -> Retention {
    Retention {
        serie: s("R001"),
        correlativo: s("1"),
        fecha_emision: lima(),
        company: Some(company()),
        proveedor: Some(provider()),
        regimen: s("01"),
        tasa: Some(Decimal::TWO),
        imp_retenido: Some(Decimal::TWO),
        imp_pagado: Some(Decimal::ONE_HUNDRED),
        details: Some(vec![RetentionDetail {
            tipo_doc: s("01"),
            num_doc: s("F001-1"),
            fecha_emision: lima(),
            imp_total: Some(Decimal::ONE_HUNDRED),
            moneda: s("PEN"),
            pagos: Some(pagos),
            fecha_retencion: lima(),
            imp_retenido: Some(Decimal::TWO),
            imp_pagar: Some(Decimal::ONE_HUNDRED),
            ..RetentionDetail::default()
        }]),
        ..Retention::default()
    }
}

/// Validadores de ambos perfiles (base y UBL 2.1): las reglas son comunes.
fn validators() -> [Validator; 2] {
    let mut v21 = Validator::new();
    v21.set_version(Some("2.1"));
    [Validator::new(), v21]
}

fn paths(violations: &[Violation]) -> Vec<&str> {
    violations.iter().map(|v| v.path.as_str()).collect()
}

#[test]
fn documentos_validos_no_tienen_violaciones() {
    for validator in validators() {
        let ok = |doc: &Perception| validator.validate_document(doc).unwrap();
        assert_eq!(ok(&perception(None, vec![payment(None)])), []);
        assert_eq!(ok(&perception(Some("01"), vec![payment(Some(1))])), []);
        let retention = retention(vec![payment(Some(3))]);
        assert_eq!(validator.validate_document(&retention).unwrap(), []);
    }
}

#[test]
fn indicador_excepcional_debe_tener_dos_caracteres() {
    for validator in validators() {
        for value in ["1", "001", "", "ñ"] {
            let violations = validator
                .validate_document(&perception(Some(value), vec![payment(None)]))
                .unwrap();
            assert_eq!(paths(&violations), ["indExcepcional"], "{value:?}");
            let violation = &violations[0];
            assert_eq!(violation.code.as_deref(), Some(CODE_EXACT_LENGTH));
            assert_eq!(
                violation.message,
                "This value should have exactly 2 characters."
            );
            assert_eq!(violation.parameters["{{ limit }}"], "2");
        }
        // Se cuentan caracteres, no bytes.
        let unicode = perception(Some("ñá"), vec![payment(None)]);
        assert_eq!(validator.validate_document(&unicode).unwrap(), []);
    }
}

#[test]
fn numero_de_pago_debe_ser_mayor_que_cero() {
    for validator in validators() {
        for numero in [0, -1, i64::MIN] {
            let violations = validator
                .validate_document(&retention(vec![payment(Some(numero))]))
                .unwrap();
            assert_eq!(
                paths(&violations),
                ["details[0].pagos[0].numero"],
                "{numero}"
            );
            let violation = &violations[0];
            assert_eq!(violation.code.as_deref(), Some(CODE_GREATER_THAN));
            assert_eq!(violation.message, "This value should be greater than 0.");
            assert_eq!(
                violation.template,
                "This value should be greater than {{ compared_value }}."
            );
            assert_eq!(violation.parameters["{{ value }}"], numero.to_string());
            assert_eq!(violation.parameters["{{ compared_value }}"], "0");
            assert_eq!(violation.parameters["{{ compared_value_type }}"], "int");
            assert_eq!(violation.invalid, json!(numero));
        }
        let cobro = perception(None, vec![payment(Some(0))]);
        assert_eq!(
            paths(&validator.validate_document(&cobro).unwrap()),
            ["details[0].cobros[0].numero"]
        );
        for numero in [1, i64::MAX] {
            let ok = retention(vec![payment(Some(numero))]);
            assert_eq!(validator.validate_document(&ok).unwrap(), [], "{numero}");
        }
    }
}

#[test]
fn un_solo_pago_o_cobro_por_documento_relacionado() {
    for validator in validators() {
        let two = vec![payment(Some(1)), payment(Some(2))];
        let violations = validator
            .validate_document(&retention(two.clone()))
            .unwrap();
        assert_eq!(paths(&violations), ["details[0].pagos"]);
        let violation = &violations[0];
        assert_eq!(violation.code.as_deref(), Some(CODE_TOO_MANY));
        assert_eq!(
            violation.message,
            "This collection should contain 1 element or less."
        );
        assert_eq!(violation.parameters["{{ count }}"], "2");
        assert_eq!(violation.parameters["{{ limit }}"], "1");

        let violations = validator.validate_document(&perception(None, two)).unwrap();
        assert_eq!(paths(&violations), ["details[0].cobros"]);

        // Sin pagos sigue aplicando NotBlank, no Count.
        let violations = validator.validate_document(&retention(Vec::new())).unwrap();
        assert_eq!(paths(&violations), ["details[0].pagos"]);
        assert_ne!(violations[0].code.as_deref(), Some(CODE_TOO_MANY));
    }
}

#[test]
fn reglas_nuevas_en_la_tabla_de_ambos_perfiles() {
    for validator in validators() {
        let payment = validator.metadata_for("Payment").unwrap();
        let fields: Vec<&str> = payment.fields.iter().map(|f| f.field.as_str()).collect();
        assert_eq!(fields, ["numero", "moneda", "importe", "fecha"]);
        assert_eq!(payment.fields[0].rules[0].name, "GreaterThan");
        assert_eq!(payment.fields[0].rules[0].options["value"], json!(0));

        let perception = validator.metadata_for("Perception").unwrap();
        let fields: Vec<&str> = perception.fields.iter().map(|f| f.field.as_str()).collect();
        let at = fields.iter().position(|f| *f == "indExcepcional").unwrap();
        assert_eq!(fields[at - 1], "observacion");
        assert_eq!(fields[at + 1], "details");

        for (model, field) in [("PerceptionDetail", "cobros"), ("RetentionDetail", "pagos")] {
            let metadata = validator.metadata_for(model).unwrap();
            let rules: Vec<&str> = metadata
                .fields
                .iter()
                .find(|f| f.field == field)
                .unwrap()
                .rules
                .iter()
                .map(|r| r.name.as_str())
                .collect();
            assert_eq!(rules, ["NotBlank", "Count", "Valid"], "{model}.{field}");
        }
    }
}

#[test]
fn greater_than_compara_numeros_y_cadenas_numericas() {
    let validator = Validator::new();
    let check = |value: Value| {
        validator
            .validate_raw(
                "Payment",
                &json!({"numero": value, "moneda": "PEN", "importe": 1, "fecha": "x"}),
            )
            .map(|violations| paths(&violations).contains(&"numero"))
    };
    assert!(!check(json!(2)).unwrap());
    assert!(!check(json!(0.5)).unwrap());
    assert!(check(json!(-0.5)).unwrap());
    assert!(!check(json!(" 3")).unwrap());
    assert!(check(json!("0")).unwrap());
    assert!(!check(Value::Null).unwrap());
    assert!(matches!(check(json!("abc")), Err(ValidatorError::Model(_))));
    assert!(matches!(check(json!([1])), Err(ValidatorError::Model(_))));
}

#[test]
fn count_rechaza_valores_que_no_son_colecciones() {
    let validator = Validator::new();
    let result = validator.validate_raw("RetentionDetail", &json!({"pagos": "uno"}));
    assert!(matches!(result, Err(ValidatorError::Model(_))));
}

/// Añade a `RetentionDetail.pagos` una regla `Count` con las opciones dadas.
struct CountRule(Value);
impl RuleExtension for CountRule {
    fn extend_metadata(&self, model_type: &str, metadata: &mut Metadata) {
        if model_type == "RetentionDetail" {
            if let Some(field) = metadata.fields.iter_mut().find(|f| f.field == "pagos") {
                field.rules = vec![Rule {
                    name: "Count".to_owned(),
                    options: self.0.clone(),
                }];
            }
        }
    }

    fn validate(&self, _model_type: &str, _model: &Value) -> Vec<Violation> {
        Vec::new()
    }
}

#[test]
fn count_con_minimo_y_cantidad_exacta() {
    let messages = json!({
        "minMessage": "This collection should contain {{ limit }} element or more.|This collection should contain {{ limit }} elements or more.",
        "maxMessage": "This collection should contain {{ limit }} element or less.|This collection should contain {{ limit }} elements or less.",
        "exactMessage": "This collection should contain exactly {{ limit }} element.|This collection should contain exactly {{ limit }} elements.",
    });
    let with = |extra: Value| {
        let mut options = messages.clone();
        options
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let mut validator = Validator::new();
        validator.set_extension(CountRule(options));
        validator
    };
    let pagos = |n: usize| json!({"pagos": vec![json!({}); n]});
    let first = |validator: &Validator, n: usize| {
        validator
            .validate_raw("RetentionDetail", &pagos(n))
            .unwrap()
            .into_iter()
            .find(|v| v.path == "pagos")
    };

    let min = with(json!({"min": 2}));
    let violation = first(&min, 1).unwrap();
    assert_eq!(violation.code.as_deref(), Some(CODE_TOO_FEW));
    assert_eq!(
        violation.message,
        "This collection should contain 2 elements or more."
    );
    assert!(first(&min, 2).is_none());

    let exact = with(json!({"min": 2, "max": 2}));
    for n in [1, 3] {
        let violation = first(&exact, n).unwrap();
        assert_eq!(violation.code.as_deref(), Some(CODE_EXACT_COUNT), "{n}");
        assert_eq!(
            violation.message,
            "This collection should contain exactly 2 elements."
        );
        assert_eq!(violation.parameters["{{ count }}"], n.to_string());
    }
    assert!(first(&exact, 2).is_none());

    let divisible = with(json!({"divisibleBy": 2}));
    assert!(matches!(
        divisible.validate_raw("RetentionDetail", &pagos(1)),
        Err(ValidatorError::UnsupportedRule(_))
    ));
}
