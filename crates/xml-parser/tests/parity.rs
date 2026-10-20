//! Paridad con la referencia.
//!
//! Lee los casos sintéticos de `tests/reference/xml-parser/cases.json`, los
//! ejecuta con los parsers Rust y compara el modelo completo, campo a campo,
//! con la salida del oráculo de referencia (`expected.json`, generado por
//! el oráculo externo (`xml_parser`) con el código de referencia original). Toda
//! divergencia debe figurar en `differences.json` (por caso y ruta) y toda
//! entrada registrada debe seguir observándose.

#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use cpeforge_core::Decimal;
use cpeforge_core::ports::DocumentParser;
use cpeforge_xml_parser::{
    DespatchParser, InvoiceParser, NoteParser, ParseOptions, PerceptionParser, ReceiptParser,
    RetentionParser, SummaryParser, VoidedDocument, VoidedParser, XmlDocument, XmlParserError,
    XmlReader,
};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use serde_json::{Map, Value, json};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Campos de los modelos que no existen en la referencia (extensiones de
/// CPEForge), por clase: la guía de remisión transportista (`31`).
const CPEFORGE_EXTENSIONS: &[(&str, &str)] = &[
    ("Despatch", "remitente"),
    ("Despatch", "pagadorFlete"),
    ("Shipment", "subcontratador"),
];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference_dir() -> PathBuf {
    workspace().join("tests/reference/xml-parser")
}

fn read_json(path: &Path) -> TestResult<Value> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn input_bytes(input: &Value) -> TestResult<Vec<u8>> {
    if let Some(xml) = input["xml"].as_str() {
        return Ok(xml.as_bytes().to_vec());
    }
    if let Some(b64) = input["b64"].as_str() {
        return STANDARD
            .decode(b64)
            .map_err(|e| format!("base64: {e:?}").into());
    }
    if let Some(file) = input["file"].as_str() {
        return Ok(fs::read(workspace().join(file))?);
    }
    let repeat = &input["repeat"];
    if let (Some(prefix), Some(unit), Some(count), Some(suffix)) = (
        repeat["prefix"].as_str(),
        repeat["unit"].as_str(),
        repeat["count"].as_u64(),
        repeat["suffix"].as_str(),
    ) {
        let mut out = prefix.as_bytes().to_vec();
        for _ in 0..count {
            out.extend_from_slice(unit.as_bytes());
        }
        out.extend_from_slice(suffix.as_bytes());
        return Ok(out);
    }
    Err(format!("entrada desconocida: {input}").into())
}

/// Resultado Rust en la misma forma que el oráculo de referencia.
fn outcome<T: serde::Serialize>(class: &str, result: Result<T, XmlParserError>) -> Value {
    match result {
        Ok(document) => json!({
            "class": class,
            "snapshot": serde_json::to_value(&document).unwrap_or(Value::Null),
        }),
        Err(err) => error_value(&err),
    }
}

fn error_value(err: &XmlParserError) -> Value {
    json!({"error": {
        "class": err.error_class(),
        "message": err.error_message(),
        "rust": err.to_string(),
    }})
}

fn run_parser(case: &Value, options: ParseOptions, bytes: &[u8]) -> Value {
    let dom = case["mode"].as_str() == Some("dom");
    macro_rules! run {
        ($parser:ty, $class:expr) => {{
            let parser = <$parser>::with_options(options);
            let result = if dom {
                XmlDocument::parse(bytes).and_then(|d| parser.parse_document(&d))
            } else {
                parser
                    .parse(bytes)
                    .and_then(|d| d.ok_or(XmlParserError::Empty))
            };
            outcome($class, result)
        }};
    }
    match case["parser"].as_str().unwrap_or_default() {
        "invoice" => run!(InvoiceParser, "Invoice"),
        "note" => run!(NoteParser, "Note"),
        "despatch" => run!(DespatchParser, "Despatch"),
        "receipt" => run!(ReceiptParser, "Receipt"),
        "summary" => run!(SummaryParser, "Summary"),
        "retention" => run!(RetentionParser, "Retention"),
        "perception" => run!(PerceptionParser, "Perception"),
        "voided" => {
            let parser = VoidedParser::with_options(options);
            let result = if dom {
                XmlDocument::parse(bytes).and_then(|d| parser.parse_document(&d))
            } else {
                parser
                    .parse(bytes)
                    .and_then(|d| d.ok_or(XmlParserError::Empty))
            };
            match result {
                Ok(VoidedDocument::Voided(v)) => outcome("Voided", Ok(v)),
                Ok(VoidedDocument::Reversion(r)) => outcome("Reversion", Ok(r)),
                Ok(_) => json!({"harness_error": "variante de VoidedDocument desconocida"}),
                Err(err) => error_value(&err),
            }
        }
        other => json!({"harness_error": format!("parser desconocido {other}")}),
    }
}

fn run_reader(case: &Value, bytes: &[u8]) -> Value {
    let reader = match XmlReader::load_xml(bytes) {
        Ok(reader) => reader,
        Err(err) => return error_value(&err),
    };
    let describe =
        |node: cpeforge_xml_parser::Node<'_>| json!({"name": node.name(), "value": node.value()});
    let mut out = Vec::new();
    for op in case["reader"].as_array().into_iter().flatten() {
        let query = op["query"].as_str().unwrap_or_default();
        let result = (|| -> Result<Value, XmlParserError> {
            let context = match op["context"].as_str() {
                Some(c) => reader.node(c, None)?,
                None => None,
            };
            Ok(match op["op"].as_str().unwrap_or_default() {
                "value" => match op.get("default") {
                    Some(default) => json!(reader.value_or(query, context, default.as_str())?),
                    None => json!(reader.value(query, context)?),
                },
                "node" => reader.node(query, context)?.map_or(Value::Null, describe),
                _ => match reader.nodes(query, context)? {
                    Some(nodes) => Value::Array(nodes.into_iter().map(describe).collect()),
                    None => json!({"$false": true}),
                },
            })
        })();
        out.push(result.unwrap_or_else(|err| json!({"$error": {"rust": err.to_string()}})));
    }
    json!({"reader": out})
}

fn timestamp(value: &Value) -> TestResult<Timestamp> {
    let unix = value["unix"].as_i64().ok_or("now.unix")?;
    let micro = value["micro"].as_i64().ok_or("now.micro")?;
    Ok(Timestamp::from_microsecond(unix * 1_000_000 + micro)?)
}

// ---------------------------------------------------------------------------
// Comparación
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Diff {
    case: String,
    path: String,
    reference: Value,
    rust: Value,
}

struct Window {
    /// Ventana `[antes, después]` de la llamada de la referencia.
    reference: (Timestamp, Timestamp),
    /// Instante fijo usado por Rust para «ahora».
    rust: Timestamp,
}

struct Comparator {
    case: String,
    window: Option<Window>,
    diffs: Vec<Diff>,
    /// (clase, campo) con valor de la referencia no nulo comparado.
    covered: BTreeSet<(String, String)>,
}

impl Comparator {
    fn diff(&mut self, path: &str, reference: &Value, rust: &Value) {
        self.diffs.push(Diff {
            case: self.case.clone(),
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
                self.compare_datetime(path, &map["$datetime"], rust);
            }
            Value::Array(items) => match rust.as_array() {
                Some(rust_items) if rust_items.len() == items.len() => {
                    for (index, (p, r)) in items.iter().zip(rust_items).enumerate() {
                        self.compare(&format!("{path}[{index}]"), p, r);
                    }
                }
                _ => self.diff(path, reference, rust),
            },
            Value::Object(map) => match rust.as_object() {
                Some(rust_map) => {
                    let keys: BTreeSet<&String> = map.keys().chain(rust_map.keys()).collect();
                    for key in keys {
                        self.compare(
                            &format!("{path}.{key}"),
                            map.get(key).unwrap_or(&Value::Null),
                            rust_map.get(key).unwrap_or(&Value::Null),
                        );
                    }
                }
                None => self.diff(path, reference, rust),
            },
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
            reference.keys().filter(|k| *k != "$class").collect();
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
            if !reference[*key].is_null() {
                self.covered.insert((class.clone(), (*key).clone()));
            }
            self.compare(&format!("{path}.{key}"), &reference[*key], &rust_map[*key]);
        }
    }

    fn compare_float(&mut self, path: &str, reference: &Map<String, Value>, rust: &Value) {
        let reference_text = reference["$float"].as_str().unwrap_or_default();
        let equal = match (
            parse_reference_float(reference_text),
            rust.as_str().and_then(parse_signed_decimal),
        ) {
            (Some(p), Some(r)) => p == r && p.is_sign_negative() == r.is_sign_negative(),
            _ => false,
        };
        if !equal {
            self.diff(path, &Value::Object(reference.clone()), rust);
        }
    }

    fn compare_datetime(&mut self, path: &str, reference: &Value, rust: &Value) {
        let Some(zoned) = rust.as_str().and_then(|t| Zoned::from_str(t).ok()) else {
            self.diff(path, reference, rust);
            return;
        };
        let rust_desc = describe_zoned(&zoned);
        // «Ahora» de la referencia (`new DateTime()` o texto vacío): el instante cae en
        // la ventana de la llamada; Rust usa un reloj fijo.
        let reference_instant = reference["unix"]
            .as_i64()
            .zip(reference["micro"].as_i64())
            .and_then(|(s, m)| Timestamp::from_microsecond(s * 1_000_000 + m).ok());
        if let (Some(window), Some(instant)) = (&self.window, reference_instant) {
            if instant >= window.reference.0 && instant <= window.reference.1 {
                if zoned.timestamp() != window.rust
                    || reference["tz"] != rust_desc["tz"]
                    || reference["offset"] != rust_desc["offset"]
                {
                    self.diff(path, reference, &rust_desc);
                }
                return;
            }
        }
        for key in ["local", "offset", "unix", "micro", "tz"] {
            if reference[key] != rust_desc[key] {
                self.diff(&format!("{path}.{key}"), &reference[key], &rust_desc[key]);
            }
        }
    }
}

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

fn parse_signed_decimal(text: &str) -> Option<Decimal> {
    let mut value = Decimal::from_str(text).ok()?;
    if value.is_zero() && text.starts_with('-') {
        value.set_sign_negative(true);
    }
    Some(value)
}

fn describe_zoned(zoned: &Zoned) -> Value {
    let dt = zoned.datetime();
    let micro = dt.subsec_nanosecond() / 1000;
    let tz = match zoned.time_zone().iana_name() {
        Some(name) => name.to_owned(),
        None => {
            let seconds = zoned.offset().seconds();
            let sign = if seconds < 0 { '-' } else { '+' };
            let abs = seconds.unsigned_abs();
            format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
        }
    };
    json!({
        "local": format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{micro:06}",
            dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute(), dt.second()
        ),
        "offset": zoned.offset().seconds(),
        "unix": zoned.timestamp().as_second(),
        "micro": micro,
        "tz": tz,
    })
}

