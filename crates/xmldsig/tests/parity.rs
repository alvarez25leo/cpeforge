//! Comparación diferencial referencia ↔ Rust de la referencia.
//!
//! - `offline_parity`: genera claves efímeras en Rust, ejecuta los casos de
//!   `tests/reference/xmldsig/cases/` y compara, tras retirar lo que depende
//!   de las claves y del azar, con las salidas de referencia redactadas de
//!   `tests/reference/xmldsig/expected/`.
//! - `live_parity` (ignorada; la ejecuta `scripts/xmldsig_parity.sh`): usa
//!   las MISMAS claves efímeras que el oráculo de referencia y compara sus salidas
//!   crudas. Solo se normaliza la aleatoriedad (GUID, `CipherValue`, clave de
//!   sesión); firmas, digest, SignedInfo canonicalizado y XML firmado se
//!   comparan byte a byte. Además comprueba que los fixtures redactados se
//!   reproducen y prepara la verificación cruzada Rust firma → la referencia verifica.
//! - `live_reference_verdicts` (ignorada): revisa los veredictos de la referencia sobre los
//!   documentos firmados por Rust.
//!
//! Toda divergencia debe figurar exactamente en `differences.json`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "support/keys.rs"]
mod keys;
#[path = "support/normalize.rs"]
mod normalize;
#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;
#[path = "support/runner.rs"]
mod runner;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use cpeforge_xmldsig::dom::Document;
use cpeforge_xmldsig::sunat::SignedXml;
use serde_json::{Map, Value, json};

