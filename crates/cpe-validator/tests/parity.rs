//! Paridad con la referencia.
//!
//! Ejecuta los casos de `tests/reference/cpe-validator/cases.json` con la
//! API Rust, paso a paso y sobre las mismas instancias, y compara cada paso
//! (tipo, ruta, los cinco campos y el orden de cada `CpeError`, excepción con
//! clase y mensaje) con la salida del oráculo de referencia (`expected.json`, generado
//! por el oráculo externo (`cpe_validator`) con el código de referencia original).
//! Toda divergencia debe figurar en `differences.json` (por caso y ruta) y
//! toda entrada registrada debe seguir observándose.
//!
//! Con `CPE_VALIDATOR_RUST_OUT` se escribe la salida Rust para el verificador
//! independiente (scripts/cpe_validator_independent_verify.py).

#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use cpeforge_cpe_validator::{
    CpeError, CpeValidator, CpeValidatorError, DocumentType, ErrorResultParser, Limits,
    ResultParser, RuleResolver, Stylesheet, Validator, XmlDocument, XmlTypeResolver,
    XslPathResolver, XslValidator,
};
use serde_json::{Map, Value, json};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference_dir() -> PathBuf {
    workspace().join("tests/reference/cpe-validator")
}

fn read_json(path: &Path) -> TestResult<Value> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// Directorios temporales de un caso y sustitución de rutas.
struct Workdir {
    base: PathBuf,
    rules: PathBuf,
    outside: PathBuf,
    replacements: Vec<(String, &'static str)>,
}

impl Workdir {
    fn create(id: &str, case: &Value) -> TestResult<Self> {
        let base = std::env::temp_dir()
            .join(format!("cpeforge-cpe-parity-{}", std::process::id()))
            .join(id);
        let _ = fs::remove_dir_all(&base);
        let rules = base.join("rules");
        let outside = base.join("outside");
        fs::create_dir_all(&rules)?;
        fs::create_dir_all(&outside)?;
        let mut replacements = Vec::new();
        for (dir, token) in [(&rules, "{ROOT}"), (&outside, "{OUTSIDE}")] {
            replacements.push((fs::canonicalize(dir)?.to_string_lossy().into_owned(), token));
            replacements.push((dir.to_string_lossy().into_owned(), token));
        }
        let dir = Self {
            base,
            rules,
            outside,
            replacements,
        };
        dir.materialize(&dir.rules, &case["rules"])?;
        dir.materialize(&dir.outside, &case["outside"])?;
        if let Some(links) = case["symlinks"].as_object() {
            for (rel, target) in links {
                let link = dir.rules.join(rel);
                if let Some(parent) = link.parent() {
                    fs::create_dir_all(parent)?;
                }
                std::os::unix::fs::symlink(dir.vars(target.as_str().unwrap_or_default()), link)?;
            }
        }
        Ok(dir)
    }

    fn vars(&self, text: &str) -> String {
        text.replace("{ROOT}", &self.rules.to_string_lossy())
            .replace("{OUTSIDE}", &self.outside.to_string_lossy())
    }

    fn materialize(&self, dir: &Path, files: &Value) -> TestResult {
        for (rel, content) in files.as_object().into_iter().flatten() {
            let path = dir.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, self.vars(content.as_str().unwrap_or_default()))?;
        }
        Ok(())
    }

