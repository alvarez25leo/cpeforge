//! Comparación diferencial referencia ↔ Rust de la referencia.
//!
//! Ejecuta en Rust los casos de `tests/reference/report/cases/` y compara con
//! las salidas de la referencia (`tests/reference/report/expected/`):
//!
//! - HTML: igualdad **de bytes**; el instante de `date(null)` es el `now` que
//!   registró el oráculo.
//! - Errores: correspondencia explícita entre la excepción (clase,
//!   excepción envuelta y plantilla) y [`ReportError`]. El texto de Twig no se
//!   compara: lo genera el motor (`diagnostico-runtime`).
//! - QR: modo, versión, máscara, matriz completa y bytes del SVG.
//! - `XmlUtils`, resolver y catálogos: valores exactos.
//!
//! Toda divergencia debe figurar exactamente en `differences.json`, que se
//! valida con el perfil seguro; una entrada sin divergencia también falla.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "../../core/tests/support/hydrate.rs"]
mod hydrate;
#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use base64::Engine as _;
use cpeforge_core::Decimal;
use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{BaseSale, Invoice, Note, Receipt};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_report::qr::{self, QrError, QrMode};
use cpeforge_report::{
    DefaultTemplateResolver, Extra, HtmlReport, ParamValue, RenderCause, ReportDocument,
    ReportError, ReportOptions, ReportParameters, TemplateResolver, TrustedHtml, XmlUtils,
    XmlUtilsError, catalog_value,
};
use hydrate::{TestResult, deserialize, harness_error, to_serde};
use jiff::Timestamp;
use serde_json::{Value, json};

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/report")
}

fn read_json(path: &Path) -> TestResult<Value> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

fn b64(node: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(node["$bytes"].as_str().unwrap())
        .unwrap()
}

/// `{"$repeat": {"$bytes": …}, "length": n}`: patrón repetido hasta `n` bytes.
fn repeat(node: &Value) -> Vec<u8> {
    let chunk = b64(&node["$repeat"]);
    let length = usize::try_from(node["length"].as_u64().unwrap()).unwrap();
    chunk.iter().copied().cycle().take(length).collect()
}

/// Valor neutral → [`ParamValue`].
fn param(node: &Value) -> ParamValue {
    match node {
        Value::Null => ParamValue::Null,
        Value::Bool(b) => ParamValue::Bool(*b),
        Value::Number(n) => ParamValue::Int(n.as_i64().unwrap()),
        Value::String(s) => ParamValue::Text(s.clone()),
        Value::Array(items) => ParamValue::List(items.iter().map(param).collect()),
        Value::Object(map) if map.contains_key("$decimal") => {
            let text = map["$decimal"].as_str().unwrap();
            ParamValue::Decimal(
                Decimal::from_str(text)
                    .or_else(|_| Decimal::from_scientific(text))
                    .unwrap(),
            )
        }
        Value::Object(map) if map.contains_key("$bytes") => ParamValue::Bytes(b64(node)),
        Value::Object(map) if map.contains_key("$repeat") => ParamValue::Bytes(repeat(node)),
        Value::Object(map) => {
            ParamValue::Map(map.iter().map(|(k, v)| (k.clone(), param(v))).collect())
        }
    }
}

/// Parámetros neutrales → [`ReportParameters`].
fn parameters(node: &Value) -> ReportParameters {
    let mut out = ReportParameters::default();
    if let Some(system) = node["system"].as_object() {
        for (key, value) in system {
            match key.as_str() {
                "logo" if value.get("$repeat").is_some() => out.system.logo = Some(repeat(value)),
                "logo" => out.system.logo = Some(b64(value)),
                "hash" => out.system.hash = Some(value.as_str().unwrap().to_owned()),
                "qr" if value.is_string() => {
                    out.system.qr = Some(value.as_str().unwrap().to_owned())
                }
                _ => out.system.other.push((key.clone(), param(value))),
            }
        }
    }
    if let Some(user) = node["user"].as_object() {
        for (key, value) in user {
            match key.as_str() {
                "header" => out.user.header = Some(TrustedHtml::new(value.as_str().unwrap())),
                "footer" => out.user.footer = Some(TrustedHtml::new(value.as_str().unwrap())),
                "numIGV" => out.user.num_igv = Some(param(value)),
                "extras" => {
                    out.user.extras = Some(
                        value
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|e| {
                                Extra::new(
                                    e["name"].as_str().unwrap(),
                                    e["value"].as_str().unwrap(),
                                )
                            })
                            .collect(),
                    );
                }
                _ => out.user.other.push((key.clone(), param(value))),
            }
        }
    }
    out
}

