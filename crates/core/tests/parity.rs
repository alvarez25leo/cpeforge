//! Comparación diferencial referencia ↔ Rust de la referencia.
//!
//! Ejecuta en Rust los mismos casos sintéticos que el oráculo externo (`core`)
//! ejecutó contra la referencia y compara el resultado con las salidas
//! La referencia guardadas en `tests/reference/core/expected/`.
//!
//! Reglas de comparación (cada normalización está justificada por campo):
//!
//! - `float` la referencia ↔ `Decimal`: igualdad numérica entre la representación más
//!   corta de ida y vuelta del `float` y el decimal Rust, incluido el signo de
//!   cero. La escala no se compara porque un `float` no la tiene.
//! - Fechas: instante (segundos y microsegundos), hora local, desfase y nombre
//!   de zona. No se compara la clase (`DateTime`/`DateTimeImmutable`):
//!   Rust no tiene esa distinción.
//! - `cdrZip`: bytes exactos (la referencia cadena binaria ↔ Base64 en serde).
//! - Errores: correspondencia explícita entre el error de la referencia y el error Rust
//!   del mismo campo (ver `expected_missing_field` y `compare_hydrate_error`).
//!
//! Toda divergencia debe figurar exactamente en `differences.json`; una
//! divergencia no registrada o un registro sin divergencia hace fallar la prueba.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::{
    AdditionalDoc, Despatch, DespatchDetail, Direction, Driver, Puerto, Shipment, Transportist,
    Vehicle,
};
use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use cpeforge_core::model::response::{
    BaseResult, BillResult, CdrResponse, ResponseError, StatusCdrResult, StatusResult,
    SummaryResult,
};
use cpeforge_core::model::retention::{Exchange, Payment, Retention, RetentionDetail};
use cpeforge_core::model::sale::{
    BaseSale, Charge, Cuota, DetailAttribute, Detraction, EmbededDespatch, Invoice, Legend, Note,
    PaymentTerms, Prepayment, Receipt, RelatedDocument, SaleDetail, SalePerception,
};
use cpeforge_core::model::summary::{Summary, SummaryDetail, SummaryPerception};
use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};
use cpeforge_core::service::{BasicToken, InvalidServiceResponse};
use cpeforge_core::{CoreError, Decimal, Document, Zoned};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

#[path = "support/hydrate.rs"]
mod hydrate;
#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

use hydrate::{HydrateError, TestResult, deserialize, harness_error, to_serde};

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/core")
}

fn read_json(path: &Path) -> TestResult<Value> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

// ---------------------------------------------------------------------------
// Ejecución Rust de un caso
// ---------------------------------------------------------------------------

type CallOutcome = Result<Value, CoreError>;

struct RustRun {
    calls: BTreeMap<String, CallOutcome>,
    snapshot: Value,
}

/// Con `#[serde(flatten)]` serde pierde la ruta del error (`.`). Para
/// verificar igualmente qué campo se rechazó, se reintenta quitando cada clave
/// de primer nivel: la clave cuya ausencia hace válido el objeto es la culpable.
fn locate_flattened_error<T: DeserializeOwned>(value: &Value, err: HydrateError) -> HydrateError {
    let HydrateError::Serde { path, message } = err else {
        return err;
    };
    if path != "." {
        return HydrateError::Serde { path, message };
    }
    let Some(map) = value.as_object() else {
        return HydrateError::Serde { path, message };
    };
    let culprits: Vec<&String> = map
        .keys()
        .filter(|key| {
            let mut reduced = map.clone();
            reduced.remove(*key);
            serde_json::from_value::<T>(Value::Object(reduced)).is_ok()
        })
        .collect();
    match culprits.as_slice() {
        [single] => HydrateError::Serde {
            path: format!("{single} (localizado por eliminación)"),
            message,
        },
        _ => HydrateError::Serde { path, message },
    }
}

