//! Compara las mismas entradas sintéticas ejecutadas por la referencia y Rust.
//! Falla cerrado mientras existan diferencias o ramas aún no comparadas.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use cpeforge_core::model::response::{
    BillResult, ResponseError, StatusCdrResult, StatusResult, SummaryResult,
};
use cpeforge_gre_api::{
    Configuration, CpeApi, HttpRequest, HttpResponse, HttpTransport, ReqwestTransport, Secret,
    TokenRequest, TransportError,
};
use cpeforge_ws::WsError;
use cpeforge_ws::api::{ApiFactory, CachedToken, Clock, GreSender, InMemoryStore, TokenStore};
use cpeforge_ws::cdr::{extract_filename, read_cdr, resolve_type};
use cpeforge_ws::services::{BillSender, ConsultCdrService, ExtService, SummarySender};
use cpeforge_ws::soap::{SoapClient, SoapStatus, SunatEndpoints, WsClient};
use cpeforge_ws::zip::{DosDateTime, ZipTime, compress, compress_at, decompress};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CASES: &str = include_str!("../../../tests/reference/ws/cases.json");
const EXPECTED: &str = include_str!("../../../tests/reference/ws/expected.json");
const CONTRACT: &str = include_str!("../../../tests/reference/ws/contract.json");
const DIFFERENCES: &str = include_str!("../../../tests/reference/ws/differences.json");

#[path = "../../../tests/support/perfil_seguro.rs"]
mod perfil_seguro;

/// IDs de casos offline registrados. Valida cada registro con el perfil seguro.
fn registered_differences() -> Vec<String> {
    let registry: Value = serde_json::from_str(DIFFERENCES).unwrap_or_default();
    let entries = registry["differences"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!entries.is_empty(), "differences.json ilegible o vacío");
    entries
        .iter()
        .map(|entry| {
            let id = entry["id"].as_str().unwrap_or_default();
            let checked = perfil_seguro::validate(entry);
            assert!(checked.is_ok(), "{id}: {checked:?}");
            id.to_owned()
        })
        .filter(|id| !id.starts_with("scripts/"))
        .collect()
}

struct FakeClient {
    kind: String,
    mode: String,
    cdr_zip: Vec<u8>,
    calls: Mutex<Vec<Value>>,
}

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }
}

struct FakeHttp {
    mode: String,
    cdr_zip: Vec<u8>,
    calls: Mutex<Vec<HttpRequest>>,
}

