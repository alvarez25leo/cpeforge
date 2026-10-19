//! Comparación diferencial referencia ↔ Rust de la referencia.
//!
//! Ejecuta en Rust los casos de `tests/reference/xml/cases/` y compara con el
//! XML que generó la referencia (`tests/reference/xml/expected/`).
//!
//! - XML: igualdad **de bytes**. Si difieren, el diagnóstico muestra el primer
//!   byte distinto y el primer nodo distinto del árbol (`{namespace}local`,
//!   atributos y texto).
//! - Errores: correspondencia explícita por clase y contenido. Del mensaje de
//!   Twig se comparan plantilla, atributo y causa; la línea de la plantilla
//!   (`at line N`) no se compara porque Rust no ejecuta las plantillas Twig.
//! - Avisos de deprecación de la referencia: se registran en el fixture pero no se
//!   comparan; son diagnósticos del intérprete, no salida del builder.
//!
//! Toda divergencia debe figurar exactamente en `differences.json`.

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

use cpeforge_core::CoreError;
use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{Invoice, Note};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_xml::{
    Autoescape, DespatchBuilder, InvoiceBuilder, NoteBuilder, PerceptionBuilder, RenderOptions,
    RetentionBuilder, SummaryBuilder, VoidedBuilder, XmlError,
};
use hydrate::{HydrateError, TestResult, deserialize, harness_error, to_serde};
use jiff::Timestamp;
use serde_json::{Value, json};

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/xml")
}

fn read_json(path: &Path) -> TestResult<Value> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

/// Opciones Rust equivalentes a las opciones Twig del caso.
fn options(case: &Value, now: i64) -> TestResult<RenderOptions> {
    let opts = case["options"].as_object().ok_or("options no es objeto")?;
    let mut out = RenderOptions {
        now: Some(Timestamp::from_second(now)?),
        ..RenderOptions::default()
    };
    for (key, value) in opts {
        match (key.as_str(), value) {
            ("autoescape", Value::Bool(false)) => out.autoescape = Autoescape::Disabled,
            ("autoescape", Value::String(s)) if s == "html" => out.autoescape = Autoescape::Html,
            ("strict_variables", Value::Bool(strict)) => out.strict_variables = *strict,
            // `cache` solo decide dónde compila Twig; no cambia la salida.
            ("cache", Value::Bool(false)) => {}
            _ => {
                return Err(harness_error(format!(
                    "opción sin equivalente Rust: {key}={value}"
                )));
            }
        }
    }
    Ok(out)
}

fn render(
    builder: &str,
    class: &str,
    value: Value,
    options: RenderOptions,
) -> TestResult<Result<String, Outcome>> {
    macro_rules! go {
        ($ty:ty, $builder:expr, $method:ident) => {
            match deserialize::<$ty>(value) {
                Ok(doc) => Ok($builder.$method(&doc).map_err(Outcome::Xml)),
                Err(err) => Ok(Err(Outcome::Hydrate(err))),
            }
        };
    }
    match (builder, class) {
        ("InvoiceBuilder", "Invoice") => go!(Invoice, InvoiceBuilder::new(options), render),
        ("NoteBuilder", "Note") => go!(Note, NoteBuilder::new(options), render),
        ("DespatchBuilder", "Despatch") => go!(Despatch, DespatchBuilder::new(options), render),
        ("SummaryBuilder", "Summary") => go!(Summary, SummaryBuilder::new(options), render),
        ("VoidedBuilder", "Voided") => go!(Voided, VoidedBuilder::new(options), render),
        ("VoidedBuilder", "Reversion") => {
            go!(Reversion, VoidedBuilder::new(options), render_reversion)
        }
        ("PerceptionBuilder", "Perception") => {
            go!(Perception, PerceptionBuilder::new(options), render)
        }
        ("RetentionBuilder", "Retention") => go!(Retention, RetentionBuilder::new(options), render),
        _ => Err(harness_error(format!(
            "combinación sin equivalente Rust: {builder} con {class}"
        ))),
    }
}