const FAMILIES: [&str; 6] = [
    "basics",
    "keys",
    "signing",
    "encryption",
    "hostile",
    "encoding",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference_dir() -> PathBuf {
    root().join("tests/reference/xmldsig")
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn cases(family: &str) -> Value {
    read_json(&reference_dir().join(format!("cases/{family}.json")))
}

/// `{familia/id: motivo}` de `differences.json`.
/// Registro aplicable al la referencia comparado: una entrada con `only_when`
/// (`{"iconv_profile": …}`, `{"libxml_xpath": …}`) solo cuenta si el oráculo
/// detectó esos perfiles.
fn registry(profiles: &Profiles) -> BTreeMap<String, String> {
    let data = read_json(&reference_dir().join("differences.json"));
    data["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| {
            d["only_when"]
                .as_object()
                .into_iter()
                .flatten()
                .all(|(key, value)| profiles.get(key).map(String::as_str) == value.as_str())
        })
        .map(|d| {
            let checked = perfil_seguro::validate(d);
            assert!(checked.is_ok(), "{checked:?}");
            (
                format!(
                    "{}/{}",
                    d["family"].as_str().unwrap(),
                    d["id"].as_str().unwrap()
                ),
                d["reason"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Solo `result` o `error`; los avisos de la referencia no son salida de la biblioteca.
fn comparable(mut case: Value) -> Value {
    if let Value::Object(m) = &mut case {
        m.remove("warnings");
    }
    case
}

fn normalized(case: &Value, redact: bool) -> Value {
    let mut v = comparable(case.clone());
    normalize::normalize_random(&mut v);
    if redact {
        normalize::redact_keys(&mut v);
    }
    v
}

/// Primer punto de divergencia para el diagnóstico.
fn first_difference(a: &Value, b: &Value, path: &str) -> String {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                let (va, vb) = (
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                );
                if va != vb {
                    return first_difference(va, vb, &format!("{path}.{k}"));
                }
            }
            String::new()
        }
        (Value::String(x), Value::String(y)) => {
            let pos = x
                .bytes()
                .zip(y.bytes())
                .position(|(p, q)| p != q)
                .unwrap_or(x.len().min(y.len()));
            let ctx = |s: &str| {
                let st = floor(s, pos.saturating_sub(60));
                let en = floor(s, pos + 60);
                s[st..en].to_owned()
            };
            format!(
                "{path}: byte {pos}\n  la referencia : {:?}\n  Rust: {:?}",
                ctx(x),
                ctx(y)
            )
        }
        _ => format!(
            "{path}:\n  la referencia : {}\n  Rust: {}",
            short(a),
            short(b)
        ),
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 300 {
        format!("{}…", &s[..floor(&s, 300)])
    } else {
        s
    }
}

fn floor(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

struct Report {
    total: usize,
    equal: usize,
    errors_equal: usize,
    differences: BTreeMap<String, String>,
}

fn compare(
    family: &str,
    reference: &Map<String, Value>,
    rust: &Map<String, Value>,
    redact: bool,
    report: &mut Report,
) {
    for (id, reference_case) in reference {
        report.total += 1;
        let key = format!("{family}/{id}");
        let Some(rust_case) = rust.get(id) else {
            report
                .differences
                .insert(key, "Rust no ejecutó el caso".into());
            continue;
        };
        let (a, b) = (
            normalized(reference_case, redact),
            normalized(rust_case, redact),
        );
        if a == b {
            report.equal += 1;
            if a.get("error").is_some() {
                report.errors_equal += 1;
            }
        } else {
            report.differences.insert(key, first_difference(&a, &b, id));
        }
    }
}

/// Perfiles del entorno de la referencia que condicionan el registro (`only_when`): su
/// iconv y el comportamiento de su libxml2 (CDATA, retorno de carro, XPath),
/// detectados por el oráculo.
const PROFILE_KEYS: [&str; 4] = ["iconv_profile", "libxml_cdata", "libxml_cr", "libxml_xpath"];

/// `{clave de perfil: valor}` reunidos de las cabeceras del oráculo.
type Profiles = BTreeMap<String, String>;

fn collect_profiles(profiles: &mut Profiles, header: &Value) {
    for key in PROFILE_KEYS {
        if let Some(value) = header[key].as_str() {
            profiles.insert(key.to_owned(), value.to_owned());
        }
    }
}

/// IDs de las entradas condicionadas al perfil `key` de la referencia.
fn profile_dependent_ids(key: &str) -> BTreeSet<String> {
    read_json(&reference_dir().join("differences.json"))["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| !d["only_when"][key].is_null())
        .map(|d| d["id"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn check_registry(report: &Report, label: &str, profiles: &Profiles) {
    let registered = registry(profiles);
    let found: BTreeSet<&String> = report.differences.keys().collect();
    let expected: BTreeSet<&String> = registered.keys().collect();
    let unregistered: Vec<_> = found.difference(&expected).collect();
    let stale: Vec<_> = expected.difference(&found).collect();
    println!(
        "{label}: {} casos, {} iguales ({} errores equivalentes), {} diferencias registradas",
        report.total,
        report.equal,
        report.errors_equal,
        report.differences.len()
    );
    let mut msg = String::new();
    for k in &unregistered {
        msg.push_str(&format!(
            "\nNO REGISTRADA {k}\n{}\n",
            report.differences[**k]
        ));
    }
    for k in &stale {
        msg.push_str(&format!("\nREGISTRO OBSOLETO {k}: {}\n", registered[**k]));
    }
    if let Ok(path) = std::env::var("XMLDSIG_DIFF_REPORT") {
        let mut all = String::new();
        for (k, v) in &report.differences {
            all.push_str(&format!("== {k}\n{v}\n"));
        }
        fs::write(path, all).unwrap();
    }
    assert!(
        unregistered.is_empty() && stale.is_empty(),
        "{label}: diferencias sin registrar u obsoletas:{msg}"
    );
}

#[test]
fn offline_parity() {
    let keys = keys::generate().unwrap();
    let run = runner::Runner::new(&root(), keys.dir(), None);
    let mut report = Report {
        total: 0,
        equal: 0,
        errors_equal: 0,
        differences: BTreeMap::new(),
    };
    let mut profiles = Profiles::new();
    for family in FAMILIES {
        let expected = read_json(&reference_dir().join(format!("expected/{family}.json")));
        collect_profiles(&mut profiles, &expected);
        let mut reference = expected["results"].as_object().unwrap().clone();
        let mut rust = run.run_all(&cases(family));
        // `purposes` y `extensions` salen de la OpenSSL enlazada (propósitos
        // de su tabla, texto de X509V3_EXT_print). Si su versión difiere de
        // la que generó el fixture no son comparables aquí; live_parity los
        // compara exactos con la referencia sobre la misma OpenSSL.
        let fixture_openssl = expected["openssl"].as_str().unwrap_or_default();
        if openssl_minor(fixture_openssl) != openssl_minor(openssl::version::version()) {
            println!(
                "offline: OpenSSL enlazada «{}» ≠ fixture «{fixture_openssl}»: \
                 purposes/extensions de {family} no se comparan",
                openssl::version::version()
            );
            mask_openssl_text(&mut reference);
            mask_openssl_text(&mut rust);
        }
        compare(family, &reference, &rust, true, &mut report);
    }
    check_registry(&report, "offline (redactado)", &profiles);
}

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(
        std::env::var(name)
            .unwrap_or_else(|_| panic!("falta {name}; ejecutar scripts/xmldsig_parity.sh")),
    )
}

/// Casos de verificación cruzada: la referencia verifica el documento firmado por Rust
/// y, como control, el firmado por la referencia en el mismo caso.
/// Fixture sin metadatos del entorno y, si el versionado y el regenerado
/// vienen de versiones de OpenSSL distintas (`other_openssl`), sin
/// `purposes` ni `extensions` (ver `offline_parity`).
fn comparable_fixture(fixture: Value, other_openssl: bool) -> Value {
    let mut fixture = without_environment(fixture);
    if other_openssl {
        if let Some(results) = fixture.get_mut("results").and_then(Value::as_object_mut) {
            mask_openssl_text(results);
        }
    }
    fixture
}

/// «2.9.14» → «2.9».
fn libxml_minor(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// Primer caso y campo en que difieren el fixture versionado y el
/// regenerado, para diagnosticar la deriva desde el log del CI.
fn first_fixture_drift(committed: Option<&Value>, regenerated: &Value) -> String {
    let Some(committed) = committed else {
        return "sin fixture versionado".to_owned();
    };
    let empty = Map::new();
    let a = committed["results"].as_object().unwrap_or(&empty);
    let b = regenerated["results"].as_object().unwrap_or(&empty);
    for (id, va) in a {
        match b.get(id) {
            None => return format!("{id} solo en el versionado"),
            Some(vb) if vb != va => {
                let field = ["result", "error", "warnings"]
                    .into_iter()
                    .find(|f| va.get(*f) != vb.get(*f))
                    .unwrap_or("?");
                return format!("{id}.{field}");
            }
            Some(_) => {}
        }
    }
    if let Some(id) = b.keys().find(|id| !a.contains_key(*id)) {
        return format!("{id} solo en el regenerado");
    }
    "cabecera".to_owned()
}

/// «OpenSSL 3.6.0 1 Oct 2025» → «3.6».
fn openssl_minor(text: &str) -> String {
    text.split_whitespace()
        .nth(1)
        .map(|v| v.split('.').take(2).collect::<Vec<_>>().join("."))
        .unwrap_or_default()
}

/// Sustituye los valores que imprime OpenSSL en los casos `x509`.
fn mask_openssl_text(results: &mut Map<String, Value>) {
    for case in results.values_mut() {
        if let Some(result) = case.get_mut("result").and_then(Value::as_object_mut) {
            for field in ["purposes", "extensions"] {
                if result.get(field).is_some_and(|v| !v.is_null()) {
                    result.insert(field.to_owned(), json!("<depende-de-openssl>"));
                }
            }
        }
    }
}

/// Fixture sin los metadatos del entorno que ejecutó el oráculo (versiones
/// de la referencia, OpenSSL y libxml2), igual que `scripts/fixture_diff.py`: cambian
/// entre macOS y el CI sin que cambie ningún resultado.
fn without_environment(mut fixture: Value) -> Value {
    if let Value::Object(map) = &mut fixture {
        for field in [
            "reference_version",
            "timezone_db",
            "openssl",
            "libxml",
            "iconv_profile",
            "libxml_cdata",
            "libxml_cr",
            "libxml_xpath",
        ] {
            if map.contains_key(field) {
                map.insert(field.to_owned(), json!("<entorno>"));
            }
        }
    }
    fixture
}

/// Salida de un caso como bytes: texto UTF-8 o `{"b64": …}` (XML en otra
/// codificación).
fn output_bytes(v: &Value) -> Option<Vec<u8>> {
    use base64::Engine as _;
    match v {
        Value::String(s) => Some(s.clone().into_bytes()),
        Value::Object(m) => m
            .get("b64")
            .and_then(Value::as_str)
            .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok()),
        _ => None,
    }
}

fn cross_cases(out: &Path, run: &runner::Runner, reference: &Map<String, Value>) -> Value {
    let mut list = Vec::new();
    let families = [cases("signing"), cases("encoding")];
    for case in families
        .iter()
        .flat_map(|f| f["cases"].as_array().cloned().unwrap_or_default())
    {
        let case = &case;
        let id = case["id"].as_str().unwrap();
        let scenario = case["scenario"].as_str().unwrap();
        if !matches!(scenario, "sunat_sign" | "dsig_sign") {
            continue;
        }
        let Some(xml) = run.run_scenario(case).ok().as_ref().and_then(output_bytes) else {
            continue;
        };
        let Some(reference_xml) = reference.get(id).and_then(|o| output_bytes(&o["result"])) else {
            continue;
        };
        for (origin, content) in [
            ("rust", xml.as_slice()),
            ("reference", reference_xml.as_slice()),
        ] {
            let file = format!("{origin}-{id}.xml");
            fs::write(out.join(&file), content).unwrap();
            if scenario == "sunat_sign" {
                list.push(json!({"id": format!("{origin}-signed-{id}"), "scenario": "sunat_verify", "input": {"input_file": file}}));
            } else {
                let mut c = json!({"id": format!("{origin}-signed-{id}"), "scenario": "dsig_verify", "input": {"input_file": file}});
                // La clave para verificar: el certificado (RSA) o la clave HMAC.
                if case["key"]["alg"]
                    .as_str()
                    .unwrap_or("")
                    .ends_with("hmac-sha1")
                {
                    c["verify_with"] = case["key"]["load"].clone();
                    c["verify_is_cert"] = json!(false);
                } else {
                    c["verify_with"] = json!({"key": "a", "suffix": "crt"});
                }
                if let Some(refs) = case["references"].as_array() {
                    if refs.iter().any(|r| r["options"]["id_name"] == "ID") {
                        c["id_keys"] = json!(["ID"]);
                    }
                }
                list.push(c);
            }
        }
    }
    json!({"family": "crosscheck", "cases": list})
}

#[test]
#[ignore = "requiere el oráculo de referencia; lo ejecuta scripts/xmldsig_parity.sh"]
fn live_parity() {
    let keys_dir = env_path("XMLDSIG_KEYS_DIR");
    let live_dir = env_path("XMLDSIG_REFERENCE_OUT");
    let cross_dir = env_path("XMLDSIG_CROSS_DIR");
    let update = std::env::var("XMLDSIG_UPDATE").is_ok_and(|v| v == "1");
    let run = runner::Runner::new(&root(), &keys_dir, None);
    let mut report = Report {
        total: 0,
        equal: 0,
        errors_equal: 0,
        differences: BTreeMap::new(),
    };
    let mut fixture_drift = Vec::new();
    let mut profiles = Profiles::new();
    let mut cross_failures: Vec<String> = Vec::new();
    let mut cross_checked = 0;
    for family in FAMILIES {
        let reference_raw = read_json(&live_dir.join(format!("{family}.json")));
        collect_profiles(&mut profiles, &reference_raw);
        let reference = reference_raw["results"].as_object().unwrap().clone();
        let rust = run.run_all(&cases(family));
        compare(family, &reference, &rust, false, &mut report);

        // Fixture redactado: lo que se versiona, sin material de las claves.
        let mut fixture = reference_raw.clone();
        for (_, v) in fixture["results"].as_object_mut().unwrap().iter_mut() {
            let warnings = v.get("warnings").cloned();
            normalize::normalize_random(v);
            normalize::redact_keys(v);
            if let (Some(w), Value::Object(m)) = (warnings, &mut *v) {
                m.insert("warnings".into(), w);
            }
        }
        let path = reference_dir().join(format!("expected/{family}.json"));
        let text = serde_json::to_string_pretty(&fixture).unwrap() + "\n";
        if update {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text).unwrap();
        } else {
            let committed = fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok());
            let other_openssl = committed.as_ref().is_some_and(|c| {
                openssl_minor(c["openssl"].as_str().unwrap_or_default())
                    != openssl_minor(fixture["openssl"].as_str().unwrap_or_default())
            });
            // Fixture de otro perfil (p. ej. iconv o libxml2 de macOS frente a
            // Linux): los casos condicionados a ese perfil no son comparables.
            // Otra libxml2 (2.9 en Ubuntu frente a 2.15 en macOS): los avisos
            // de la referencia (`warnings`) llevan textos de libxml2 que cambian entre
            // versiones. No son salida de la biblioteca (la comparación live
            // también los omite), así que no se comparan entre versiones.
            let other_libxml = committed.as_ref().is_some_and(|c| {
                libxml_minor(c["libxml"].as_str().unwrap_or_default())
                    != libxml_minor(fixture["libxml"].as_str().unwrap_or_default())
            });
            let other_profiles: Vec<&str> = PROFILE_KEYS
                .into_iter()
                .filter(|key| {
                    committed
                        .as_ref()
                        .is_some_and(|c| c[*key].is_string() && c[*key] != fixture[*key])
                })
                .collect();
            let comparable = |value: Value| {
                let mut value = comparable_fixture(value, other_openssl);
                if let Some(results) = value.get_mut("results").and_then(Value::as_object_mut) {
                    for key in &other_profiles {
                        for id in profile_dependent_ids(key) {
                            results.remove(&id);
                        }
                    }
                    if other_libxml {
                        for case in results.values_mut() {
                            if let Value::Object(case) = case {
                                case.remove("warnings");
                            }
                        }
                    }
                }
                value
            };
            let committed = committed.map(comparable);
            let regenerated = comparable(fixture.clone());
            if committed.as_ref() != Some(&regenerated) {
                fixture_drift.push(format!(
                    "{family}: {}",
                    first_fixture_drift(committed.as_ref(), &regenerated)
                ));
            }
        }

        // La referencia firma → Rust verifica, con el certificado "a" configurado.
        // Incluye los documentos ISO-8859-1/windows-1252 de `encoding`.
        if family == "signing" || family == "encoding" {
            let cert = String::from_utf8(fs::read(keys_dir.join("a.pem")).unwrap()).unwrap();
            for (id, out) in &reference {
                let Some(xml) = output_bytes(&out["result"]) else {
                    continue;
                };
                if !id.starts_with("sunat-sign-") && !id.starts_with("encoding-sunat-sign-") {
                    continue;
                }
                // Salidas de la referencia sin firma (p. ej. XML mal formado) no aplican.
                let Ok(mut doc) = Document::parse_bytes(&xml) else {
                    continue;
                };
                if !xml.windows(14).any(|w| w == b"SignatureValue") {
                    continue;
                }
                cross_checked += 1;
                let mut verifier = SignedXml::new();
                verifier.set_certificate(&cert);
                let ok = verifier.verify(&mut doc);
                if !matches!(ok, Ok(true)) {
                    cross_failures.push(format!(
                        "Rust no verifica la firma de la referencia {id}: {ok:?}"
                    ));
                }
                let tampered = match xml.windows(9).position(|w| w == b"</cbc:ID>") {
                    Some(at) => [&xml[..at], b"X", &xml[at..]].concat(),
                    None => xml.clone(),
                };
                if tampered != xml {
                    let mut doc = Document::parse_bytes(&tampered).unwrap();
                    let mut v = SignedXml::new();
                    v.set_certificate(&cert);
                    if !matches!(v.verify(&mut doc), Ok(false)) {
                        cross_failures.push(format!(
                            "Rust acepta la firma de la referencia alterada {id}"
                        ));
                    }
                }
            }
        }
    }
    fs::create_dir_all(cross_dir.join("rust-signed")).unwrap();
    let mut reference_signing = read_json(&live_dir.join("signing.json"))["results"]
        .as_object()
        .unwrap()
        .clone();
    reference_signing.extend(
        read_json(&live_dir.join("encoding.json"))["results"]
            .as_object()
            .cloned()
            .unwrap_or_default(),
    );
    let cross = cross_cases(&cross_dir.join("rust-signed"), &run, &reference_signing);
    fs::write(
        cross_dir.join("cases.json"),
        serde_json::to_string_pretty(&cross).unwrap(),
    )
    .unwrap();
    check_registry(&report, "live (mismas claves, sin redactar)", &profiles);
    assert!(
        fixture_drift.is_empty(),
        "fixtures redactados no reproducidos: {fixture_drift:?}"
    );
    println!("Rust verificó {cross_checked} documentos firmados por la referencia");
    assert!(cross_failures.is_empty(), "{cross_failures:#?}");
}

#[test]
#[ignore = "requiere el oráculo de referencia; lo ejecuta scripts/xmldsig_parity.sh"]
fn live_reference_verdicts() {
    let cross_dir = env_path("XMLDSIG_CROSS_DIR");
    let verdicts = read_json(&cross_dir.join("reference-verdicts.json"));
    let results = verdicts["results"].as_object().unwrap();
    let verdict = |out: &Value| {
        let r = &out["result"];
        if let Some(v) = r.get("result") {
            v == &json!(true)
        } else {
            (r["verify"] == json!(1) || r["verify"] == json!(true))
                && r["references"] == json!(true)
        }
    };
    let mut checked = 0;
    let mut valid = 0;
    let mut mismatches = Vec::new();
    for (id, out) in results {
        let Some(original) = id.strip_prefix("rust-signed-") else {
            continue;
        };
        let reference_out = &results[&format!("reference-signed-{original}")];
        let (rust_ok, reference_ok) = (verdict(out), verdict(reference_out));
        if rust_ok != reference_ok {
            mismatches.push(format!(
                "{original}: la referencia verifica firma Rust={rust_ok}, firma de la referencia={reference_ok}"
            ));
        }
        checked += 1;
        valid += usize::from(rust_ok);
    }
    println!(
        "La referencia verificó {checked} documentos firmados por Rust ({valid} válidos, mismo veredicto que la firma de la referencia)"
    );
    assert!(mismatches.is_empty(), "{mismatches:#?}");

    assert!(checked > 30);
}

/// `XMLDSIG_WRITE_KEYS=<dir> cargo test -p cpeforge-xmldsig --test parity
/// write_keys -- --ignored`: material efímero equivalente al de
/// `scripts/xmldsig_keys.sh`, para sistemas con OpenSSL < 3.4.
#[test]
#[ignore = "lo invoca scripts/xmldsig_keys.sh"]
fn write_keys() {
    let dir = env_path("XMLDSIG_WRITE_KEYS");
    fs::create_dir_all(&dir).unwrap();
    keys::fill(&dir).unwrap();
}