impl FakeHttp {
    fn new(mode: &str, cdr_xml: &str) -> Self {
        Self {
            mode: mode.to_owned(),
            cdr_zip: compress("R-20000000001-01-F001-1.xml", cdr_xml.as_bytes())
                .unwrap_or_default(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<HttpRequest> {
        self.calls.lock().map_or_else(|_| vec![], |v| v.clone())
    }
}

impl HttpTransport for FakeHttp {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let url = request.url.clone();
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(request);
        }
        let (status, body) = if url.contains("oauth2/token") {
            let access = if self.mode == "empty" {
                ""
            } else {
                "TOKEN-FICTICIO"
            };
            (
                200,
                json!({"access_token": access, "expires_in": 3600}).to_string(),
            )
        } else if self.mode == "gre-send" {
            (200, json!({"numTicket": "TICKET-FICTICIO"}).to_string())
        } else if self.mode == "422" {
            (422, json!({"cod":"422", "msg":"Unprocessable Entity", "errors":[{"cod":"501", "msg":"Error ficticio"}]}).to_string())
        } else {
            let mut body = json!({"codRespuesta": self.mode, "indCdrGenerado": "0"});
            if self.mode == "99" {
                body["error"] = json!({"numError": "1345", "desError": "Error ficticio"});
            }
            if self.mode == "0" {
                body["indCdrGenerado"] = json!("1");
                body["arcCdr"] =
                    json!(base64::engine::general_purpose::STANDARD.encode(&self.cdr_zip));
            }
            (200, body.to_string())
        };
        Ok(HttpResponse {
            status,
            reason: if status == 200 {
                "OK".into()
            } else {
                "Unprocessable Entity".into()
            },
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: body.into_bytes(),
        })
    }
}

async fn run_gre(case: &Value, cdr_xml: &str) -> (Result<Value, WsError>, Vec<Value>) {
    let kind = case["kind"].as_str().unwrap_or_default();
    let mode = case["response"].as_str().unwrap_or_default();
    let transport = Arc::new(FakeHttp::new(
        if kind == "gre-send" { "gre-send" } else { mode },
        cdr_xml,
    ));
    let calls;
    let result = if kind == "factory" {
        let store = Arc::new(InMemoryStore::default());
        if mode == "cache" || mode == "expired" {
            let expiry = if mode == "cache" {
                FixedClock.now() + Duration::from_secs(3600)
            } else {
                FixedClock.now() - Duration::from_secs(60)
            };
            let _ = store.set(
                "CLIENTE-FICTICIO",
                CachedToken {
                    value: Secret::new(if mode == "cache" {
                        "TOKEN-CACHE"
                    } else {
                        "TOKEN-CADUCADO"
                    }),
                    expires_at: expiry,
                },
            );
        }
        let factory = ApiFactory::new(
            transport.clone(),
            Configuration::auth().with_host("http://127.0.0.1:1/v1"),
            "https://api-cpe.sunat.gob.pe/v1",
            store.clone(),
            FixedClock,
        );
        let request = TokenRequest::password_grant(
            "CLIENTE-FICTICIO",
            "SECRETO-FICTICIO",
            "USUARIO-FICTICIO",
            "CLAVE-FICTICIA",
        );
        let result = factory.create(&request).await.map(|api| {
            json!({
                "auth_calls": transport.calls().len(),
                "host": api.config().host(),
                "has_token": api.config().access_token().is_some(),
                "stored": store.get("CLIENTE-FICTICIO").ok().flatten().is_some(),
            })
        });
        calls = vec![json!({"auth_calls": transport.calls().len()})];
        result
    } else {
        let api = CpeApi::new(
            transport.clone(),
            Configuration::cpe("TOKEN-FICTICIO").with_host("http://127.0.0.1:1/v1"),
        );
        let sender = GreSender::new(api);
        if kind == "gre-send" {
            let name = case["name"].as_str().unwrap_or_default();
            let result = sender
                .send(
                    name,
                    case["content"].as_str().unwrap_or_default().as_bytes(),
                )
                .await
                .map(|v| summary_result(&v));
            let request = transport.calls().into_iter().next();
            calls = if let Some(request) = request {
                let body: Value = serde_json::from_slice(&request.body).unwrap_or_default();
                let file = &body["archivo"];
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(file["arcGreZip"].as_str().unwrap_or_default())
                    .unwrap_or_default();
                let hash = sha256_hex(&bytes);
                vec![json!({
                    "filename": name,
                    "nomArchivo": file["nomArchivo"],
                    "hash_matches": hash == file["hashZip"].as_str().unwrap_or_default(),
                    "entries": entries_json(&bytes).unwrap_or_default(),
                })]
            } else {
                vec![]
            };
            result
        } else {
            let ticket = case["ticket"].as_str().unwrap_or_default();
            let result = sender.status(ticket).await.map(|v| status_result(&v));
            calls = vec![json!({"ticket": ticket})];
            result
        }
    };
    (result, calls)
}

#[tokio::test]
#[ignore = "requiere servidor SOAP local de scripts/ws_reference_oracle.sh"]
async fn live_parity() {
    let base = std::env::var("WS_SOAP_URL").unwrap_or_default();
    assert!(base.starts_with("http://127.0.0.1:"), "solo loopback");
    let log = std::env::var("WS_SOAP_LOG").unwrap_or_default();
    let cases: Value = serde_json::from_str(CASES).unwrap_or_default();
    let expected: Value = serde_json::from_str(EXPECTED).unwrap_or_default();
    let expected_contract: Value = serde_json::from_str(CONTRACT).unwrap_or_default();
    let transport = ReqwestTransport::new().unwrap_or_else(|e| panic!("transporte: {e}"));
    let mut equal = 0;
    let mut differences = Vec::new();
    for case in cases["cases"].as_array().cloned().unwrap_or_default() {
        if case["kind"] != "soap" {
            continue;
        }
        let id = case["id"].as_str().unwrap_or_default();
        let client = SoapClient::new(
            transport.clone(),
            format!("{base}/{id}"),
            "USUARIO-FICTICIO",
            "CLAVE-FICTICIA",
        );
        let result = match case["operation"].as_str().unwrap_or_default() {
            "sendBill" => BillSender::new(client)
                .send(
                    case["name"].as_str().unwrap_or_default(),
                    case["content"].as_str().unwrap_or_default().as_bytes(),
                )
                .await
                .map(|v| bill_result(&v)),
            "sendSummary" => SummarySender::new(client)
                .send(
                    case["name"].as_str().unwrap_or_default(),
                    case["content"].as_str().unwrap_or_default().as_bytes(),
                )
                .await
                .map(|v| summary_result(&v)),
            "getStatus" => ExtService::new(client)
                .get_status(case["ticket"].as_str().unwrap_or_default())
                .await
                .map(|v| status_result(&v)),
            "getStatusCdr" => ConsultCdrService::new(client)
                .get_status_cdr(
                    case["ruc"].as_str().unwrap_or_default(),
                    case["tipo"].as_str().unwrap_or_default(),
                    case["serie"].as_str().unwrap_or_default(),
                    i32::try_from(case["numero"].as_i64().unwrap_or_default()).unwrap_or_default(),
                )
                .await
                .map(|v| consult_result(&v)),
            _ => Err(WsError::Soap("operación desconocida")),
        };
        let actual = match result {
            Ok(value) => json!({"result": value, "calls": [], "warnings": []}),
            Err(error) => {
                json!({"error": error_json(&error), "calls": [], "warnings": []})
            }
        };
        if actual == expected["results"][id] {
            equal += 1;
        } else {
            differences.push((id.to_owned(), actual, expected["results"][id].clone()));
        }
    }
    let lines = std::fs::read_to_string(&log).unwrap_or_default();
    let records: Vec<Value> = lines
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let reference_records = &records[..records.len().min(5)];
    let rust_records = &records[records.len().min(5)..];
    let expected_records = expected_contract.as_array().cloned().unwrap_or_default();
    let contract_equal = reference_records == expected_records && rust_records == expected_records;
    eprintln!(
        "ws SOAP: {equal}/5 resultados iguales; contrato de la referencia/Rust={contract_equal}; diferencias={}",
        differences.len()
    );
    for (id, actual, expected) in &differences {
        eprintln!("{id}: Rust={actual} la referencia={expected}");
    }
    assert!(
        differences.is_empty() && contract_equal,
        "paridad SOAP incompleta"
    );
}

impl FakeClient {
    fn new(kind: &str, mode: &str, cdr_xml: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            mode: mode.to_owned(),
            cdr_zip: compress("R-20000000001-01-F001-1.xml", cdr_xml.as_bytes())
                .unwrap_or_default(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn log(&self, method: &str, params: Value) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(json!({"method": method, "params": params}));
        }
    }