#[derive(Debug)]
enum Outcome {
    Xml(XmlError),
    /// El modelo no se pudo construir desde el caso (se informa con `Debug`).
    Hydrate(#[allow(dead_code, reason = "solo se muestra en el diagnóstico")] HydrateError),
}

/// Correspondencia entre el error de la referencia y el error Rust.
fn errors_match(reference: &Value, rust: &Outcome) -> bool {
    let class = reference["class"].as_str().unwrap_or_default();
    let message = reference["message"].as_str().unwrap_or_default();
    let Outcome::Xml(rust) = rust else {
        return false;
    };
    match rust {
        XmlError::TemplateNotFound { template } => {
            class == "Twig\\Error\\LoaderError" && message.starts_with(&format!("Unable to find template \"{template}\" "))
        }
        XmlError::NullAttribute { template, attribute } => {
            class == "Twig\\Error\\RuntimeError"
                && message.starts_with(&format!(
                    "Impossible to access an attribute (\"{attribute}\") on a null variable in \"{template}\" at line "
                ))
        }
        XmlError::Document { template, source: CoreError::MissingField { field, .. } } => {
            let previous = reference["previous"]["message"].as_str().unwrap_or_default();
            let expected_field = if previous.ends_with("AdditionalDoc::getNro(): Return value must be of type string, null returned") {
                "nro"
            } else if previous.contains("Summary::getDateWithTimezone(): Argument #1 ($date) must be of type DateTimeInterface, null given") {
                "fecResumen"
            } else if previous.contains("Voided::getDateWithTimezone(): Argument #1 ($date) must be of type DateTimeInterface, null given") {
                "fecComunicacion"
            } else {
                return false;
            };
            class == "Twig\\Error\\RuntimeError"
                && reference["previous"]["class"] == "TypeError"
                && message.starts_with("An exception has been thrown during the rendering of a template (")
                && message.contains(&format!(") in \"{template}\" at line "))
                && *field == expected_field
        }
        _ => false,
    }
}

/// Primer punto de divergencia entre dos XML: byte y nodo.
fn describe_difference(reference: &str, rust: &str) -> String {
    let offset = reference
        .bytes()
        .zip(rust.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(reference.len().min(rust.len()));
    let context = |text: &str| {
        let mut start = offset.saturating_sub(60).min(text.len());
        while !text.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = (offset + 60).min(text.len());
        while !text.is_char_boundary(end) {
            end += 1;
        }
        text[start..end].to_owned()
    };
    let tree = match (
        roxmltree::Document::parse(reference),
        roxmltree::Document::parse(rust),
    ) {
        (Ok(a), Ok(b)) => first_node_difference(a.root_element(), b.root_element(), String::new())
            .unwrap_or_else(|| {
                "árboles iguales (diferencia solo en bytes: prefijos, declaración o escape)".into()
            }),
        (Err(err), Ok(_)) => format!("La referencia no es XML bien formado ({err})"),
        (Ok(_), Err(err)) => format!("Rust no es XML bien formado ({err})"),
        (Err(_), Err(_)) => "ninguno es XML bien formado".into(),
    };
    format!(
        "byte {offset}\n  la referencia:  …{}…\n  Rust: …{}…\n  árbol: {tree}",
        context(reference),
        context(rust)
    )
}

fn expanded(node: roxmltree::Node<'_, '_>) -> String {
    let name = node.tag_name();
    format!(
        "{{{}}}{}",
        name.namespace().unwrap_or_default(),
        name.name()
    )
}

fn first_node_difference(
    a: roxmltree::Node<'_, '_>,
    b: roxmltree::Node<'_, '_>,
    path: String,
) -> Option<String> {
    let path = format!("{path}/{}", expanded(a));
    if expanded(a) != expanded(b) {
        return Some(format!("{path}: nombre {} ≠ {}", expanded(a), expanded(b)));
    }
    let attrs = |n: roxmltree::Node<'_, '_>| -> BTreeMap<String, String> {
        n.attributes()
            .map(|at| {
                (
                    format!("{{{}}}{}", at.namespace().unwrap_or_default(), at.name()),
                    at.value().to_owned(),
                )
            })
            .collect()
    };
    if attrs(a) != attrs(b) {
        return Some(format!("{path}: atributos {:?} ≠ {:?}", attrs(a), attrs(b)));
    }
    let ca: Vec<_> = a
        .children()
        .filter(|c| c.is_element() || c.is_text())
        .collect();
    let cb: Vec<_> = b
        .children()
        .filter(|c| c.is_element() || c.is_text())
        .collect();
    for (x, y) in ca.iter().zip(cb.iter()) {
        match (x.is_element(), y.is_element()) {
            (true, true) => {
                if let Some(diff) = first_node_difference(*x, *y, path.clone()) {
                    return Some(diff);
                }
            }
            (false, false) if x.text() != y.text() => {
                return Some(format!("{path}: texto {:?} ≠ {:?}", x.text(), y.text()));
            }
            (false, false) => {}
            _ => return Some(format!("{path}: tipo de hijo distinto")),
        }
    }
    (ca.len() != cb.len()).then(|| format!("{path}: {} hijos ≠ {}", ca.len(), cb.len()))
}

/// Plantilla referencia que produjo un XML, deducida de la raíz y las versiones.
fn template_of(xml: &str) -> Option<&'static str> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();
    let child_text = |local: &str| {
        root.children()
            .find(|c| c.tag_name().name() == local)
            .and_then(|c| c.text())
            .unwrap_or_default()
    };
    Some(
        match (
            root.tag_name().name(),
            child_text("UBLVersionID"),
            child_text("CustomizationID"),
        ) {
            ("Invoice", "2.0", _) => "invoice2.0.xml.twig",
            ("Invoice", "2.1", _) => "invoice2.1.xml.twig",
            ("CreditNote", "2.0", _) => "notacr2.0.xml.twig",
            ("CreditNote", "2.1", _) => "notacr2.1.xml.twig",
            ("DebitNote", "2.0", _) => "notadb2.0.xml.twig",
            ("DebitNote", "2.1", _) => "notadb2.1.xml.twig",
            ("DespatchAdvice", _, "1.0") => "despatch.xml.twig",
            ("DespatchAdvice", _, "2.0") => "despatch2022.xml.twig",
            ("SummaryDocuments", _, _) => "summary.xml.twig",
            ("VoidedDocuments", _, _) => "voided.xml.twig",
            ("Retention", _, _) => "retention.xml.twig",
            ("Perception", _, _) => "perception.xml.twig",
            _ => return None,
        },
    )
}

#[test]
fn reference_and_rust_xml_match() -> TestResult {
    let dir = reference_dir();
    let reference = read_json(&dir.join("../core/inventory.json"))?["reference"].clone();
    let registered = read_json(&dir.join("differences.json"))?;
    let registered: Vec<(String, String)> = registered["differences"]
        .as_array()
        .ok_or("differences.json sin lista")?
        .iter()
        .map(|d| {
            for field in ["case", "kind"] {
                assert!(
                    d[field].as_str().is_some_and(|s| !s.is_empty()),
                    "diferencia sin {field}: {d}"
                );
            }
            let checked = perfil_seguro::validate(d);
            assert!(checked.is_ok(), "{checked:?}");
            (
                d["case"].as_str().unwrap_or_default().to_owned(),
                d["kind"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();

    let mut found: Vec<(String, String, String)> = Vec::new();
    let mut templates: BTreeSet<&'static str> = BTreeSet::new();
    let mut embedded = false;
    let mut error_kinds: BTreeSet<&'static str> = BTreeSet::new();
    let mut total = 0;
    let mut files: Vec<PathBuf> = fs::read_dir(dir.join("cases"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no hay casos");
    for file in files {
        let family = file
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("nombre inválido")?
            .to_owned();
        let cases = read_json(&file)?;
        let expected = read_json(&dir.join("expected").join(format!("{family}.json")))?;
        assert_eq!(
            expected["reference"]["commit"], reference["baseline_commit"],
            "{family}: referencia distinta"
        );
        let results = expected["results"].as_object().ok_or("sin resultados")?;
        let list = cases["cases"].as_array().ok_or("sin casos")?;
        assert_eq!(
            results.len(),
            list.len(),
            "{family}: casos y salidas de referencia no coinciden"
        );
        for case in list {
            total += 1;
            let id = case["id"].as_str().ok_or("caso sin id")?;
            let reference = &results[id];
            let now = reference["now"].as_i64().ok_or("sin now")?;
            let class = case["input"]["$class"].as_str().ok_or("sin $class")?;
            let builder = case["builder"].as_str().ok_or("sin builder")?;
            let value = match to_serde(&case["input"])? {
                Ok(value) => value,
                Err(err) => return Err(harness_error(format!("{id}: no se pudo hidratar: {err}"))),
            };
            let rust = render(builder, class, value, options(case, now)?)?;
            let reference_xml = if reference.get("sha256").is_some() {
                Some(fs::read_to_string(
                    dir.join("expected").join(&family).join(format!("{id}.xml")),
                )?)
            } else {
                None
            };
            match (reference_xml, reference.get("error"), rust) {
                (Some(reference_xml), _, Ok(rust_xml)) => {
                    if reference_xml == rust_xml {
                        if let Some(t) = template_of(&reference_xml) {
                            templates.insert(t);
                        }
                        embedded |= reference_xml.contains("<sac:SUNATEmbededDespatchAdvice>");
                    } else {
                        found.push((
                            id.to_owned(),
                            "xml".into(),
                            describe_difference(&reference_xml, &rust_xml),
                        ));
                    }
                }
                (None, Some(reference_err), Err(rust_err)) => {
                    if errors_match(reference_err, &rust_err) {
                        if let Outcome::Xml(e) = &rust_err {
                            error_kinds.insert(match e {
                                XmlError::TemplateNotFound { .. } => "TemplateNotFound",
                                XmlError::NullAttribute { .. } => "NullAttribute",
                                XmlError::Document { .. } => "Document",
                                _ => "otro",
                            });
                        }
                    } else {
                        found.push((
                            id.to_owned(),
                            "error".into(),
                            format!("La referencia {reference_err}\n  Rust {rust_err:?}"),
                        ));
                    }
                }
                (Some(_), _, Err(rust_err)) => found.push((
                    id.to_owned(),
                    "error".into(),
                    format!("La referencia generó XML; Rust {rust_err:?}"),
                )),
                (None, Some(reference_err), Ok(_)) => found.push((
                    id.to_owned(),
                    "error".into(),
                    format!("La referencia {reference_err}; Rust generó XML"),
                )),
                (None, None, _) => {
                    return Err(harness_error(format!(
                        "{id}: salida de referencia sin XML ni error"
                    )));
                }
            }
        }
    }

    let mut problems = Vec::new();
    for (case, kind, detail) in &found {
        if !registered.iter().any(|(c, k)| c == case && k == kind) {
            problems.push(format!(
                "DIVERGENCIA NO REGISTRADA {case} [{kind}]\n  {detail}"
            ));
        }
    }
    for (case, kind) in &registered {
        if !found.iter().any(|(c, k, _)| c == case && k == kind) {
            problems.push(format!("REGISTRO OBSOLETO {case} [{kind}]"));
        }
    }
    let all_templates = [
        "invoice2.0.xml.twig",
        "invoice2.1.xml.twig",
        "notacr2.0.xml.twig",
        "notacr2.1.xml.twig",
        "notadb2.0.xml.twig",
        "notadb2.1.xml.twig",
        "despatch.xml.twig",
        "despatch2022.xml.twig",
        "summary.xml.twig",
        "voided.xml.twig",
        "retention.xml.twig",
        "perception.xml.twig",
    ];
    for template in all_templates {
        if !templates.contains(template) {
            problems.push(format!("PLANTILLA SIN XML IGUAL: {template}"));
        }
    }
    if !embedded {
        problems.push("PLANTILLA SIN XML IGUAL: embededDespatch.xml.twig".into());
    }
    for kind in ["TemplateNotFound", "NullAttribute", "Document"] {
        if !error_kinds.contains(kind) {
            problems.push(format!("ERROR SIN CASO: {kind}"));
        }
    }
    println!("casos: {total}; divergencias registradas: {}", found.len());
    for (case, kind, detail) in &found {
        println!(
            "  {case} [{kind}] {}",
            detail.lines().next().unwrap_or_default()
        );
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(harness_error(problems.join("\n")))
    }
}

/// Filtros y funciones comparados con el oráculo externo (`xml`).
#[test]
fn filters_match_reference() -> TestResult {
    use cpeforge_xml::format::{
        float_string, n_format, n_format_limit, tributo, tributo_by_afectacion,
    };
    let expected = read_json(&reference_dir().join("filters/expected.json"))?;
    let registered = read_json(&reference_dir().join("differences.json"))?;
    let registered: BTreeSet<String> = registered["filter_differences"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            let checked = perfil_seguro::validate(d);
            assert!(checked.is_ok(), "{checked:?}");
            d["input"].to_string()
        })
        .collect();
    let mut mismatches = Vec::new();
    let mut total = 0;
    for result in expected["results"].as_array().ok_or("sin resultados")? {
        total += 1;
        let input = &result["input"];
        let value = &input["value"];
        let decimal = || -> TestResult<Option<cpeforge_core::Decimal>> {
            match to_serde(value)? {
                Ok(Value::Null) => Ok(None),
                Ok(Value::String(text)) => {
                    // Igual que el adaptador serde del crate core: conserva el signo de `-0`.
                    let mut value: cpeforge_core::Decimal = text.parse()?;
                    if value.is_zero() && text.starts_with('-') {
                        value.set_sign_negative(true);
                    }
                    Ok(Some(value))
                }
                other => Err(harness_error(format!("valor inesperado {other:?}"))),
            }
        };
        let decimals = input["decimals"]
            .as_u64()
            .map(|d| u32::try_from(d).unwrap());
        let rust = match input["fn"].as_str().unwrap_or_default() {
            "print" => json!(decimal()?.map(float_string).unwrap_or_default()),
            "n_format" => json!(n_format(decimal()?, decimals.unwrap_or(2))),
            "n_format_limit" => {
                let out = n_format_limit(decimal()?, decimals.unwrap_or_default());
                if decimal()?.is_none() {
                    Value::Null
                } else {
                    json!(out)
                }
            }
            "tributo_afectacion" => {
                let t = tributo_by_afectacion(value.as_str());
                json!({"id": t.id, "code": t.code, "name": t.name})
            }
            "tributo" => tributo(value.as_str()).map_or(
                Value::Null,
                |t| json!({"id": t.id, "code": t.code, "name": t.name}),
            ),
            other => return Err(harness_error(format!("función desconocida {other}"))),
        };
        let reference = &result["output"];
        let differs = reference != &rust;
        let is_registered = registered.contains(&input.to_string());
        if differs && !is_registered {
            mismatches.push(format!(
                "NO REGISTRADA {input}: la referencia {reference} | Rust {rust}"
            ));
        } else if !differs && is_registered {
            mismatches.push(format!("REGISTRO OBSOLETO {input}"));
        }
    }
    println!(
        "filtros: {total}; diferencias registradas: {}",
        registered.len()
    );
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(harness_error(mismatches.join("\n")))
    }
}