fn run_root<T, F>(
    value: Value,
    calls: &[String],
    call: F,
) -> TestResult<Result<RustRun, HydrateError>>
where
    T: DeserializeOwned + Serialize,
    F: Fn(&T, &str) -> Option<CallOutcome>,
{
    let object: T = match deserialize(value.clone()) {
        Ok(object) => object,
        Err(err) => return Ok(Err(locate_flattened_error::<T>(&value, err))),
    };
    let mut outcomes = BTreeMap::new();
    for name in calls {
        let outcome = call(&object, name)
            .ok_or_else(|| harness_error(format!("llamada sin equivalente: {name}")))?;
        outcomes.insert(name.clone(), outcome);
    }
    Ok(Ok(RustRun {
        calls: outcomes,
        snapshot: serde_json::to_value(&object)?,
    }))
}

fn name_of(document: &dyn Document) -> CallOutcome {
    document.name().map(Value::String)
}

fn none<T>(_: &T, _: &str) -> Option<CallOutcome> {
    None
}

fn document_call<T: Document>(object: &T, method: &str) -> Option<CallOutcome> {
    (method == "getName").then(|| name_of(object))
}

fn run_case(
    class: &str,
    value: Value,
    calls: &[String],
) -> TestResult<Result<RustRun, HydrateError>> {
    match class {
        "Address" => run_root::<Address, _>(value, calls, none),
        "Company" => run_root::<Company, _>(value, calls, none),
        "Client" => run_root::<Client, _>(value, calls, none),
        "BaseSale" => run_root::<BaseSale, _>(value, calls, document_call),
        "Invoice" => run_root::<Invoice, _>(value, calls, document_call),
        "Note" => run_root::<Note, _>(value, calls, document_call),
        "Receipt" => run_root::<Receipt, _>(value, calls, document_call),
        "SaleDetail" => run_root::<SaleDetail, _>(value, calls, none),
        "Charge" => run_root::<Charge, _>(value, calls, none),
        "Cuota" => run_root::<Cuota, _>(value, calls, none),
        "DetailAttribute" => run_root::<DetailAttribute, _>(value, calls, none),
        "Detraction" => run_root::<Detraction, _>(value, calls, none),
        "Document" => run_root::<RelatedDocument, _>(value, calls, none),
        "EmbededDespatch" => run_root::<EmbededDespatch, _>(value, calls, none),
        "Legend" => run_root::<Legend, _>(value, calls, none),
        "Prepayment" => run_root::<Prepayment, _>(value, calls, none),
        "SalePerception" => run_root::<SalePerception, _>(value, calls, none),
        "PaymentTerms" | "FormaPagoContado" | "FormaPagoCredito" => {
            run_root::<PaymentTerms, _>(value, calls, none)
        }
        "Despatch" => run_root::<Despatch, _>(value, calls, document_call),
        "AdditionalDoc" => {
            run_root::<AdditionalDoc, _>(value, calls, |doc: &AdditionalDoc, method: &str| {
                (method == "getNro").then(|| doc.nro().map(|nro| Value::String(nro.to_owned())))
            })
        }
        "DespatchDetail" => run_root::<DespatchDetail, _>(value, calls, none),
        "Direction" => run_root::<Direction, _>(value, calls, none),
        "Driver" => run_root::<Driver, _>(value, calls, none),
        "Puerto" => run_root::<Puerto, _>(value, calls, none),
        "Shipment" => run_root::<Shipment, _>(value, calls, none),
        "Transportist" => run_root::<Transportist, _>(value, calls, none),
        "Vehicle" => run_root::<Vehicle, _>(value, calls, none),
        "Retention" => run_root::<Retention, _>(value, calls, document_call),
        "RetentionDetail" => run_root::<RetentionDetail, _>(value, calls, none),
        "Payment" => run_root::<Payment, _>(value, calls, none),
        "Exchange" => run_root::<Exchange, _>(value, calls, none),
        "Perception" => run_root::<Perception, _>(value, calls, document_call),
        "PerceptionDetail" => run_root::<PerceptionDetail, _>(value, calls, none),
        "Summary" => {
            run_root::<Summary, _>(
                value,
                calls,
                |summary: &Summary, method: &str| match method {
                    "getXmlId" => Some(summary.xml_id().map(Value::String)),
                    other => document_call(summary, other),
                },
            )
        }
        "SummaryDetail" => run_root::<SummaryDetail, _>(value, calls, none),
        "SummaryPerception" => run_root::<SummaryPerception, _>(value, calls, none),
        "Voided" => {
            run_root::<Voided, _>(value, calls, |voided: &Voided, method: &str| match method {
                "getXmlId" => Some(voided.xml_id().map(Value::String)),
                other => document_call(voided, other),
            })
        }
        "Reversion" => {
            run_root::<Reversion, _>(value, calls, |reversion: &Reversion, method: &str| {
                match method {
                    "getXmlId" => Some(reversion.xml_id().map(Value::String)),
                    other => document_call(reversion, other),
                }
            })
        }
        "VoidedDetail" => run_root::<VoidedDetail, _>(value, calls, none),
        "BaseResult" => run_root::<BaseResult, _>(value, calls, none),
        "BillResult" => run_root::<BillResult, _>(value, calls, none),
        "SummaryResult" => run_root::<SummaryResult, _>(value, calls, none),
        "StatusResult" => run_root::<StatusResult, _>(value, calls, none),
        "StatusCdrResult" => run_root::<StatusCdrResult, _>(value, calls, none),
        "CdrResponse" => {
            run_root::<CdrResponse, _>(value, calls, |cdr: &CdrResponse, method: &str| {
                (method == "isAccepted").then(|| Ok(Value::Bool(cdr.is_accepted())))
            })
        }
        "Error" => run_root::<ResponseError, _>(value, calls, none),
        "BasicToken" => run_root::<BasicToken, _>(value, calls, none),
        other => Err(harness_error(format!(
            "clase raíz sin equivalente Rust: {other}"
        ))),
    }
}

