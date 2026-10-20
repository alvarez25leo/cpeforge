//! Oráculo vivo referencia ↔ Rust. Ejecutar solo mediante `scripts/cpeforge_parity.sh`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "el arnés aborta al detectar divergencias"
)]

#[path = "../../core/tests/support/hydrate.rs"]
mod hydrate;

use std::error::Error;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use cpeforge::resolver::{XmlBuilder, XmlBuilderResolver};
use cpeforge::{
    Api, BuilderOptionsPatch, DocumentKind, DocumentRef, ErrorCodeProvider, See, SendResult,
};
use cpeforge_core::Document;
use cpeforge_core::model::sale::Invoice;
use cpeforge_core::model::summary::Summary;
use cpeforge_gre_api::{
    Configuration, HttpRequest, HttpResponse, HttpTransport, Method, TransportError,
};
use cpeforge_ws::api::{ApiFactory, InMemoryStore, SystemClock};
use cpeforge_ws::zip;
use cpeforge_xml::{Autoescape, RenderOptions};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Default)]
struct OracleTransport(Arc<Mutex<Vec<HttpRequest>>>, Arc<AtomicBool>);

impl HttpTransport for OracleTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url.ends_with("/oauth2/token/") {
            br#"{"access_token":"fake-token","expires_in":3600}"#.to_vec()
        } else if request.method == Method::Get {
            br#"{"codRespuesta":"98"}"#.to_vec()
        } else if request.url.contains("/contribuyente/gem/comprobantes/") {
            if self.1.load(Ordering::SeqCst) {
                br#"{"cod":"5000","msg":"Fallo ficticio"}"#.to_vec()
            } else {
                br#"{"numTicket":"T-123"}"#.to_vec()
            }
        } else if request
            .body
            .windows(b"getStatus".len())
            .any(|w| w == b"getStatus")
        {
            br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><getStatusResponse xmlns="http://service.sunat.gob.pe"><status><statusCode>98</statusCode></status></getStatusResponse></soap:Body></soap:Envelope>"#.to_vec()
        } else {
            br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><sendSummaryResponse xmlns="http://service.sunat.gob.pe"><ticket>T-123</ticket></sendSummaryResponse></soap:Body></soap:Envelope>"#.to_vec()
        };
        let status = if self.1.load(Ordering::SeqCst)
            && request.url.contains("/contribuyente/gem/comprobantes/")
        {
            500
        } else {
            200
        };
        self.0.lock().unwrap().push(request);
        Ok(HttpResponse {
            status,
            reason: "OK".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body,
        })
    }
}

