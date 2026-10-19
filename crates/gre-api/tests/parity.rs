//! Comparación diferencial referencia ↔ Rust de la referencia.
//!
//! - `offline_parity`: servidor local en proceso que reproduce las respuestas
//!   de `tests/reference/gre-api/cases.json`; compara resultados, errores y
//!   solicitudes de Rust con las salidas de referencia registradas en
//!   `tests/reference/gre-api/expected.json`.
//! - `live_parity` (ignorada; la ejecuta `scripts/gre_api_parity.sh`): mismo
//!   servidor Python independiente y mismo puerto que la ejecución de referencia;
//!   compara sin normalizar el puerto e incluye la validación de contrato
//!   OpenAPI de cada solicitud.
//!
//! Normalizaciones (ver la evidencia del paquete): nombres de cabecera sin
//! mayúsculas (RFC 9110); de las cabeceras de solicitud solo se comparan
//! `Accept`, `Content-Type`, `Authorization`, `User-Agent` y
//! `Content-Length`; en offline, el puerto del servidor y las credenciales
//! ficticias se sustituyen por marcadores en ambos lados. Toda divergencia
//! debe figurar en `differences.json`. El fixture `expected.json` omite
//! además los datos del entorno del oráculo (ver `normalize_environment`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "support/runner.rs"]
mod runner;
#[path = "support/server.rs"]
mod server;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use cpeforge_gre_api::{
    ApiError, ApiToken, Configuration, CpeApi, HttpRequest, HttpResponse, HttpTransport, Method,
    ReqwestTransport, TokenRequest, TransportErrorKind,
};
use serde_json::{Value, json};

const SECRETS: [&str; 3] = [
    "secreto-ficticio",
    "clave-ficticia",
    "token-ficticio-0123456789",
];
const COMPARED_HEADERS: [&str; 5] = [
    "accept",
    "content-type",
    "authorization",
    "user-agent",
    "content-length",
];

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/gre-api")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

fn cases() -> Vec<Value> {
    read_json(&reference_dir().join("cases.json"))["cases"]
        .as_array()
        .unwrap()
        .clone()
}

#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

/// Entradas `section` de `differences.json` (`differences` o
/// `not_applicable`), validadas contra el perfil seguro.
fn registry(section: &str) -> BTreeMap<String, String> {
    read_json(&reference_dir().join("differences.json"))[section]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let id = d["id"].as_str().unwrap().to_owned();
            let checked = perfil_seguro::validate(d);
            assert!(checked.is_ok(), "{id}: {checked:?}");
            (id, d["reason"].as_str().unwrap().to_owned())
        })
        .collect()
}

/// Solicitud registrada reducida a lo comparable.
fn comparable_request(r: &Value, with_contract: bool) -> Value {
    // Orden de cabeceras no contractual: se ordenan por nombre.
    let mut headers: Vec<(String, String)> = r["headers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|h| {
            let k = h[0].as_str()?.to_ascii_lowercase();
            COMPARED_HEADERS
                .contains(&k.as_str())
                .then(|| (k, h[1].as_str().unwrap_or("").to_owned()))
        })
        .collect();
    headers.sort();
    let headers: Vec<Value> = headers.into_iter().map(|(k, v)| json!([k, v])).collect();
    let mut out =
        json!({"method": r["method"], "path": r["path"], "headers": headers, "body": r["body"]});
    if with_contract {
        out["contract"] = r["contract"].clone();
    }
    out
}

/// Caso comparable: resultado o error y solicitudes.
fn comparable(case: &Value, with_contract: bool) -> Value {
    let mut out = json!({});
    for key in ["result", "error"] {
        if let Some(v) = case.get(key) {
            out[key] = v.clone();
        }
    }
    // Las deprecaciones de la referencia/Guzzle 7 son avisos del runtime; los
    // warnings de conversión de datos sí son observables y se comparan.
    let data_warnings: Vec<Value> = case["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|w| {
            let s = w.as_str().unwrap_or("");
            !s.contains(" is deprecated") && !s.contains("Implicitly marking parameter")
        })
        .cloned()
        .collect();
    if !data_warnings.is_empty() {
        out["warnings"] = Value::Array(data_warnings);
    }
    if let Some(reqs) = case.get("requests").and_then(Value::as_array) {
        out["requests"] = Value::Array(
            reqs.iter()
                .map(|r| comparable_request(r, with_contract))
                .collect(),
        );
    }
    out
}