/// Compara una llamada: documento (clase + modelo) o error.
fn compare_call(comparator: &mut Comparator, path: &str, reference: &Value, rust: &Value) {
    match (reference.get("error"), rust.get("error")) {
        (Some(reference_err), Some(rust_err)) => {
            for key in ["class", "message"] {
                if reference_err[key] != rust_err[key] {
                    let mut rust_view = rust_err.clone();
                    if let Some(map) = rust_view.as_object_mut() {
                        map.retain(|k, _| k == key || k == "rust");
                    }
                    comparator.diff(
                        &format!("{path}.error.{key}"),
                        &reference_err[key],
                        &rust_view,
                    );
                }
            }
        }
        (None, None) => {
            if reference["class"] != rust["class"] {
                comparator.diff(
                    &format!("{path}.class"),
                    &reference["class"],
                    &rust["class"],
                );
            }
            comparator.compare(
                &format!("{path}.snapshot"),
                &reference["snapshot"],
                &rust["snapshot"],
            );
        }
        _ => {
            let summary = |v: &Value| match v.get("error") {
                Some(err) => json!({"error": err}),
                None => json!({"class": v["class"]}),
            };
            comparator.diff(
                &format!("{path}.outcome"),
                &summary(reference),
                &summary(rust),
            );
        }
    }
}

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
        for field in ["id", "path"] {
            if entry[field].as_str().is_none_or(str::is_empty) {
                return Err(format!("diferencia sin {field}: {entry}").into());
            }
        }
        perfil_seguro::validate(entry)?;
        out.push(Registered {
            case: entry["id"].as_str().unwrap_or_default().to_owned(),
            path: entry["path"].as_str().unwrap_or_default().to_owned(),
        });
    }
    Ok(out)
}