/// Causa de prueba equivalente a `LogicException` (mensaje y código).
#[derive(Debug)]
struct LogicError {
    message: String,
    code: i64,
}

impl fmt::Display for LogicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for LogicError {}

/// Caso raíz `InvalidServiceResponseException`: se construye con la API Rust y
/// se describe con la misma forma que el oráculo de referencia.
fn run_exception(map: &Map<String, Value>) -> TestResult<Value> {
    let args = map
        .get("$ctor")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let error = if args.is_empty() {
        InvalidServiceResponse::default()
    } else {
        let message = args[0].as_str().ok_or("mensaje no es texto")?;
        let code = args.get(1).and_then(Value::as_i64).unwrap_or_default();
        let mut error = InvalidServiceResponse::new(message, code);
        if let Some(previous) = args.get(2) {
            let ctor = previous["$ctor"].as_array().ok_or("causa sin $ctor")?;
            let message = ctor[0]
                .as_str()
                .ok_or("mensaje de causa no es texto")?
                .to_owned();
            let code = ctor.get(1).and_then(Value::as_i64).unwrap_or_default();
            error = error.with_source(LogicError { message, code });
        }
        error
    };
    Ok(describe_error(&error, "InvalidServiceResponse"))
}

fn describe_error(error: &(dyn StdError + 'static), class: &str) -> Value {
    let (message, code) = if let Some(err) = error.downcast_ref::<InvalidServiceResponse>() {
        (err.message().to_owned(), err.code())
    } else if let Some(err) = error.downcast_ref::<LogicError>() {
        (err.message.clone(), err.code)
    } else {
        (error.to_string(), 0)
    };
    let previous = error.source().map_or(Value::Null, |source| {
        let class = if source.is::<LogicError>() {
            "LogicError"
        } else {
            "?"
        };
        describe_error(source, class)
    });
    json!({"$throwable": class, "message": message, "code": code, "previous": previous})
}

// ---------------------------------------------------------------------------
// Comparación
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Diff {
    path: String,
    reference: Value,
    rust: Value,
}

struct Comparator<'a> {
    inventory: &'a Inventory,
    diffs: Vec<Diff>,
    /// (clase que declara el getter, propiedad) con valor no nulo comparado.
    covered: BTreeSet<(String, String)>,
}