/// Sustituye el puerto local y las credenciales ficticias (solo offline).
fn redact(v: &mut Value) {
    match v {
        Value::String(s) => {
            for secret in SECRETS {
                *s = s.replace(secret, "<secreto>");
            }
            for host in ["127.0.0.1:", "localhost:"] {
                let mut out = String::new();
                let mut rest = s.as_str();
                while let Some(p) = rest.find(host) {
                    out.push_str(&rest[..p + host.len()]);
                    let after = &rest[p + host.len()..];
                    let digits = after.bytes().take_while(u8::is_ascii_digit).count();
                    // El puerto 1 es el de «conexión rechazada»: se conserva.
                    if digits > 1 {
                        out.push_str("<puerto>");
                    } else {
                        out.push_str(&after[..digits]);
                    }
                    rest = &after[digits..];
                }
                out.push_str(rest);
                *s = out;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(redact),
        Value::Object(m) => m.values_mut().for_each(redact),
        _ => {}
    }
}

/// Quita del fixture lo que depende del entorno que ejecuta el oráculo y no
/// del comportamiento de gre-api: versión parche de la referencia, versión de libcurl,
/// tiempos medidos por cURL y el texto de `curl_easy_strerror`, que cambió
/// entre versiones de libcurl («Couldn't» → «Could not»). El código de error
/// cURL y el resto del mensaje se conservan.
fn normalize_environment(fixture: &mut Value) {
    if let Some(reference) = fixture["reference_version"].as_str() {
        let minor: Vec<&str> = reference.split('.').take(2).collect();
        fixture["reference_version"] = Value::String(minor.join("."));
    }
    fixture["curl"] = Value::String("<entorno>".to_owned());
    normalize_curl_messages(fixture);
}

fn normalize_curl_messages(v: &mut Value) {
    match v {
        Value::String(s) if s.contains("cURL error ") => {
            *s = s.replace("Couldn't connect to server", "Could not connect to server");
            let mut out = String::new();
            let mut rest = s.as_str();
            while let Some(p) = rest.find("after ") {
                out.push_str(&rest[..p + "after ".len()]);
                let after = &rest[p + "after ".len()..];
                let digits = after.bytes().take_while(u8::is_ascii_digit).count();
                let unit = &after[digits..];
                if digits > 0 && (unit.starts_with(" ms") || unit.starts_with(" milliseconds")) {
                    out.push_str("<ms>");
                } else {
                    out.push_str(&after[..digits]);
                }
                rest = unit;
            }
            out.push_str(rest);
            *s = out;
        }
        Value::Array(a) => a.iter_mut().for_each(normalize_curl_messages),
        Value::Object(m) => m.values_mut().for_each(normalize_curl_messages),
        _ => {}
    }
}

fn first_difference(a: &Value, b: &Value, path: &str) -> String {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()).collect::<BTreeSet<_>>() {
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
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                if va != vb {
                    return first_difference(va, vb, &format!("{path}[{i}]"));
                }
            }
            String::new()
        }
        _ => {
            let cut = |v: &Value| {
                let s = v.to_string();
                s.chars().take(300).collect::<String>()
            };
            format!("{path}\n  la referencia : {}\n  Rust: {}", cut(a), cut(b))
        }
    }
}

struct Report {
    compared: usize,
    equal: usize,
    not_applicable: Vec<String>,
    differences: BTreeMap<String, String>,
}

fn check(report: &Report, label: &str) {
    let registered = registry("differences");
    let found: BTreeSet<&String> = report.differences.keys().collect();
    let expected: BTreeSet<&String> = registered.keys().collect();
    let mut msg = String::new();
    let not_applicable = registry("not_applicable");
    let found_na: BTreeSet<&String> = report.not_applicable.iter().collect();
    let expected_na: BTreeSet<&String> = not_applicable.keys().collect();
    for k in found_na.symmetric_difference(&expected_na) {
        msg.push_str(&format!("\nNO APLICABLE SIN REGISTRO U OBSOLETO {k}\n"));
    }
    for k in found.difference(&expected) {
        msg.push_str(&format!(
            "\nNO REGISTRADA {k}\n{}\n",
            report.differences[*k]
        ));
    }
    for k in expected.difference(&found) {
        msg.push_str(&format!("\nREGISTRO OBSOLETO {k}: {}\n", registered[*k]));
    }
    println!(
        "{label}: {} casos comparados, {} iguales, {} diferencias registradas, {} no aplicables a Rust",
        report.compared,
        report.equal,
        report.differences.len(),
        report.not_applicable.len()
    );
    if let Ok(path) = std::env::var("GRE_API_DIFF_REPORT") {
        let all = report
            .differences
            .iter()
            .fold(String::new(), |mut out, (k, v)| {
                use std::fmt::Write as _;
                let _ = write!(out, "== {k}\n{v}\n");
                out
            });
        fs::write(path, all).unwrap();
    }
    assert!(msg.is_empty(), "{label}:{msg}");
}