fn render_case(case: &Value, now: i64) -> TestResult<Result<String, ReportError>> {
    let options = &case["options"];
    let mut opts = ReportOptions::default();
    opts.strict_variables = options["strict_variables"].as_bool().unwrap_or(false);
    opts.autoescape = options["autoescape"].as_bool().unwrap_or(true);
    opts.now = Some(Timestamp::from_second(now)?);
    if let Some(dir) = case["template_dir"].as_str() {
        opts.template_dir = Some(reference_dir().join("cases").join(dir));
    }
    let mut report = HtmlReport::new(opts);
    if let Some(globals) = options["globals"].as_object() {
        for (name, value) in globals {
            report.add_global(name.clone(), param(value));
        }
    }
    if let Some(template) = case["template"].as_str() {
        report.set_template(template);
    }
    let params = parameters(&case["params"]);
    let node = &case["document"];
    let class = node["$class"].as_str().ok_or("documento sin $class")?;
    let value = to_serde(node)?.map_err(|e| harness_error(format!("{e}")))?;
    macro_rules! go {
        ($ty:ty) => {{
            let doc: $ty = deserialize(value).map_err(|e| harness_error(format!("{e}")))?;
            Ok(report.render_html(&doc, &params))
        }};
    }
    match class {
        "Invoice" => go!(Invoice),
        "Note" => go!(Note),
        "Despatch" => go!(Despatch),
        "Summary" => go!(Summary),
        "Voided" => go!(Voided),
        "Reversion" => go!(Reversion),
        "Perception" => go!(Perception),
        "Retention" => go!(Retention),
        other => Err(harness_error(format!("clase sin equivalente: {other}"))),
    }
}

/// Plantilla citada en un mensaje de Twig (`in "x.html.twig" at line N`).
fn twig_template(message: &str) -> Option<&str> {
    let start = message.rfind(" in \"")? + 5;
    let rest = &message[start..];
    Some(&rest[..rest.find('"')?])
}