impl Comparator<'_> {
    fn diff(&mut self, path: &str, reference: &Value, rust: &Value) {
        self.diffs.push(Diff {
            path: path.to_owned(),
            reference: reference.clone(),
            rust: rust.clone(),
        });
    }

    fn compare(&mut self, path: &str, reference: &Value, rust: &Value) {
        match reference {
            Value::Object(map) if map.contains_key("$class") => {
                self.compare_object(path, map, rust)
            }
            Value::Object(map) if map.contains_key("$float") => self.compare_float(path, map, rust),
            Value::Object(map) if map.contains_key("$datetime") => {
                self.compare_datetime(path, &map["$datetime"], rust)
            }
            Value::Array(items) => match rust.as_array() {
                Some(rust_items) if rust_items.len() == items.len() => {
                    for (index, (reference_item, rust_item)) in
                        items.iter().zip(rust_items).enumerate()
                    {
                        self.compare(&format!("{path}[{index}]"), reference_item, rust_item);
                    }
                }
                _ => self.diff(path, reference, rust),
            },
            Value::Object(_) => self.diff(path, reference, rust),
            scalar => {
                if scalar != rust {
                    self.diff(path, reference, rust);
                }
            }
        }
    }

    fn compare_object(&mut self, path: &str, reference: &Map<String, Value>, rust: &Value) {
        let Some(rust_map) = rust.as_object() else {
            self.diff(path, &Value::Object(reference.clone()), rust);
            return;
        };
        let class = reference["$class"].as_str().unwrap_or_default().to_owned();
        let reference_keys: BTreeSet<&String> =
            reference.keys().filter(|key| *key != "$class").collect();
        let rust_keys: BTreeSet<&String> = rust_map.keys().collect();
        for missing in reference_keys.difference(&rust_keys) {
            self.diff(
                &format!("{path}.{missing}"),
                &reference[*missing],
                &json!("<ausente en Rust>"),
            );
        }
        for extra in rust_keys.difference(&reference_keys) {
            // Campos que solo existen en CPEForge: la referencia no los tiene
            // y en sus casos siempre quedan vacíos.
            if CPEFORGE_EXTENSIONS.contains(&(class.as_str(), extra.as_str()))
                && rust_map[*extra].is_null()
            {
                continue;
            }
            self.diff(
                &format!("{path}.{extra}"),
                &json!("<ausente en la referencia>"),
                &rust_map[*extra],
            );
        }
        for key in reference_keys.intersection(&rust_keys) {
            let (reference_value, rust_value) = (&reference[*key], &rust_map[*key]);
            let field_path = format!("{path}.{key}");
            if !reference_value.is_null() {
                if let Some(owner) = self.inventory.getter_owner(&class, key) {
                    self.covered.insert((owner, (*key).clone()));
                }
            }
            if key.as_str() == "cdrZip" {
                self.compare_bytes(&field_path, reference_value, rust_value);
            } else if class == "AdditionalDoc"
                && key.as_str() == "nro"
                && is_reference_error(reference_value, NRO_TYPE_ERROR)
            {
                // getNro() lanza TypeError con nro null; el campo Rust es `None`
                // y `AdditionalDoc::nro()` devuelve el error (caso additional-doc-empty).
                if !rust_value.is_null() {
                    self.diff(&field_path, reference_value, rust_value);
                }
            } else {
                self.compare(&field_path, reference_value, rust_value);
            }
        }
    }

    fn compare_float(&mut self, path: &str, reference: &Map<String, Value>, rust: &Value) {
        let reference_text = reference["$float"].as_str().unwrap_or_default();
        let equal = match (
            parse_reference_float(reference_text),
            rust.as_str().and_then(parse_signed_decimal),
        ) {
            (Some(reference_value), Some(rust_value)) => {
                reference_value == rust_value
                    && reference_value.is_sign_negative() == rust_value.is_sign_negative()
            }
            _ => false,
        };
        if !equal {
            self.diff(path, &Value::Object(reference.clone()), rust);
        }
    }

    fn compare_datetime(&mut self, path: &str, reference: &Value, rust: &Value) {
        let described = rust
            .as_str()
            .and_then(|text| Zoned::from_str(text).ok())
            .map(|zoned| describe_zoned(&zoned));
        let Some(rust_desc) = described else {
            self.diff(path, reference, rust);
            return;
        };
        for key in ["local", "offset", "unix", "micro", "tz"] {
            if reference[key] != rust_desc[key] {
                self.diff(&format!("{path}.{key}"), &reference[key], &rust_desc[key]);
            }
        }
    }

    fn compare_bytes(&mut self, path: &str, reference: &Value, rust: &Value) {
        let reference_bytes = match reference {
            Value::Null => None,
            Value::String(text) => Some(text.as_bytes().to_vec()),
            Value::Object(map) => map
                .get("$bytes")
                .and_then(Value::as_str)
                .and_then(|b64| STANDARD.decode(b64).ok()),
            _ => None,
        };
        let rust_bytes = match rust {
            Value::Null => None,
            Value::String(b64) => STANDARD.decode(b64).ok(),
            _ => None,
        };
        if reference_bytes != rust_bytes || (reference_bytes.is_none() && !reference.is_null()) {
            self.diff(path, reference, rust);
        }
    }
}