async fn compare_all(
    reference: &serde_json::Map<String, Value>,
    base: &str,
    take: impl Fn(&str) -> Vec<Value>,
    live: bool,
) -> (Report, serde_json::Map<String, Value>) {
    let mut report = Report {
        compared: 0,
        equal: 0,
        not_applicable: Vec::new(),
        differences: BTreeMap::new(),
    };
    let mut rust_out = serde_json::Map::new();
    for c in cases() {
        let id = c["id"].as_str().unwrap().to_owned();
        let Some(result) = runner::run_case(&c, base).await else {
            report.not_applicable.push(id);
            continue;
        };
        let mut out = match result {
            Ok(v) => json!({"result": v}),
            Err(e) => json!({"error": runner::error_out(&e)}),
        };
        let mock = c["config"]["host"] == "mock"
            && !matches!(c["variant"].as_str(), Some("request" | "config"));
        if mock {
            out["requests"] = Value::Array(take(&id));
        }
        rust_out.insert(id.clone(), out.clone());
        let Some(reference_case) = reference.get(&id) else {
            panic!("La referencia no ejecutó {id}")
        };
        let (mut a, mut b) = (comparable(reference_case, live), comparable(&out, live));
        if !live {
            redact(&mut a);
            redact(&mut b);
        }
        report.compared += 1;
        if a == b {
            report.equal += 1;
        } else {
            report
                .differences
                .insert(id.clone(), first_difference(&a, &b, &id));
        }
    }
    (report, rust_out)
}

#[tokio::test(flavor = "current_thread")]
async fn offline_parity() {
    let expected = read_json(&reference_dir().join("expected.json"));
    let reference = expected["results"].as_object().unwrap().clone();
    let by_id: HashMap<String, Value> = cases()
        .into_iter()
        .map(|c| (c["id"].as_str().unwrap().to_owned(), c))
        .collect();
    let srv = server::start(by_id).await;
    let base = format!("http://127.0.0.1:{}", srv.port);
    let records = srv.records.clone();
    let take = move |id: &str| records.lock().unwrap().remove(id).unwrap_or_default();
    let (report, _) = compare_all(&reference, &base, take, false).await;
    check(
        &report,
        "offline (servidor en proceso, puerto y secretos normalizados)",
    );
}