    fn normalize(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (from, to) in &self.replacements {
            out = out.replace(from.as_str(), to);
        }
        out
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn input_xml(input: &Value) -> TestResult<Option<Vec<u8>>> {
    if input.is_null() {
        return Ok(None);
    }
    if let Some(xml) = input["xml"].as_str() {
        return Ok(Some(xml.as_bytes().to_vec()));
    }
    if let Some(file) = input["file"].as_str() {
        return Ok(Some(fs::read(workspace().join(file))?));
    }
    Err(format!("entrada desconocida: {input}").into())
}

fn opt(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |v| Value::String(v.to_owned()))
}

fn error_row(error: &CpeError) -> Value {
    json!([
        opt(error.code.as_deref()),
        opt(error.message.as_deref()),
        opt(error.level.map(|l| l.code())),
        opt(error.node_path.as_deref()),
        opt(error.node_value.as_deref()),
    ])
}

fn errors_outcome(errors: &[CpeError]) -> Value {
    json!({"outcome": "errors", "errors": errors.iter().map(error_row).collect::<Vec<_>>()})
}

/// Nombre de la variante de `CpeValidatorError` (para errores sin
/// equivalente de excepción).
fn variant(error: &CpeValidatorError) -> String {
    let debug = format!("{error:?}");
    let end = debug.find([' ', '{', '(']).unwrap_or(debug.len());
    debug[..end].to_owned()
}

fn exception(error: &CpeValidatorError, dir: &Workdir) -> Value {
    let class = error
        .error_class()
        .map_or_else(|| format!("rust:{}", variant(error)), str::to_owned);
    json!({"outcome": "exception", "class": class, "message": dir.normalize(&error.to_string())})
}

fn result_outcome(result: Result<Vec<CpeError>, CpeValidatorError>, dir: &Workdir) -> Value {
    match result {
        Ok(errors) => errors_outcome(&errors),
        Err(error) => exception(&error, dir),
    }
}

fn not_representable(reason: &str) -> Value {
    json!({"outcome": "not-representable", "reason": reason})
}

/// Resolutor sin regla para ningún tipo (la referencia usa un TypeResolver que
/// devuelve `'00'`, tipo sin ruta en XslPathResolver).
struct NoRule;

impl RuleResolver for NoRule {
    fn rule_path(&self, _: DocumentType) -> Option<PathBuf> {
        None
    }
}

fn limits_for(case: &Value) -> Limits {
    let mut limits = Limits::default();
    if let Some(max) = case["rust_limits"]["max_xml_bytes"].as_u64() {
        limits = limits.with_max_xml_bytes(max);
    }
    limits
}

/// Ejecuta un caso y devuelve `{"steps": [...]}` en la forma del oráculo.
fn run_case(id: &str, case: &Value) -> TestResult<Value> {
    let dir = Workdir::create(id, case)?;
    let limits = limits_for(case);
    let parser = ErrorResultParser;
    let mut xsl_validator = XslValidator::new(ErrorResultParser);
    // La referencia compila la hoja dentro de validate(); Rust al configurarla. El
    // error de compilación se informa en el paso de validación para comparar
    // el mismo punto observable.
    let mut pending_xsl_error: Option<Value> = None;
    let cpe = CpeValidator::new(&dir.rules).with_limits(limits);
    let cpe_no_rule = CpeValidator::with_parts(XmlTypeResolver, NoRule, XslValidator::default())
        .with_limits(limits);
    let no_rule = case["resolver"].as_str() == Some("unmapped_type");
    let mut steps = Vec::new();
    for step in case["steps"].as_array().into_iter().flatten() {
        let op = step["op"].as_str().unwrap_or_default();
        let out = match op {
            "parse" => match step["raw"].as_str() {
                None => not_representable("ResultParser::parse recibe &str"),
                Some(raw) => parser
                    .parse(raw)
                    .map_or_else(|| json!({"outcome": "null"}), |e| errors_outcome(&[e])),
            },
            "path" => match step["code"].as_str() {
                None => not_representable("XslPathResolver::path_for_code recibe &str"),
                Some(code) => {
                    let path = XslPathResolver::new(step["base"].as_str().unwrap_or_default())
                        .path_for_code(code);
                    json!({"outcome": "value", "value": path.map(|p| p.to_string_lossy().into_owned())})
                }
            },
            "type_xml" | "type" | "rule" => match input_xml(&step["xml"])? {
                None => not_representable("XmlTypeResolver::resolve_xml recibe bytes"),
                Some(xml) => match XmlTypeResolver.resolve_xml(&xml) {
                    Err(error) => exception(&error, &dir),
                    Ok(found) if op == "rule" => {
                        let path =
                            found.and_then(|t| XslPathResolver::new(&dir.rules).rule_path(t));
                        json!({"outcome": "value", "value": path.map(|p| dir.normalize(&p.to_string_lossy()))})
                    }
                    Ok(found) => {
                        json!({"outcome": "value", "value": found.map(DocumentType::code)})
                    }
                },
            },
            "xsl_set" => {
                let path = dir.rules.join(step["path"].as_str().unwrap_or_default());
                match Stylesheet::from_file(&path, Some(&dir.rules), &limits) {
                    Ok(sheet) => {
                        xsl_validator.set_xsl(sheet);
                        pending_xsl_error = None;
                    }
                    Err(error) => pending_xsl_error = Some(exception(&error, &dir)),
                }
                json!({"outcome": "ok"})
            }
            "xsl_validate" => {
                if let Some(error) = &pending_xsl_error {
                    error.clone()
                } else {
                    let filename = step["filename"].as_str().unwrap_or_default();
                    let xml = input_xml(&step["xml"])?.unwrap_or_default();
                    match XmlDocument::parse_with_limits(&xml, &limits) {
                        Err(error) => exception(&error, &dir),
                        Ok(document) => {
                            result_outcome(xsl_validator.validate(filename, &document), &dir)
                        }
                    }
                }
            }
            "cpe_validate" => {
                let filename = step["filename"].as_str().unwrap_or_default();
                let xml = input_xml(&step["xml"])?.unwrap_or_default();
                let dom = step["mode"].as_str() == Some("dom");
                let result = match (dom, no_rule) {
                    (false, false) => cpe.validate_xml(filename, &xml),
                    (false, true) => cpe_no_rule.validate_xml(filename, &xml),
                    (true, _) => XmlDocument::parse_with_limits(&xml, &limits).and_then(|d| {
                        if no_rule {
                            cpe_no_rule.validate(filename, &d)
                        } else {
                            cpe.validate(filename, &d)
                        }
                    }),
                };
                result_outcome(result, &dir)
            }
            "exists" => {
                json!({"outcome": "value", "value": dir.rules.join(step["path"].as_str().unwrap_or_default()).exists()})
            }
            other => return Err(format!("paso desconocido: {other}").into()),
        };
        steps.push(out);
    }
    Ok(json!({ "steps": steps }))
}

/// Primera ruta divergente de un paso.
fn divergence(index: usize, reference: &Value, rust: &Value) -> Option<String> {
    if reference == rust {
        return None;
    }
    let field = ["outcome", "errors", "value", "class", "message"]
        .into_iter()
        .find(|f| reference.get(*f) != rust.get(*f))
        .unwrap_or("outcome");
    Some(format!("$.steps[{index}].{field}"))
}

#[test]
fn rust_matches_reference_oracle() -> TestResult {
    let cases = read_json(&reference_dir().join("cases.json"))?;
    let expected = read_json(&reference_dir().join("expected.json"))?;
    let registry = read_json(&reference_dir().join("differences.json"))?;
    let commit = expected["reference"]["commit"].as_str();
    assert_eq!(
        commit,
        Some("f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f"),
        "oráculo de otra referencia"
    );

    let mut registered = BTreeSet::new();
    for entry in registry["differences"].as_array().into_iter().flatten() {
        perfil_seguro::validate(entry)?;
        let key = (
            entry["id"].as_str().unwrap_or_default().to_owned(),
            entry["path"].as_str().unwrap_or_default().to_owned(),
        );
        assert!(registered.insert(key.clone()), "entrada duplicada: {key:?}");
    }

    let mut observed = BTreeSet::new();
    let mut unregistered = Vec::new();
    let mut rust_out = Map::new();
    let mut count = 0usize;
    for case in cases["cases"].as_array().into_iter().flatten() {
        let id = case["id"].as_str().ok_or("caso sin id")?;
        let reference = &expected["results"][id];
        assert!(
            reference.get("harness_error").is_none(),
            "{id}: el oráculo de referencia falló: {reference}"
        );
        let rust = run_case(id, case)?;
        let reference_steps = reference["steps"]
            .as_array()
            .ok_or_else(|| format!("{id}: sin salida de referencia"))?;
        let rust_steps = rust["steps"].as_array().ok_or("sin pasos")?;
        assert_eq!(
            reference_steps.len(),
            rust_steps.len(),
            "{id}: número de pasos"
        );
        for (index, (p, r)) in reference_steps.iter().zip(rust_steps).enumerate() {
            if let Some(path) = divergence(index, p, r) {
                let key = (id.to_owned(), path.clone());
                if registered.contains(&key) {
                    observed.insert(key);
                } else {
                    unregistered.push(format!("{id} {path}\n  la referencia:  {p}\n  Rust: {r}"));
                }
            }
        }
        rust_out.insert(id.to_owned(), rust);
        count += 1;
    }
    if let Ok(out) = std::env::var("CPE_VALIDATOR_RUST_OUT") {
        let engine = cpeforge_cpe_validator::engine_info();
        let doc = json!({"engine": {"libxml2": engine.libxml2, "libxslt": engine.libxslt,
                         "xpath_operation_limit": engine.xpath_operation_limit}, "results": rust_out});
        fs::write(out, serde_json::to_string_pretty(&doc)? + "\n")?;
    }
    let stale: Vec<_> = registered.difference(&observed).collect();
    assert!(
        unregistered.is_empty(),
        "{} divergencias sin registrar:\n{}",
        unregistered.len(),
        unregistered.join("\n")
    );
    assert!(
        stale.is_empty(),
        "entradas registradas que ya no se observan: {stale:?}"
    );
    assert_eq!(count, cases["cases"].as_array().map_or(0, Vec::len));
    Ok(())
}

/// El mapa de rutas Rust coincide con el que el generador copia de la referencia, y
/// el inventario registra los 14 archivos fuente.
#[test]
fn path_map_and_inventory_match_reference() -> TestResult {
    let cases = read_json(&reference_dir().join("cases.json"))?;
    let paths = cases["paths"].as_object().ok_or("sin mapa")?;
    assert_eq!(paths.len(), 10);
    for t in DocumentType::ALL {
        assert_eq!(
            paths[t.code()].as_str(),
            Some(XslPathResolver::relative_path(t)),
            "{t}"
        );
        assert_eq!(DocumentType::from_code(t.code()), Some(t));
    }
    let inventory = read_json(&reference_dir().join("inventory.json"))?;
    let files: Vec<&str> = inventory["files"]
        .as_array()
        .ok_or("sin archivos")?
        .iter()
        .filter_map(|f| f["file"].as_str())
        .collect();
    assert_eq!(files.len(), 14, "{files:?}");
    let reference_map = &inventory["path_mapping"];
    for t in DocumentType::ALL {
        assert_eq!(
            reference_map[t.code()].as_str(),
            Some(XslPathResolver::relative_path(t)),
            "inventario {t}"
        );
    }
    Ok(())
}