#[derive(Clone)]
struct FaultTransport;
impl HttpTransport for FaultTransport {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        // Como SUNAT: con `Content-Type` XML, ext/soap lee el fault aunque sea un 500.
        Ok(HttpResponse { status: 500, reason: "Fault".into(), headers: vec![("Content-Type".into(), "text/xml; charset=utf-8".into())], body: br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><soap:Fault><faultcode>soap:Client.0100</faultcode><faultstring>Original</faultstring></soap:Fault></soap:Body></soap:Envelope>"#.to_vec() })
    }
}
struct Codes;
impl ErrorCodeProvider for Codes {
    fn get_value(&self, code: &str) -> Option<String> {
        (code == "0100").then(|| "Mensaje ficticio".to_owned())
    }
}

fn case(family: &str, id: &str) -> Result<Value, Box<dyn Error>> {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{root}/../../tests/reference/xml/cases/{family}.json");
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    value["cases"]
        .as_array()
        .ok_or("casos ausentes")?
        .iter()
        .find(|c| c["id"] == id)
        .cloned()
        .ok_or_else(|| "caso ausente".into())
}

fn doc<T: serde::de::DeserializeOwned>(family: &str, id: &str) -> Result<T, Box<dyn Error>> {
    let node = case(family, id)?;
    let value = hydrate::to_serde(&node["input"])?.map_err(|e| format!("{e}"))?;
    hydrate::deserialize(value).map_err(|e| format!("{e}").into())
}

fn soap_content(request: &HttpRequest) -> Result<(String, Vec<u8>), Box<dyn Error>> {
    let xml = std::str::from_utf8(&request.body)?;
    let name = xml
        .split("<fileName>")
        .nth(1)
        .and_then(|s| s.split("</fileName>").next())
        .ok_or("fileName ausente")?
        .to_owned();
    let encoded = xml
        .split("<contentFile>")
        .nth(1)
        .and_then(|s| s.split("</contentFile>").next())
        .ok_or("contentFile ausente")?;
    let zip = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    Ok((name, zip))
}

fn gre_content(request: &HttpRequest) -> Result<(String, Vec<u8>, String), Box<dyn Error>> {
    let data: Value = serde_json::from_slice(&request.body)?;
    let archive = &data["archivo"];
    let name = archive["nomArchivo"]
        .as_str()
        .ok_or("nomArchivo ausente")?
        .to_owned();
    let hash = archive["hashZip"]
        .as_str()
        .ok_or("hashZip ausente")?
        .to_owned();
    let zip = base64::engine::general_purpose::STANDARD
        .decode(archive["arcGreZip"].as_str().ok_or("arcGreZip ausente")?)?;
    Ok((name, zip, hash))
}

fn sha256(input: &[u8]) -> String {
    Sha256::digest(input)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// ZIP con la hora/fecha MS-DOS a cero, igual que `zip_hash_without_dos_time`
/// del oráculo de referencia: nelexa/zip escribe `time()` en esos cuatro bytes.
fn without_dos_time(zip: &[u8]) -> Vec<u8> {
    let mut out = zip.to_vec();
    let central = zip
        .len()
        .checked_sub(6)
        .and_then(|at| zip.get(at..at + 4))
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(|b| u32::from_le_bytes(b) as usize);
    for at in [Some(10), central.map(|c| c + 12)].into_iter().flatten() {
        if let Some(field) = out.get_mut(at..at + 4) {
            field.fill(0);
        }
    }
    out
}

fn compare(expected: &Value, differences: &mut Vec<String>, id: &str, actual: Value) {
    if expected[id] != actual {
        differences.push(id.to_owned());
    }
}

#[tokio::test]
#[ignore = "requiere oráculo de referencia y PEM efímero; usar scripts/cpeforge_parity.sh"]
async fn live_reference_parity() -> Result<(), Box<dyn Error>> {
    let expected: Value = serde_json::from_str(&fs::read_to_string(std::env::var(
        "CPEFORGE_REFERENCE_OUT",
    )?)?)?;
    let pem = fs::read_to_string(format!("{}/a.pem", std::env::var("CPEFORGE_KEYS_DIR")?))?;
    let invoice: Invoice = doc("invoice", "inv21-full-cpeforge")?;
    let summary: Summary = doc("summary", "summary-full-cpeforge")?;
    let mut differences = Vec::<String>::new();
    compare(
        &expected,
        &mut differences,
        "invoice_pre_sign",
        json!(
            XmlBuilderResolver::new(RenderOptions::cpeforge())
                .find(DocumentKind::Invoice)
                .build(DocumentRef::Invoice(&invoice))?
        ),
    );
    compare(
        &expected,
        &mut differences,
        "summary_pre_sign",
        json!(
            XmlBuilderResolver::new(RenderOptions::cpeforge())
                .find(DocumentKind::Summary)
                .build(DocumentRef::Summary(&summary))?
        ),
    );

    let soap_transport = OracleTransport::default();
    let mut see = See::new(soap_transport.clone());
    see.set_service("http://127.0.0.1:9/soap")
        .set_certificate(&pem)?;
    compare(
        &expected,
        &mut differences,
        "see_xml_before",
        json!(see.factory_ref().last_xml()),
    );
    let signed_summary = see
        .get_xml_signed(DocumentRef::Summary(&summary))?
        .to_owned();
    compare(
        &expected,
        &mut differences,
        "see_signed_summary",
        json!(signed_summary),
    );
    compare(
        &expected,
        &mut differences,
        "see_xml_after_signed",
        json!(see.factory_ref().last_xml()),
    );
    let raw = see
        .send_xml(DocumentKind::Summary, &summary.name()?, b"<caller-xml/>")
        .await?;
    compare(&expected, &mut differences, "see_send_xml", result(raw));
    compare(
        &expected,
        &mut differences,
        "see_xml_after_raw",
        json!(see.factory_ref().last_xml()),
    );
    let file = see.send_xml_file(&signed_summary).await?;
    compare(
        &expected,
        &mut differences,
        "see_send_xml_file",
        result(file),
    );
    compare(
        &expected,
        &mut differences,
        "see_xml_after_file",
        json!(see.factory_ref().last_xml()),
    );
    let sent = see.send(DocumentRef::Summary(&summary)).await?;
    compare(&expected, &mut differences, "see_send", result(sent));
    compare(
        &expected,
        &mut differences,
        "see_xml_after_send",
        json!(see.factory_ref().last_xml()),
    );
    let status = see.get_status("T-123").await?;
    compare(
        &expected,
        &mut differences,
        "see_status",
        json!({"success":status.bill.base.success,"ticket":null,"error":status.bill.base.error.as_ref().map(|e| json!({"code":e.code,"message":e.message}))}),
    );
    let soap_calls = soap_transport.0.lock().unwrap().clone();
    let reference_calls = expected["see_calls"]
        .as_array()
        .ok_or("see_calls ausente")?;
    for (index, call) in soap_calls.iter().take(3).enumerate() {
        let (name, raw_zip) = soap_content(call)?;
        let entries = zip::decompress(&raw_zip)?;
        if call.method != Method::Post
            || call.header("SOAPAction") != Some("\"urn:sendSummary\"")
            || reference_calls[index]["operation"] != "sendSummary"
        {
            differences.push(format!("see_call_{index}_operation"));
        }
        if json!(name) != reference_calls[index]["params"]["fileName"] {
            differences.push(format!("see_call_{index}_name"));
        }
        if json!(sha256(&without_dos_time(&raw_zip)))
            != reference_calls[index]["params"]["zip_hash_without_dos_time"]
        {
            differences.push(format!("see_call_{index}_zip_hash"));
        }
        if json!(raw_zip.len()) != reference_calls[index]["params"]["zip_bytes"] {
            differences.push(format!("see_call_{index}_zip_bytes"));
        }
        if json!(
            entries
                .first()
                .map(|e| String::from_utf8_lossy(&e.content).into_owned())
        ) != reference_calls[index]["params"]["contentFile"][0]["content"]
        {
            differences.push(format!("see_call_{index}_xml"));
        }
    }
    if let Some(status_request) = soap_calls.get(3) {
        let body = String::from_utf8_lossy(&status_request.body);
        if status_request.header("SOAPAction") != Some("\"urn:getStatus\"")
            || !body.contains("<ticket>T-123</ticket>")
            || reference_calls[3]["operation"] != "getStatus"
            || reference_calls[3]["params"]["ticket"] != "T-123"
        {
            differences.push("see_status_request".into());
        }
    } else {
        differences.push("see_status_request_missing".into());
    }

    let mut fault_see = See::new(FaultTransport);
    fault_see.set_code_provider(Some(Arc::new(Codes)));
    compare(
        &expected,
        &mut differences,
        "see_fault",
        result(
            fault_see
                .send_xml(DocumentKind::Invoice, "fake", b"<raw/>")
                .await?,
        ),
    );
    let unknown = DocumentKind::from_class_name("Example\\Unknown")
        .err()
        .ok_or("se esperaba error de builder desconocido")?;
    if json!(unknown.to_string()) != expected["see_unknown_error"]["message"] {
        differences.push("see_unknown_error_message".into());
    }
    let mut invalid_see = See::new(OracleTransport::default());
    let invalid = invalid_see
        .send_xml_file("<broken")
        .await
        .err()
        .ok_or("se esperaba error de XML inválido")?;
    if json!(invalid.to_string()) != expected["see_invalid_xml_error"]["message"] {
        differences.push("see_invalid_xml_error_message".into());
    }

    let gre_transport = OracleTransport::default();
    let gre_factory = ApiFactory::new(
        gre_transport.clone(),
        Configuration::auth().with_host("http://127.0.0.1:9/auth"),
        "http://127.0.0.1:9/cpe",
        InMemoryStore::default(),
        SystemClock,
    );
    let mut api = Api::new(gre_factory);
    api.set_certificate(&pem)
        .set_api_credentials("client.test", "secret.test")
        .set_clave_sol("20000000001", "TEST", "password.test");
    compare(
        &expected,
        &mut differences,
        "api_xml_before",
        json!(api.last_xml()),
    );
    let raw_result = api.send_xml("raw", b"<caller-xml/>").await?;
    compare(
        &expected,
        &mut differences,
        "api_send_xml",
        json!({"success":raw_result.base.success,"ticket":raw_result.ticket,"error":null}),
    );
    compare(
        &expected,
        &mut differences,
        "api_xml_after_raw",
        json!(api.last_xml()),
    );
    let result = api.send(DocumentRef::Invoice(&invoice)).await?;
    compare(
        &expected,
        &mut differences,
        "api_send",
        json!({"success":result.base.success,"ticket":result.ticket,"error":null}),
    );
    compare(
        &expected,
        &mut differences,
        "api_xml_after_send",
        json!(api.last_xml()),
    );
    let status = api.get_status("T-123").await?;
    compare(
        &expected,
        &mut differences,
        "api_status",
        json!({"success":status.bill.base.success,"ticket":null,"error":status.bill.base.error.as_ref().map(|e| json!({"code":e.code,"message":e.message}))}),
    );
    gre_transport.1.store(true, Ordering::SeqCst);
    let server_error = api.send_xml("error", b"<caller-xml/>").await?;
    compare(
        &expected,
        &mut differences,
        "api_server_error",
        json!({"success":server_error.base.success,"ticket":server_error.ticket,"error":server_error.base.error.as_ref().map(|e| json!({"code":e.code,"message":e.message}))}),
    );
    compare(
        &expected,
        &mut differences,
        "api_xml_after_server_error",
        json!(api.last_xml()),
    );
    let send_error = api.send(DocumentRef::Invoice(&invoice)).await?;
    compare(
        &expected,
        &mut differences,
        "api_send_error",
        json!({"success":send_error.base.success,"ticket":send_error.ticket,"error":send_error.base.error.as_ref().map(|e| json!({"code":e.code,"message":e.message}))}),
    );
    compare(
        &expected,
        &mut differences,
        "api_xml_after_send_error",
        json!(api.last_xml()),
    );
    gre_transport.1.store(false, Ordering::SeqCst);
    let calls = gre_transport.0.lock().unwrap().clone();
    let sends: Vec<_> = calls
        .iter()
        .filter(|r| r.url.contains("/contribuyente/gem/comprobantes/") && r.method == Method::Post)
        .collect();
    let reference = expected["api_calls"]
        .as_array()
        .ok_or("api_calls ausente")?;
    for (index, request) in sends.iter().take(2).enumerate() {
        let (name, bytes, hash) = gre_content(request)?;
        let entries = zip::decompress(&bytes)?;
        if !request.url.ends_with(
            reference[index]["name"]
                .as_str()
                .ok_or("nombre CPE ausente")?,
        ) || request.header("Authorization") != Some("Bearer fake-token")
        {
            differences.push(format!("api_call_{index}_http"));
        }
        if json!(name) != reference[index]["zip_name"] {
            differences.push(format!("api_call_{index}_zip_name"));
        }
        if json!(sha256(&without_dos_time(&bytes))) != reference[index]["zip_hash_without_dos_time"]
        {
            differences.push(format!("api_call_{index}_hash"));
        }
        if sha256(&bytes) != hash {
            differences.push(format!("api_call_{index}_hash_invalid"));
        }
        if json!(entries.first().map(|e| &e.filename)) != reference[index]["entries"][0]["filename"]
        {
            differences.push(format!("api_call_{index}_entry_name"));
        }
        if json!(
            entries
                .first()
                .map(|e| String::from_utf8_lossy(&e.content).into_owned())
        ) != reference[index]["entries"][0]["content"]
        {
            differences.push(format!("api_call_{index}_entry_content"));
        }
    }
    let auth = calls
        .iter()
        .find(|request| request.url.ends_with("/oauth2/token/"));
    if let Some(auth) = auth {
        let body = String::from_utf8_lossy(&auth.body);
        if auth.method != Method::Post
            || !auth.url.contains("client.test")
            || !body.contains("client_secret=secret.test")
            || !body.contains("username=20000000001TEST")
            || !body.contains("password=password.test")
        {
            differences.push("api_auth_request".into());
        }
    } else {
        differences.push("api_auth_request_missing".into());
    }
    let get = calls.iter().find(|request| request.method == Method::Get);
    if let Some(get) = get {
        if !get.url.ends_with("/envios/T-123")
            || get.header("Authorization") != Some("Bearer fake-token")
        {
            differences.push("api_status_request".into());
        }
    } else {
        differences.push("api_status_request_missing".into());
    }
    drop(calls);
    api.set_certificate("PEM NO VALIDO");
    let sign_error = api.send(DocumentRef::Invoice(&invoice)).await;
    if sign_error.is_ok() || expected["api_sign_error"].is_null() {
        differences.push("api_sign_error".into());
    }
    compare(
        &expected,
        &mut differences,
        "api_xml_after_sign_error",
        json!(api.last_xml()),
    );
    let mut html_invoice = invoice.clone();
    if let Some(client) = html_invoice.base.client.as_mut() {
        client.rzn_social = Some("CLIENTE & SINTETICO".into());
    }
    let mut html_options = RenderOptions::cpeforge();
    html_options.autoescape = Autoescape::Html;
    html_options.strict_variables = true;
    compare(
        &expected,
        &mut differences,
        "html_invoice_pre_sign",
        json!(
            XmlBuilderResolver::new(html_options)
                .find(DocumentKind::Invoice)
                .build(DocumentRef::Invoice(&html_invoice))?
        ),
    );
    let mut html_see = See::new(OracleTransport::default());
    let mut strict = BuilderOptionsPatch::default();
    strict.strict_variables = Some(true);
    let mut escaped = BuilderOptionsPatch::default();
    escaped.autoescape = Some(Autoescape::Html);
    html_see
        .set_builder_options(strict)
        .set_builder_options(escaped)
        .set_cache_path(None);
    html_see.set_certificate(&pem)?;
    compare(
        &expected,
        &mut differences,
        "html_invoice_signed",
        json!(html_see.get_xml_signed(DocumentRef::Invoice(&html_invoice))?),
    );
    // Todas las comparaciones son exactas; después se exige que el conjunto
    // observado coincida con el registro del perfil seguro, sin sobrantes.
    if let Ok(path) = std::env::var("CPEFORGE_DIFF_OUT") {
        fs::write(path, serde_json::to_vec_pretty(&differences)?)?;
    }
    println!(
        "cpeforge referencia↔Rust: {} diferencias: {differences:?}",
        differences.len()
    );
    let mut observed = differences;
    let mut registered = registered_differences()?;
    observed.sort();
    registered.sort();
    assert_eq!(
        observed, registered,
        "diferencias no registradas u obsoletas en tests/reference/cpeforge/differences.json"
    );
    Ok(())
}

#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

fn registered_differences() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let registry: Value = serde_json::from_str(include_str!(
        "../../../tests/reference/cpeforge/differences.json"
    ))?;
    let mut ids = Vec::new();
    for entry in registry["differences"]
        .as_array()
        .ok_or("differences ausente")?
    {
        let id = entry["id"].as_str().ok_or("id ausente")?;
        perfil_seguro::validate(entry).map_err(|problem| format!("{id}: {problem}"))?;
        ids.push(id.to_owned());
    }
    Ok(ids)
}

fn result(value: SendResult) -> Value {
    match value {
        SendResult::Bill(b) => {
            json!({"success":b.base.success,"ticket":null,"error":b.base.error.as_ref().map(|e| json!({"code":e.code,"message":e.message}))})
        }
        SendResult::Summary(s) => {
            json!({"success":s.base.success,"ticket":s.ticket,"error":s.base.error.as_ref().map(|e| json!({"code":e.code,"message":e.message}))})
        }
        _ => json!({"harness_error": "variante de SendResult desconocida"}),
    }
}