/// GET /__take del servidor Python (sin cliente HTTP adicional).
fn take_from_python(base: &str, id: &str) -> Vec<Value> {
    let addr = base.trim_start_matches("http://");
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    write!(
        stream,
        "GET /__take?case={id} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();
    let body = buf.split_once("\r\n\r\n").map_or("[]", |(_, b)| b);
    serde_json::from_str::<Vec<Value>>(body).unwrap()
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requiere el oráculo de referencia y el servidor Python; lo ejecuta scripts/gre_api_parity.sh"]
async fn live_parity() {
    let base = std::env::var("GRE_API_MOCK_URL").expect("GRE_API_MOCK_URL");
    assert!(
        base.starts_with("http://127.0.0.1:"),
        "servidor de prueba no local"
    );
    let reference_path =
        PathBuf::from(std::env::var("GRE_API_REFERENCE_OUT").expect("GRE_API_REFERENCE_OUT"));
    let reference_raw = read_json(&reference_path);
    let reference = reference_raw["results"].as_object().unwrap().clone();
    let b = base.clone();
    let (report, _) =
        compare_all(&reference, &base, move |id| take_from_python(&b, id), true).await;

    // Fixture versionado: salida de referencia con puerto y credenciales ficticias
    // redactados y sin datos del entorno (reproducible en macOS y Linux).
    let mut fixture = reference_raw.clone();
    redact(&mut fixture);
    normalize_environment(&mut fixture);
    let text = serde_json::to_string_pretty(&fixture).unwrap() + "\n";
    let path = reference_dir().join("expected.json");
    if std::env::var("GRE_API_UPDATE").is_ok_and(|v| v == "1") {
        fs::write(&path, text).unwrap();
    } else {
        assert_eq!(
            fs::read_to_string(&path).ok().as_deref(),
            Some(text.as_str()),
            "expected.json no se reproduce"
        );
    }

    // Contrato: cada solicitud de los casos dentro del contrato no debe tener
    // observaciones del validador independiente.
    let mut contract_issues = BTreeMap::new();
    for (id, case) in &reference {
        for r in case["requests"].as_array().into_iter().flatten() {
            if let Some(list) = r["contract"].as_array().filter(|l| !l.is_empty()) {
                contract_issues.insert(id.clone(), list.clone());
            }
        }
    }
    let contract_path = reference_dir().join("contract.json");
    let mut contract_text = serde_json::to_string_pretty(&contract_issues).unwrap() + "\n";
    for secret in SECRETS {
        contract_text = contract_text.replace(secret, "<secreto>");
    }
    if std::env::var("GRE_API_UPDATE").is_ok_and(|v| v == "1") {
        fs::write(&contract_path, &contract_text).unwrap();
    } else {
        assert_eq!(
            fs::read_to_string(&contract_path).ok().as_deref(),
            Some(contract_text.as_str()),
            "contract.json no se reproduce"
        );
    }
    println!(
        "validador de contrato: {} casos con observaciones (ver contract.json)",
        contract_issues.len()
    );
    for id in [
        "token-ok-sync",
        "envio-ok-sync",
        "consulta-98-sync",
        "consulta-99",
        "consulta-0",
    ] {
        assert!(
            !contract_issues.contains_key(id),
            "el caso positivo {id} incumple OpenAPI"
        );
    }
    check(&report, "live (servidor Python, mismo puerto)");
}

#[tokio::test(flavor = "current_thread")]
async fn security_limits_redirects_and_debug() {
    let id = "security-cross-host";
    let script = json!({
        "id": id,
        "response": {
            "status": 302,
            "headers": [["Location", "@other/final"]],
            "body": "",
            "final": {"status": 200, "body": "{\"codRespuesta\":\"0\"}"}
        }
    });
    let srv = server::start(HashMap::from([(id.to_owned(), script)])).await;
    let config = Configuration::cpe("token-privado-unico")
        .with_host(format!("http://127.0.0.1:{}/c/{id}", srv.port));
    let transport = ReqwestTransport::new().unwrap();
    let api = CpeApi::new(transport, config.clone());
    let result = api.consultar_envio("ticket-privado-unico").await;
    assert!(matches!(result, Err(ApiError::Http { code: 302, .. })));
    assert_eq!(srv.records.lock().unwrap().get(id).map(Vec::len), Some(1));

    let raw = HttpRequest {
        method: Method::Get,
        url: "http://example.com/secreto-en-ruta".to_owned(),
        headers: vec![(
            "Authorization".to_owned(),
            "Bearer token-privado-unico".to_owned(),
        )],
        body: Vec::new(),
    };
    let blocked = ReqwestTransport::new().unwrap().send(raw.clone()).await;
    assert!(matches!(blocked, Err(e) if e.kind == TransportErrorKind::InsecureUrl));

    let token_request = TokenRequest::password_grant(
        "cliente-privado-unico",
        "clave-privada-unica",
        "usuario-privado-unico",
        "password-privado-unico",
    );
    let response = HttpResponse {
        status: 200,
        reason: "OK".to_owned(),
        headers: vec![],
        body: b"token-privado-unico".to_vec(),
    };
    let token = ApiToken {
        access_token: Some("token-privado-unico".to_owned()),
        ..ApiToken::default()
    };
    let rendered = format!("{raw:?} {response:?} {token_request:?} {token:?} {config:?}");
    for secret in [
        "token-privado-unico",
        "secreto-en-ruta",
        "cliente-privado-unico",
        "usuario-privado-unico",
        "clave-privada-unica",
        "password-privado-unico",
    ] {
        assert!(!rendered.contains(secret), "Debug filtró {secret}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn response_size_limit() {
    let id = "security-size-limit";
    let script = json!({"id": id, "response": {"status": 200, "body": "x".repeat(128)}});
    let srv = server::start(HashMap::from([(id.to_owned(), script)])).await;
    let transport = ReqwestTransport::builder()
        .max_response_bytes(16)
        .build()
        .unwrap();
    let request = HttpRequest {
        method: Method::Get,
        url: format!("http://127.0.0.1:{}/c/{id}/x", srv.port),
        headers: vec![],
        body: vec![],
    };
    let result = transport.send(request).await;
    assert!(matches!(result, Err(e) if e.kind == TransportErrorKind::TooLarge));
}