/// Campos de los modelos que no existen en la referencia (extensiones de
/// CPEForge), por clase: la guía de remisión transportista (`31`).
const CPEFORGE_EXTENSIONS: &[(&str, &str)] = &[
    ("Despatch", "remitente"),
    ("Despatch", "pagadorFlete"),
    ("Shipment", "subcontratador"),
];

const NRO_TYPE_ERROR: &str =
    "AdditionalDoc::getNro(): Return value must be of type string, null returned";
const NULL_COMPANY_ERROR: &str = "Call to a member function getRuc() on null";
const NULL_DATE_ERROR: &str =
    "getDateWithTimezone(): Argument #1 ($date) must be of type DateTimeInterface, null given";

fn is_reference_error(value: &Value, message_part: &str) -> bool {
    value["$error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains(message_part))
}

/// Interpreta la representación de la referencia de un `float` (`1.0e-6`, `-0.0`, `1.0e+30`).
fn parse_reference_float(text: &str) -> Option<Decimal> {
    if text.contains(['e', 'E']) {
        let mut value = Decimal::from_scientific(text).ok()?;
        if value.is_zero() && text.starts_with('-') {
            value.set_sign_negative(true);
        }
        return Some(value);
    }
    parse_signed_decimal(text)
}

/// `Decimal::from_str` normaliza `-0.00` a `0.00`; aquí se conserva el signo
/// para poder comparar el signo de cero.
fn parse_signed_decimal(text: &str) -> Option<Decimal> {
    let mut value = Decimal::from_str(text).ok()?;
    if value.is_zero() && text.starts_with('-') {
        value.set_sign_negative(true);
    }
    Some(value)
}

fn describe_zoned(zoned: &Zoned) -> Value {
    let dt = zoned.datetime();
    let local = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.subsec_nanosecond() / 1000
    );
    let tz = zoned.time_zone();
    let tz_name = match tz.iana_name() {
        Some(name) => name.to_owned(),
        None => {
            let seconds = zoned.offset().seconds();
            let sign = if seconds < 0 { '-' } else { '+' };
            let abs = seconds.unsigned_abs();
            format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
        }
    };
    json!({
        "local": local,
        "offset": zoned.offset().seconds(),
        "unix": zoned.timestamp().as_second(),
        "micro": dt.subsec_nanosecond() / 1000,
        "tz": tz_name,
    })
}

/// Campo Rust que debe faltar cuando la referencia falla en un método derivado.
fn expected_missing_field(class: &str, reference_error: &Value) -> Option<&'static str> {
    let message = reference_error["$error"]["message"].as_str()?;
    if message.contains(NULL_COMPANY_ERROR) {
        return Some(if class == "Receipt" {
            "person"
        } else {
            "company"
        });
    }
    if message.contains(NULL_DATE_ERROR) {
        return match class {
            "Summary" => Some("fecResumen"),
            "Voided" | "Reversion" => Some("fecComunicacion"),
            _ => None,
        };
    }
    if message.contains(NRO_TYPE_ERROR) {
        return Some("nro");
    }
    None
}