    fn calls(&self) -> Vec<Value> {
        self.calls.lock().map_or_else(|_| vec![], |v| v.clone())
    }
}

impl WsClient for &FakeClient {
    async fn send_bill(&self, filename: &str, zip: &[u8]) -> Result<Option<Vec<u8>>, WsError> {
        let entries = entries_json(zip)?;
        self.log(
            "sendBill",
            json!({"fileName": filename, "contentFile": entries}),
        );
        if self.mode == "fault" {
            Err(WsError::SoapFault {
                code: "0156".into(),
                message: "ERROR TEST".into(),
                detail: None,
            })
        } else if self.mode == "empty" {
            Ok(None)
        } else {
            Ok(Some(self.cdr_zip.clone()))
        }
    }

    async fn send_summary(&self, filename: &str, zip: &[u8]) -> Result<Option<String>, WsError> {
        self.log(
            "sendSummary",
            json!({"fileName": filename, "contentFile": entries_json(zip)?}),
        );
        if self.mode == "fault" {
            Err(WsError::SoapFault {
                code: "2001".into(),
                message: "ERROR TEST".into(),
                detail: None,
            })
        } else {
            Ok(Some("TICKET-FICTICIO".into()))
        }
    }

    async fn get_status(&self, ticket: &str) -> Result<Option<SoapStatus>, WsError> {
        self.log("getStatus", json!({"ticket": ticket}));
        Ok(match self.mode.as_str() {
            "missing" => None,
            "pending" => Some(SoapStatus {
                code: Some("98".into()),
                message: None,
                content: None,
            }),
            "empty" => Some(SoapStatus {
                code: Some("0".into()),
                message: None,
                content: Some(vec![]),
            }),
            _ => Some(SoapStatus {
                code: Some("0".into()),
                message: None,
                content: Some(self.cdr_zip.clone()),
            }),
        })
    }