/// Correspondencia entre la excepción y el error Rust.
fn errors_match(reference: &Value, rust: &ReportError) -> bool {
    let class = reference["class"].as_str().unwrap_or_default();
    let message = reference["message"].as_str().unwrap_or_default();
    let previous_class = reference["previous"]["class"].as_str().unwrap_or_default();
    let previous = reference["previous"]["message"]
        .as_str()
        .unwrap_or_default();
    let same_template = |template: &Option<String>| {
        twig_template(message).is_none_or(|t| template.as_deref() == Some(t))
    };
    match rust {
        ReportError::TemplateNotFound { name } => {
            class == "Twig\\Error\\LoaderError"
                && message.starts_with(&format!("Unable to find template \"{name}\""))
        }
        ReportError::InvalidTemplateName { .. } => {
            class == "Twig\\Error\\LoaderError"
                && message.starts_with(
                    "Looks like you try to load a template outside configured directories",
                )
        }
        ReportError::Syntax { template, .. } => {
            class == "Twig\\Error\\SyntaxError" && same_template(template)
        }
        ReportError::Render {
            template, cause, ..
        } => {
            if class != "Twig\\Error\\RuntimeError" || !same_template(template) {
                return false;
            }
            match cause {
                RenderCause::Undefined => {
                    previous_class.is_empty()
                        && (message.starts_with("Impossible to access an attribute")
                            || message.starts_with("Key \"")
                            || message.starts_with("Neither the property")
                            || message.starts_with("Variable \""))
                }
                RenderCause::TypeError { function } => {
                    previous_class == "TypeError"
                        && previous.starts_with(&format!("{function}(): Argument #1"))
                }
                RenderCause::Document(_) => {
                    previous_class == "Error"
                        && previous.starts_with("Call to a member function ")
                        && previous.ends_with(" on null")
                }
                RenderCause::UndefinedMethod { class: c, method } => {
                    previous_class == "Error"
                        && previous.starts_with("Call to undefined method ")
                        && previous.ends_with(&format!("\\{c}::{method}()"))
                }
                RenderCause::Qr(QrError::EmptyContent) => {
                    previous_class == "BaconQrCode\\Exception\\InvalidArgumentException"
                        && previous == "Found empty contents"
                }
                RenderCause::Qr(QrError::DataTooBig) => {
                    previous_class == "BaconQrCode\\Exception\\WriterException"
                        && previous == "Data too big"
                }
                _ => false,
            }
        }
        ReportError::InvalidDocument { class: c } => {
            class == "CPEForge\\Report\\Resolver\\InvalidDocumentException"
                && message == format!("Not found template for {c}")
        }
        _ => false,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    use std::fmt::Write as _;
    let mut out = String::new();
    for byte in sha2::Sha256::digest(bytes) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Primer byte distinto, con contexto.
fn describe(reference: &str, rust: &str) -> String {
    let offset = reference
        .bytes()
        .zip(rust.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(reference.len().min(rust.len()));
    let window = |text: &str| {
        let mut start = offset.saturating_sub(80).min(text.len());
        while !text.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = (offset + 80).min(text.len());
        while !text.is_char_boundary(end) {
            end += 1;
        }
        text[start..end].replace('\n', "⏎")
    };
    format!(
        "byte {offset} (la referencia {} bytes, Rust {} bytes)\n  la referencia:  …{}…\n  Rust: …{}…",
        reference.len(),
        rust.len(),
        window(reference),
        window(rust)
    )
}

/// Diferencias observadas: (familia, caso) → descripción.
type Observed = BTreeMap<(String, String), String>;

fn check_registry(observed: &Observed) -> TestResult {
    let registry = read_json(&reference_dir().join("differences.json"))?;
    let mut registered = BTreeSet::new();
    for entry in registry["differences"]
        .as_array()
        .ok_or("differences no es lista")?
    {
        perfil_seguro::validate(entry)?;
        registered.insert((
            entry["kind"].as_str().unwrap_or_default().to_owned(),
            entry["case"].as_str().unwrap_or_default().to_owned(),
        ));
    }
    let seen: BTreeSet<_> = observed.keys().cloned().collect();
    let unregistered: Vec<_> = seen.difference(&registered).collect();
    let stale: Vec<_> = registered.difference(&seen).collect();
    for key in &unregistered {
        eprintln!("NO REGISTRADA {key:?}: {}", observed[*key]);
    }
    for key in &stale {
        eprintln!("REGISTRO OBSOLETO {key:?}: ya no hay divergencia");
    }
    if !unregistered.is_empty() || !stale.is_empty() {
        return Err(harness_error(format!(
            "{} diferencias sin registrar y {} registros obsoletos",
            unregistered.len(),
            stale.len()
        )));
    }
    Ok(())
}

/// Carpeta donde guardar las salidas Rust para la verificación independiente
/// (`REPORT_RUST_OUT`), si se pidió.
fn rust_out(sub: &str) -> TestResult<Option<PathBuf>> {
    let Some(dir) = std::env::var_os("REPORT_RUST_OUT") else {
        return Ok(None);
    };
    let dir = PathBuf::from(dir).join(sub);
    fs::create_dir_all(&dir)?;
    Ok(Some(dir))
}

fn html_family(observed: &mut Observed) -> TestResult<usize> {
    let dir = reference_dir();
    let out = rust_out("html")?;
    let cases = read_json(&dir.join("cases/html.json"))?;
    let expected = read_json(&dir.join("expected/html.json"))?;
    let mut count = 0;
    for case in cases["cases"].as_array().ok_or("casos")? {
        let id = case["id"].as_str().unwrap();
        let reference = &expected["results"][id];
        let now = reference["now"].as_i64().ok_or("now")?;
        let rust = render_case(case, now)?;
        count += 1;
        if let (Some(out), Ok(html)) = (&out, &rust) {
            if html.len() <= 1024 * 1024 {
                fs::write(out.join(format!("{id}.html")), html)?;
            }
        }
        let diff = match (reference.get("error"), rust) {
            (None, Ok(html)) if reference["stored"] == false => {
                let digest = sha256_hex(html.as_bytes());
                (reference["sha256"] != json!(digest)).then(|| {
                    format!(
                        "SHA-256 distinto: la referencia {} Rust {digest}",
                        reference["sha256"]
                    )
                })
            }
            (None, Ok(html)) => {
                let reference_html =
                    fs::read_to_string(dir.join(format!("expected/html/{id}.html")))?;
                (reference_html != html).then(|| describe(&reference_html, &html))
            }
            (Some(err), Err(rust_err)) => (!errors_match(err, &rust_err)).then(|| {
                format!("errores distintos\n  la referencia:  {err}\n  Rust: {rust_err:?}")
            }),
            (Some(err), Ok(_)) => Some(format!("La referencia falla y Rust no: {err}")),
            (None, Err(rust_err)) => Some(format!("Rust falla y la referencia no: {rust_err:?}")),
        };
        if let Some(diff) = diff {
            observed.insert(("html".into(), id.into()), diff);
        }
    }
    Ok(count)
}

fn qr_family(observed: &mut Observed) -> TestResult<usize> {
    let dir = reference_dir();
    let out = rust_out("qr")?;
    let cases = read_json(&dir.join("cases/qr.json"))?;
    let expected = read_json(&dir.join("expected/qr.json"))?;
    let mut count = 0;
    for case in cases["cases"].as_array().ok_or("casos")? {
        let id = case["id"].as_str().unwrap();
        let reference = &expected["results"][id];
        let content = b64(&case["content"]);
        count += 1;
        let mut problems = Vec::new();
        match (qr::encode(&content), reference.get("rows")) {
            (Ok(symbol), Some(rows)) => {
                let mode = match symbol.mode {
                    QrMode::Numeric => "NUMERIC",
                    QrMode::Alphanumeric => "ALPHANUMERIC",
                    _ => "BYTE",
                };
                if reference["mode"] != mode
                    || reference["version"] != json!(symbol.version)
                    || reference["mask"] != json!(symbol.mask)
                {
                    problems.push(format!(
                        "símbolo de la referencia {}/{}/{} Rust {mode}/{}/{}",
                        reference["mode"],
                        reference["version"],
                        reference["mask"],
                        symbol.version,
                        symbol.mask
                    ));
                }
                let ours: Vec<String> = symbol
                    .modules
                    .iter()
                    .map(|row| row.iter().map(|b| if *b { '1' } else { '0' }).collect())
                    .collect();
                if json!(ours) != *rows {
                    problems.push("matriz distinta".into());
                }
            }
            (Err(err), None) => {
                let message = reference["encode_error"]["message"]
                    .as_str()
                    .unwrap_or_default();
                // Bacon no rechaza el contenido vacío en Encoder::encode (lo hace Writer).
                if message != err.to_string()
                    && !(err == QrError::EmptyContent && reference["mode"].is_string())
                {
                    problems.push(format!(
                        "error de codificación de la referencia {message} Rust {err}"
                    ));
                }
            }
            (Err(QrError::EmptyContent), Some(_)) => {}
            (ours, _) => problems.push(format!(
                "codificación de la referencia {reference} Rust {ours:?}"
            )),
        }
        if let (Some(out), Ok(svg)) = (&out, qr::svg(&content)) {
            fs::write(out.join(format!("{id}.svg")), svg)?;
        }
        match (qr::svg(&content), reference.get("svg_sha256")) {
            (Ok(svg), Some(_)) => {
                let reference_svg = fs::read_to_string(dir.join(format!("expected/qr/{id}.svg")))?;
                if reference_svg != svg {
                    problems.push(format!("SVG distinto: {}", describe(&reference_svg, &svg)));
                }
            }
            (Err(err), None) => {
                let message = reference["error"]["message"].as_str().unwrap_or_default();
                if message != err.to_string() {
                    problems.push(format!(
                        "error de SVG de la referencia {message} Rust {err}"
                    ));
                }
            }
            (ours, _) => problems.push(format!(
                "SVG de la referencia {} Rust {:?}",
                reference["error"],
                ours.err()
            )),
        }
        if !problems.is_empty() {
            observed.insert(("qr".into(), id.into()), problems.join("; "));
        }
    }
    Ok(count)
}

fn xml_result(result: Result<String, XmlUtilsError>) -> Value {
    match result {
        Ok(hash) => json!({ "hash": hash }),
        Err(err) => json!({ "rust_error": format!("{err:?}") }),
    }
}

fn xml_family(observed: &mut Observed) -> TestResult<usize> {
    let dir = reference_dir();
    let cases = read_json(&dir.join("cases/xml.json"))?;
    let expected = read_json(&dir.join("expected/xml.json"))?;
    let utils = XmlUtils::default();
    let tmp = tempfile::tempdir()?;
    let mut count = 0;
    for case in cases["cases"].as_array().ok_or("casos")? {
        let id = case["id"].as_str().unwrap();
        let reference = &expected["results"][id];
        let mut ours = serde_json::Map::new();
        if let Some(file) = case["file"].as_str() {
            ours.insert(
                "file".into(),
                xml_result(utils.hash_sign_from_file(tmp.path().join(file))),
            );
        } else {
            let xml = b64(&case["xml"]);
            let path = tmp.path().join(format!("{id}.xml"));
            fs::write(&path, &xml)?;
            let string = xml_result(utils.hash_sign(&xml));
            ours.insert("string".into(), string.clone());
            ours.insert("file".into(), xml_result(utils.hash_sign_from_file(&path)));
            // DOMDocument ya cargado: misma lectura sobre el árbol de roxmltree.
            ours.insert("doc".into(), string);
        }
        count += 1;
        let mut problems = Vec::new();
        for (mode, rust) in &ours {
            let reference_mode = &reference[mode];
            let same = match (reference_mode.get("hash"), rust.get("hash")) {
                (Some(a), Some(b)) => a == b,
                // `ValueError` de la referencia con contenido vacío ↔ XmlUtilsError::Empty.
                (None, None) => {
                    reference_mode["error"]["class"] == "ValueError"
                        && rust["rust_error"] == "Empty"
                }
                _ => false,
            };
            if !same {
                problems.push(format!(
                    "{mode}: la referencia {reference_mode} Rust {rust}"
                ));
            }
        }
        if !problems.is_empty() {
            observed.insert(("xml".into(), id.into()), problems.join("; "));
        }
    }
    Ok(count)
}

fn resolver_family(observed: &mut Observed) -> TestResult<usize> {
    let dir = reference_dir();
    let cases = read_json(&dir.join("cases/resolver.json"))?;
    let expected = read_json(&dir.join("expected/resolver.json"))?;
    let resolver = DefaultTemplateResolver;
    let mut count = 0;
    for case in cases["cases"].as_array().ok_or("casos")? {
        let id = case["id"].as_str().unwrap();
        let reference = &expected["results"][id];
        let document: Box<dyn ReportDocument> = match case["class"].as_str().unwrap() {
            "Invoice" => Box::new(Invoice::default()),
            "Note" => Box::new(Note::default()),
            "Retention" => Box::new(Retention::default()),
            "Perception" => Box::new(Perception::default()),
            "Despatch" => Box::new(Despatch::default()),
            "Summary" => Box::new(Summary::default()),
            "Voided" => Box::new(Voided::default()),
            "Reversion" => Box::new(Reversion::default()),
            "BaseSale" => Box::new(BaseSale::default()),
            "Receipt" => Box::new(Receipt::default()),
            other => return Err(harness_error(format!("clase {other}"))),
        };
        count += 1;
        let same = match (
            resolver.template(document.as_ref()),
            reference.get("template"),
        ) {
            (Ok(t), Some(expected)) => expected == &json!(t),
            (Err(err), None) => errors_match(&reference["error"], &err),
            _ => false,
        };
        if !same {
            observed.insert(
                ("resolver".into(), id.into()),
                format!("La referencia {reference}"),
            );
        }
    }
    Ok(count)
}

fn catalog_family(observed: &mut Observed) -> TestResult<usize> {
    let expected = read_json(&reference_dir().join("expected/catalog.json"))?;
    let mut count = 0;
    for group in ["entries", "misses"] {
        for entry in expected["results"][group].as_array().ok_or("catálogo")? {
            let (catalog, key) = (
                entry["catalog"].as_str().unwrap(),
                entry["key"].as_str().unwrap(),
            );
            count += 1;
            if catalog_value(key, catalog) != entry["value"].as_str().unwrap() {
                observed.insert(
                    ("catalog".into(), format!("{catalog}:{key}")),
                    format!("La referencia {entry}"),
                );
            }
        }
    }
    Ok(count)
}

#[test]
fn reference_and_rust_match() -> TestResult {
    let mut observed = Observed::new();
    let counts = [
        ("html", html_family(&mut observed)?),
        ("qr", qr_family(&mut observed)?),
        ("xml", xml_family(&mut observed)?),
        ("resolver", resolver_family(&mut observed)?),
        ("catalog", catalog_family(&mut observed)?),
    ];
    for (family, count) in counts {
        let diffs = observed.keys().filter(|(f, _)| f == family).count();
        eprintln!("{family}: {count} casos, {diffs} diferencias");
    }
    for ((family, id), diff) in &observed {
        eprintln!("--- {family}/{id}: {diff}");
    }
    check_registry(&observed)
}