fn compare_call(
    comparator: &mut Comparator<'_>,
    path: &str,
    class: &str,
    reference: &Value,
    rust: &CallOutcome,
) {
    match (reference.get("$error"), rust) {
        (Some(_), Err(CoreError::MissingField { field, .. })) => {
            if expected_missing_field(class, reference) != Some(*field) {
                comparator.diff(path, reference, &json!({"$error": field}));
            }
        }
        (None, Ok(value)) => comparator.compare(path, reference, value),
        (_, Ok(value)) => comparator.diff(path, reference, value),
        (_, Err(err)) => comparator.diff(path, reference, &json!({"$error": err.to_string()})),
    }
}

/// Correspondencia entre el fallo de la referencia al construir y el fallo Rust.
fn compare_hydrate_error(reference: &Value, rust: &HydrateError) -> bool {
    let class = reference["class"].as_str().unwrap_or_default();
    let message = reference["message"].as_str().unwrap_or_default();
    match rust {
        // TypeError de un setter: `Clase::setX(): Argument #1 ($x) must be of type ...`.
        HydrateError::Serde { path, .. } if class == "TypeError" => {
            let param = message
                .split("($")
                .nth(1)
                .and_then(|rest| rest.split(')').next())
                .unwrap_or_default();
            !param.is_empty() && path.split(['.', '[', ' ']).any(|segment| segment == param)
        }
        // ArgumentCountError: `... ::__construct(), 0 passed and exactly 2 expected`.
        HydrateError::Arity {
            class: rust_class,
            given,
            expected,
        } if class == "ArgumentCountError" => message.contains(&format!(
            "\\{rust_class}::__construct(), {given} passed and exactly {expected} expected"
        )),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Inventario de la referencia
// ---------------------------------------------------------------------------

/// Clase padre y getters declarados como (método, clave de instantánea).
type ClassGetters = (Option<String>, Vec<(String, String)>);

struct Inventory {
    /// Nombre corto -> clase padre y getters declarados.
    classes: BTreeMap<String, ClassGetters>,
    raw: Value,
}

impl Inventory {
    fn load() -> TestResult<Self> {
        let raw = read_json(&reference_dir().join("inventory.json"))?;
        let mut classes = BTreeMap::new();
        for (fqcn, class) in raw["classes"].as_object().ok_or("inventario sin clases")? {
            let short = short_name(fqcn);
            let parent = class["parent"].as_str().map(short_name);
            let getters = class["methods"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|m| m["kind"] == "getter")
                .map(|m| {
                    (
                        m["name"].as_str().unwrap_or_default().to_owned(),
                        getter_key(m),
                    )
                })
                .collect();
            classes.insert(short, (parent, getters));
        }
        Ok(Self { classes, raw })
    }

    /// Clase que declara el getter cuyo nombre de instantánea es `key`.
    fn getter_owner(&self, class: &str, key: &str) -> Option<String> {
        let mut current = Some(class.to_owned());
        while let Some(name) = current {
            let (parent, getters) = self.classes.get(&name)?;
            if getters.iter().any(|(_, getter_key)| getter_key == key) {
                return Some(name);
            }
            current = parent.clone();
        }
        None
    }
}

fn short_name(fqcn: &str) -> String {
    fqcn.rsplit('\\').next().unwrap_or(fqcn).to_owned()
}

/// Clave de instantánea de un getter: `getFoo` -> `foo`, `isFoo` -> `foo`.
fn getter_key(method: &Value) -> String {
    let name = method["name"].as_str().unwrap_or_default();
    let stripped = name
        .strip_prefix("get")
        .or_else(|| name.strip_prefix("is"))
        .unwrap_or(name);
    let mut chars = stripped.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Prueba principal
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Registered {
    case: String,
    path: String,
}

fn load_differences() -> TestResult<Vec<Registered>> {
    let raw = read_json(&reference_dir().join("differences.json"))?;
    let mut out = Vec::new();
    for entry in raw["differences"]
        .as_array()
        .ok_or("differences.json sin lista")?
    {
        for field in ["case", "path"] {
            if entry[field].as_str().is_none_or(str::is_empty) {
                return Err(harness_error(format!("diferencia sin {field}: {entry}")));
            }
        }
        perfil_seguro::validate(entry).map_err(harness_error)?;
        out.push(Registered {
            case: entry["case"].as_str().unwrap_or_default().to_owned(),
            path: entry["path"].as_str().unwrap_or_default().to_owned(),
        });
    }
    Ok(out)
}

#[test]
fn reference_and_rust_outputs_match() -> TestResult {
    let dir = reference_dir();
    let reference = read_json(&dir.join("inventory.json"))?["reference"].clone();
    let inventory = Inventory::load()?;
    let mut comparator = Comparator {
        inventory: &inventory,
        diffs: Vec::new(),
        covered: BTreeSet::new(),
    };
    let mut found: Vec<(String, Diff)> = Vec::new();
    let mut roots: BTreeSet<String> = BTreeSet::new();
    let mut calls_seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut total = 0usize;

    let mut case_files: Vec<PathBuf> = fs::read_dir(dir.join("cases"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    case_files.sort();
    if case_files.is_empty() {
        return Err(harness_error("no hay archivos de casos"));
    }
    for case_file in case_files {
        let file_name = case_file.file_name().ok_or("ruta sin nombre")?.to_owned();
        let cases = read_json(&case_file)?;
        let expected = read_json(&dir.join("expected").join(&file_name))?;
        if expected["reference"]["commit"] != reference["baseline_commit"] {
            return Err(harness_error(format!(
                "{file_name:?} no se generó con la referencia fijada"
            )));
        }
        let results = expected["results"]
            .as_object()
            .ok_or("salida de referencia sin resultados")?;
        let case_list = cases["cases"]
            .as_array()
            .ok_or("archivo de casos sin lista")?;
        if results.len() != case_list.len() {
            return Err(harness_error(format!(
                "{file_name:?}: casos y salidas de referencia no coinciden"
            )));
        }
        for case in case_list {
            total += 1;
            let id = case["id"].as_str().ok_or("caso sin id")?;
            let reference = results
                .get(id)
                .ok_or_else(|| harness_error(format!("sin salida de referencia para {id}")))?;
            let input = case["input"].as_object().ok_or("entrada sin objeto")?;
            let class = input["$class"].as_str().ok_or("entrada sin $class")?;
            roots.insert(class.to_owned());
            let calls: Vec<String> = case["calls"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            for call in &calls {
                calls_seen.insert((class.to_owned(), call.clone()));
            }
            let before = comparator.diffs.len();

            if class == "InvalidServiceResponseException" {
                let rust = run_exception(input)?;
                compare_throwable(&mut comparator, "snapshot", &reference["snapshot"], &rust);
            } else {
                let rust = match to_serde(&case["input"])? {
                    Ok(value) => run_case(class, value, &calls)?,
                    Err(err) => Err(err),
                };
                match (reference.get("hydrate_error"), rust) {
                    (Some(reference_error), Err(rust_error)) => {
                        if !compare_hydrate_error(reference_error, &rust_error) {
                            comparator.diff(
                                "hydrate",
                                reference_error,
                                &json!(rust_error.to_string()),
                            );
                        }
                    }
                    (Some(reference_error), Ok(run)) => {
                        comparator.diff("hydrate", reference_error, &run.snapshot)
                    }
                    (None, Err(rust_error)) => comparator.diff(
                        "hydrate",
                        &json!("<sin error en la referencia>"),
                        &json!(rust_error.to_string()),
                    ),
                    (None, Ok(run)) => {
                        let reference_calls = reference["calls"]
                            .as_object()
                            .ok_or("salida de referencia sin calls")?;
                        if reference_calls.len() != run.calls.len() {
                            return Err(harness_error(format!(
                                "{id}: llamadas de la referencia y Rust no coinciden"
                            )));
                        }
                        for (name, outcome) in &run.calls {
                            let reference_call = reference_calls
                                .get(name)
                                .ok_or("llamada ausente en la referencia")?;
                            compare_call(
                                &mut comparator,
                                &format!("calls.{name}"),
                                class,
                                reference_call,
                                outcome,
                            );
                        }
                        comparator.compare("snapshot", &reference["snapshot"], &run.snapshot);
                    }
                }
            }
            for diff in comparator.diffs.drain(before..).collect::<Vec<_>>() {
                found.push((id.to_owned(), diff));
            }
        }
    }

    // Toda divergencia debe estar registrada exactamente y viceversa.
    let registered = load_differences()?;
    let mut problems = Vec::new();
    for (case, diff) in &found {
        if !registered
            .iter()
            .any(|r| &r.case == case && r.path == diff.path)
        {
            problems.push(format!(
                "DIVERGENCIA NO REGISTRADA {case} {}\n  la referencia:  {}\n  Rust: {}",
                diff.path, diff.reference, diff.rust
            ));
        }
    }
    for entry in &registered {
        if !found
            .iter()
            .any(|(case, diff)| case == &entry.case && diff.path == entry.path)
        {
            problems.push(format!(
                "REGISTRO OBSOLETO {} {} (ya no diverge)",
                entry.case, entry.path
            ));
        }
    }

    // Cobertura del inventario: cada getter con al menos un valor no nulo comparado,
    // cada clase como raíz de algún caso y cada método derivado ejecutado.
    for (class, (_, getters)) in &inventory.classes {
        for (_, key) in getters {
            if !comparator.covered.contains(&(class.clone(), key.clone())) {
                problems.push(format!("SIN COBERTURA no nula: {class}::{key}"));
            }
        }
    }
    for (fqcn, class) in inventory.raw["classes"]
        .as_object()
        .ok_or("inventario sin clases")?
    {
        let short = short_name(fqcn);
        let kind = class["kind"].as_str().unwrap_or_default();
        let is_model = kind.ends_with("class") && short != "TimeZonePe";
        if is_model && !roots.contains(&short) {
            problems.push(format!("CLASE SIN CASO RAÍZ: {short}"));
        }
        for method in class["methods"].as_array().into_iter().flatten() {
            let name = method["name"].as_str().unwrap_or_default();
            if method["kind"] == "custom"
                && method["visibility"] == "public"
                && name != "__construct"
                && !calls_seen.contains(&(short.clone(), name.to_owned()))
            {
                problems.push(format!("MÉTODO SIN LLAMADA: {short}::{name}"));
            }
        }
    }

    println!("casos: {total}; divergencias registradas: {}", found.len());
    for (case, diff) in &found {
        println!(
            "  registrada {case} {}: la referencia {} | Rust {}",
            diff.path, diff.reference, diff.rust
        );
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(harness_error(problems.join("\n")))
    }
}

fn compare_throwable(comparator: &mut Comparator<'_>, path: &str, reference: &Value, rust: &Value) {
    if reference.is_null() || rust.is_null() {
        if reference.is_null() != rust.is_null() {
            comparator.diff(path, reference, rust);
        }
        return;
    }
    let class_pairs = [
        (
            "CPEForge\\Services\\InvalidServiceResponseException",
            "InvalidServiceResponse",
        ),
        ("LogicException", "LogicError"),
    ];
    if !class_pairs.contains(&(
        reference["$throwable"].as_str().unwrap_or_default(),
        rust["$throwable"].as_str().unwrap_or_default(),
    )) {
        comparator.diff(
            &format!("{path}.$throwable"),
            &reference["$throwable"],
            &rust["$throwable"],
        );
    }
    for key in ["message", "code"] {
        if reference[key] != rust[key] {
            comparator.diff(&format!("{path}.{key}"), &reference[key], &rust[key]);
        }
    }
    compare_throwable(
        comparator,
        &format!("{path}.previous"),
        &reference["previous"],
        &rust["previous"],
    );
}