#[test]
fn reference_and_rust_outputs_match() -> TestResult {
    let dir = reference_dir();
    let cases = read_json(&dir.join("cases.json"))?;
    let expected = read_json(&dir.join("expected.json"))?;
    let results = expected["results"]
        .as_object()
        .ok_or("expected.json sin results")?;
    let cases = cases["cases"].as_array().ok_or("cases.json sin cases")?;
    if cases.len() != results.len() {
        return Err(format!(
            "{} casos y {} resultados de referencia",
            cases.len(),
            results.len()
        )
        .into());
    }
    let mut diffs = Vec::new();
    let mut covered = BTreeSet::new();
    // Salidas Rust para la verificación independiente
    // (scripts/xml_parser_independent_verify.py).
    let mut rust_outputs = Map::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("caso sin id")?;
        let reference = results
            .get(id)
            .ok_or_else(|| format!("sin resultado de referencia para {id}"))?;
        if reference.get("harness_error").is_some() {
            return Err(format!("{id}: error del arnés de la referencia {reference}").into());
        }
        let bytes = input_bytes(&case["input"])?;
        let zone = TimeZone::get(case["time_zone"].as_str().unwrap_or("America/Lima"))?;
        let mut comparator = Comparator {
            case: id.to_owned(),
            window: None,
            diffs: Vec::new(),
            covered: BTreeSet::new(),
        };
        if case.get("reader").is_some() {
            let rust = run_reader(case, &bytes);
            comparator.compare("$.reader", &reference["reader"], &rust["reader"]);
            rust_outputs.insert(id.to_owned(), rust);
        } else {
            let calls: Vec<(&Value, Vec<u8>)> = match reference.get("calls") {
                Some(calls) => {
                    let calls = calls.as_array().ok_or("calls")?;
                    let second = input_bytes(&case["input2"])?;
                    vec![(&calls[0], bytes.clone()), (&calls[1], second)]
                }
                None => vec![(reference, bytes)],
            };
            for (index, (reference_call, input)) in calls.iter().enumerate() {
                let before = timestamp(&reference_call["now"]["before"])?;
                let after = timestamp(&reference_call["now"]["after"])?;
                let options = ParseOptions::new()
                    .with_time_zone(zone.clone())
                    .with_fixed_now(before);
                let rust = run_parser(case, options, input);
                comparator.window = Some(Window {
                    reference: (before, after),
                    rust: before,
                });
                let path = if calls.len() > 1 {
                    format!("$.calls[{index}]")
                } else {
                    "$".to_owned()
                };
                compare_call(&mut comparator, &path, reference_call, &rust);
                if index == 0 {
                    rust_outputs.insert(id.to_owned(), rust);
                }
            }
        }
        diffs.extend(comparator.diffs);
        covered.extend(comparator.covered);
    }

    if let Some(out) = std::env::var_os("XML_PARSER_RUST_OUT") {
        fs::write(
            out,
            serde_json::to_string_pretty(&json!({"results": rust_outputs}))?,
        )?;
    }

    let registered = load_differences()?;
    let mut problems = Vec::new();
    let mut found = BTreeSet::new();
    for diff in &diffs {
        if registered
            .iter()
            .any(|r| r.case == diff.case && r.path == diff.path)
        {
            found.insert((diff.case.clone(), diff.path.clone()));
        } else {
            problems.push(format!(
                "SIN REGISTRAR {} {}: la referencia {} | Rust {}",
                diff.case, diff.path, diff.reference, diff.rust
            ));
        }
    }
    for entry in &registered {
        if !found.contains(&(entry.case.clone(), entry.path.clone())) {
            problems.push(format!(
                "REGISTRADA Y NO OBSERVADA {} {}",
                entry.case, entry.path
            ));
        }
    }
    // Cobertura: cada propiedad que asigna un setter de los parsers de la referencia
    // (inventory.json) se compara con un valor de la referencia no nulo en algún caso.
    let inventory = read_json(&dir.join("inventory.json"))?;
    let files = inventory["files"]
        .as_array()
        .ok_or("inventario sin archivos")?;
    if files.len() != 10 {
        problems.push(format!(
            "el inventario tiene {} archivos fuente; se esperaban 10",
            files.len()
        ));
    }
    // Cada familia y el lector tienen al menos un caso con documento de la referencia.
    for family in [
        "invoice",
        "note",
        "despatch",
        "receipt",
        "summary",
        "voided",
        "retention",
        "perception",
        "reader",
    ] {
        let documented = cases.iter().any(|c| {
            c["parser"] == family
                && results
                    .get(c["id"].as_str().unwrap_or_default())
                    .is_some_and(|r| r.get("snapshot").is_some() || r.get("reader").is_some())
        });
        if !documented {
            problems.push(format!("FAMILIA SIN CASO VÁLIDO: {family}"));
        }
    }
    let covered_keys: BTreeSet<&str> = covered.iter().map(|(_, key)| key.as_str()).collect();
    let mut setters = BTreeSet::new();
    for file in files {
        for setter in file["setters"].as_array().into_iter().flatten() {
            setters.insert(setter.as_str().unwrap_or_default());
        }
    }
    for setter in &setters {
        if !covered_keys.contains(setter) {
            problems.push(format!("SIN COBERTURA no nula: {setter}"));
        }
    }
    println!(
        "casos: {}; divergencias registradas: {}",
        cases.len(),
        found.len()
    );
    println!(
        "propiedades asignadas por los parsers de la referencia: {}",
        setters.len()
    );
    println!(
        "campos de la referencia no nulos comparados: {}",
        covered.len()
    );
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n").into())
    }
}