    async fn consult(
        &self,
        cdr: bool,
        ruc: &str,
        tipo: &str,
        serie: &str,
        numero: i32,
    ) -> Result<Option<SoapStatus>, WsError> {
        let method = if cdr { "getStatusCdr" } else { "getStatus" };
        self.log(method, json!({"rucComprobante": ruc, "tipoComprobante": tipo, "serieComprobante": serie, "numeroComprobante": numero}));
        Ok(Some(SoapStatus {
            code: Some("0".into()),
            message: Some("ACEPTADA".into()),
            content: if self.kind == "consult" && cdr {
                Some(self.cdr_zip.clone())
            } else {
                None
            },
        }))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hash = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        hash.push(char::from(HEX[usize::from(byte >> 4)]));
        hash.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    hash
}

fn entries_json(bytes: &[u8]) -> Result<Value, WsError> {
    Ok(Value::Array(
        decompress(bytes)?
            .into_iter()
            .map(|entry| {
                json!({
                    "filename": entry.filename,
                    "content": String::from_utf8_lossy(&entry.content).to_string(),
                })
            })
            .collect(),
    ))
}

/// Clase de la referencia equivalente a la variante Rust. Las variantes sin equivalente
/// La referencia conservan el nombre Rust y nunca coinciden con un fixture de referencia.
fn error_json(error: &WsError) -> Value {
    let class = match error {
        WsError::XmlReader(_) => "CPEForge\\Ws\\Reader\\XmlReaderException",
        WsError::InvalidServiceResponse(_) => "CPEForge\\Services\\InvalidServiceResponseException",
        _ => "WsError",
    };
    json!({"class": class, "message": error.to_string()})
}

fn err(e: &ResponseError) -> Value {
    json!({"code": e.code, "message": e.message})
}

fn bill_result(v: &BillResult) -> Value {
    json!({
        "success": v.base.success,
        "error": v.base.error.as_ref().map(err),
        "cdr": v.cdr_response.as_ref().map(|c| json!({
            "id": c.id, "code": c.code, "description": c.description,
            "notes": c.notes, "reference": c.reference,
        })),
        "has_cdr_zip": v.cdr_zip.as_ref().is_some_and(|z| !z.is_empty()),
    })
}

fn summary_result(v: &SummaryResult) -> Value {
    json!({"success": v.base.success, "error": v.base.error.as_ref().map(err), "ticket": v.ticket})
}

fn status_result(v: &StatusResult) -> Value {
    let mut base = bill_result(&v.bill);
    base["code"] = json!(v.code);
    base
}

fn consult_result(v: &StatusCdrResult) -> Value {
    let mut base = bill_result(&v.bill);
    base["code"] = json!(v.code);
    base["message"] = json!(v.message);
    base
}

#[allow(deprecated)] // Estas dos constantes existían en la referencia y siguen inventariadas.
fn endpoint_values() -> Value {
    json!({
        "fe_beta": SunatEndpoints::FE_BETA,
        "fe_homologacion": SunatEndpoints::FE_HOMOLOGACION,
        "fe_produccion": SunatEndpoints::FE_PRODUCCION,
        "fe_consulta_cdr": SunatEndpoints::FE_CONSULTA_CDR,
        "guia_beta": SunatEndpoints::GUIA_BETA,
        "guia_produccion": SunatEndpoints::GUIA_PRODUCCION,
        "retencion_beta": SunatEndpoints::RETENCION_BETA,
        "retencion_produccion": SunatEndpoints::RETENCION_PRODUCCION,
        "wsdl_endpoint": SunatEndpoints::WSDL_ENDPOINT,
    })
}

async fn run_case(case: &Value, cdr_xml: &str) -> Value {
    let id = case["id"].as_str().unwrap_or_default();
    let kind = case["kind"].as_str().unwrap_or_default();
    let mode = case["response"].as_str().unwrap_or_default();
    let client = FakeClient::new(kind, mode, cdr_xml);
    if matches!(kind, "gre-send" | "gre-status" | "factory") {
        let (result, calls) = run_gre(case, cdr_xml).await;
        return match result {
            Ok(value) => json!({"result": value, "calls": calls, "warnings": []}),
            Err(error) => {
                json!({"error": error_json(&error), "calls": calls, "warnings": []})
            }
        };
    }
    let result: Result<Value, WsError> = match kind {
        "endpoints" => Ok(endpoint_values()),
        "zip" => {
            let name = case["name"].as_str().unwrap_or_default();
            let content = case["content"].as_str().unwrap_or_default();
            // Misma normalización que el oráculo: hora/fecha MS-DOS a cero.
            compress_at(name, content.as_bytes(), ZipTime::Fixed(DosDateTime::MIN)).and_then(
                |bytes| {
                    Ok(json!({
                        "entries": entries_json(&bytes)?,
                        "length": bytes.len(),
                        "sha256_without_dos_time": sha256_hex(&bytes),
                    }))
                },
            )
        }
        "cdr" => read_cdr(case["xml"].as_str().unwrap_or_default()).map(|c| {
            json!({
                "id": c.id, "code": c.code, "description": c.description,
                "notes": c.notes, "reference": c.reference,
            })
        }),
        "name" => extract_filename(case["xml"].as_str().unwrap_or_default()).map(Value::String),
        "type" => resolve_type(case["xml"].as_str().unwrap_or_default()).map(|s| json!(s)),
        "bill" => BillSender::new(&client)
            .send(
                case["name"].as_str().unwrap_or_default(),
                case["content"].as_str().unwrap_or_default().as_bytes(),
            )
            .await
            .map(|v| bill_result(&v)),
        "summary" => SummarySender::new(&client)
            .send(
                case["name"].as_str().unwrap_or_default(),
                case["content"].as_str().unwrap_or_default().as_bytes(),
            )
            .await
            .map(|v| summary_result(&v)),
        "ext" => ExtService::new(&client)
            .get_status(case["ticket"].as_str().unwrap_or_default())
            .await
            .map(|v| status_result(&v)),
        "consult" => {
            let ruc = case["ruc"].as_str().unwrap_or_default();
            let tipo = case["tipo"].as_str().unwrap_or_default();
            let serie = case["serie"].as_str().unwrap_or_default();
            let numero = case["numero"]
                .as_i64()
                .and_then(|v| i32::try_from(v).ok())
                .unwrap_or_default();
            if mode == "cdr" {
                ConsultCdrService::new(&client)
                    .get_status_cdr(ruc, tipo, serie, numero)
                    .await
                    .map(|v| consult_result(&v))
            } else {
                ConsultCdrService::new(&client)
                    .get_status(ruc, tipo, serie, numero)
                    .await
                    .map(|v| consult_result(&v))
            }
        }
        _ => return json!({"not_applicable": format!("{id}: caso SOAP requiere servidor local")}),
    };
    match result {
        Ok(value) => json!({"result": value, "calls": client.calls(), "warnings": []}),
        Err(error) => {
            json!({"error": error_json(&error), "calls": client.calls(), "warnings": []})
        }
    }
}

#[tokio::test]
async fn offline_parity() {
    let cases: Value = serde_json::from_str(CASES).unwrap_or_default();
    let expected: Value = serde_json::from_str(EXPECTED).unwrap_or_default();
    let inputs = cases["cases"].as_array().cloned().unwrap_or_default();
    let cdr_xml = inputs
        .iter()
        .find(|c| c["id"] == "cdr-ok")
        .and_then(|c| c["xml"].as_str())
        .unwrap_or_default();
    let mut equal = 0;
    let mut differences = Vec::new();
    let mut soap_delegated = 0;
    for case in &inputs {
        let id = case["id"].as_str().unwrap_or_default();
        let actual = run_case(case, cdr_xml).await;
        if actual.get("not_applicable").is_some() {
            assert_eq!(case["kind"], "soap", "solo SOAP se compara en live_parity");
            soap_delegated += 1;
        } else if actual == expected["results"][id] {
            equal += 1;
        } else {
            differences.push((id.to_owned(), actual, expected["results"][id].clone()));
        }
    }
    eprintln!(
        "ws offline: {} casos, {equal} iguales, {} diferencias, {soap_delegated} SOAP delegados a live_parity",
        inputs.len(),
        differences.len()
    );
    for (id, actual, expected) in &differences {
        eprintln!("{id}: Rust={actual} la referencia={expected}");
    }
    assert_eq!(soap_delegated, 5, "faltan fixtures SOAP del servidor local");
    let mut observed: Vec<String> = differences.iter().map(|(id, ..)| id.clone()).collect();
    let mut registered = registered_differences();
    observed.sort();
    registered.sort();
    assert_eq!(
        observed, registered,
        "diferencias no registradas u obsoletas en tests/reference/ws/differences.json"
    );
}
