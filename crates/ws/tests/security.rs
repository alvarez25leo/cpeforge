//! Casos negativos sin Internet ni datos reales.

use std::io::{Cursor, Write};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cpeforge_core::model::response::ResponseError;
use cpeforge_core::service::BasicToken;
use cpeforge_gre_api::{
    Configuration, HttpRequest, HttpResponse, HttpTransport, ReqwestTransport, Secret,
    TokenRequest, TransportError, TransportErrorKind,
};
use cpeforge_ws::WsError;
use cpeforge_ws::api::{ApiFactory, CachedToken, Clock, InMemoryStore, TokenStore};
use cpeforge_ws::cdr::read_cdr;
use cpeforge_ws::services::{BillSender, SummarySender};
use cpeforge_ws::soap::{SoapClient, WsClient};
use cpeforge_ws::zip::{MAX_UNCOMPRESSED_BYTES, compress, decompress};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Clone)]
struct StaticTransport {
    response: HttpResponse,
}

struct FailureTransport;
impl HttpTransport for FailureTransport {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        Err(TransportError {
            kind: TransportErrorKind::Timeout,
            message: "tiempo agotado".into(),
        })
    }
}

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }
}

#[derive(Clone)]
struct AuthTransport(Arc<AtomicUsize>);
impl HttpTransport for AuthTransport {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![],
            body: br#"{"access_token":"TOKEN-FICTICIO","expires_in":3600}"#.to_vec(),
        })
    }
}

impl HttpTransport for StaticTransport {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(self.response.clone())
    }
}

fn raw_zip(name: &str, content: &[u8]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    if writer.start_file(name, options).is_err() {
        return vec![];
    }
    if writer.write_all(content).is_err() {
        return vec![];
    }
    writer.finish().map_or_else(|_| vec![], Cursor::into_inner)
}

#[test]
fn zip_traversal_bomb_and_corruption() {
    let traversal = raw_zip("../escape.xml", b"<x/>");
    assert!(matches!(decompress(&traversal), Err(WsError::Security(_))));
    assert!(matches!(
        compress("/absolute.xml", b"x"),
        Err(WsError::Security(_))
    ));
    assert!(matches!(decompress(b"bad zip"), Err(WsError::Zip(_))));
    let bomb = raw_zip("bomb.xml", &vec![b'A'; MAX_UNCOMPRESSED_BYTES + 1]);
    assert!(matches!(decompress(&bomb), Err(WsError::Zip(_))));
}

#[test]
fn xml_dtd_and_malformed_are_rejected() {
    assert!(matches!(
        read_cdr("<ApplicationResponse"),
        Err(WsError::Xml(_))
    ));
    assert!(matches!(
        read_cdr(
            "<!DOCTYPE x [<!ENTITY e SYSTEM 'file:///ruta-inexistente-sintetica/xxe.txt'>]><x>&e;</x>"
        ),
        Err(WsError::Xml(_))
    ));
}

#[test]
fn inherited_core_debug_hides_token_and_error_message() {
    let token = BasicToken::new(Some("TOKEN-FICTICIO".into()), None);
    let error = ResponseError::new(Some("501".into()), Some("CLAVE-FICTICIA".into()));
    assert!(!format!("{token:?}").contains("TOKEN-FICTICIO"));
    assert!(!format!("{error:?}").contains("CLAVE-FICTICIA"));
}

#[tokio::test]
async fn token_margin_is_strict_and_debug_is_redacted() -> Result<(), WsError> {
    let calls = Arc::new(AtomicUsize::new(0));
    let store = Arc::new(InMemoryStore::default());
    let request = TokenRequest::password_grant(
        "CLIENTE-FICTICIO",
        "SECRETO-FICTICIO",
        "USUARIO-FICTICIO",
        "CLAVE-FICTICIA",
    );
    let factory = ApiFactory::new(
        AuthTransport(calls.clone()),
        Configuration::auth().with_host("http://127.0.0.1:1/v1"),
        "http://127.0.0.1:1/v1",
        store.clone(),
        FixedClock,
    );
    let at_600 = CachedToken {
        value: Secret::new("TOKEN-CACHE"),
        expires_at: FixedClock.now() + Duration::from_secs(600),
    };
    assert!(!format!("{at_600:?}").contains("TOKEN-CACHE"));
    store.set("CLIENTE-FICTICIO", at_600)?;
    let _ = factory.create(&request).await?;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "600 segundos exactos renueva"
    );
    store.set(
        "CLIENTE-FICTICIO",
        CachedToken {
            value: Secret::new("TOKEN-CACHE"),
            expires_at: FixedClock.now() + Duration::from_secs(601),
        },
    )?;
    let _ = factory.create(&request).await?;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "más de 600 segundos reutiliza"
    );
    Ok(())
}

#[tokio::test]
async fn transport_timeout_is_typed_without_network() {
    let client = SoapClient::new(FailureTransport, "http://127.0.0.1:1", "u", "p");
    assert!(matches!(
        client.send_bill("A.zip", b"x").await,
        Err(WsError::Transport)
    ));
}

#[tokio::test]
#[ignore = "scripts/ws_tls_probe.sh levanta el servidor HTTPS local con certificado efímero"]
async fn untrusted_tls_is_rejected() {
    let url = std::env::var("WS_TLS_URL").unwrap_or_default();
    assert!(url.starts_with("https://127.0.0.1:"), "solo loopback");
    let transport = ReqwestTransport::new().unwrap_or_else(|error| panic!("{error}"));
    let client = SoapClient::new(transport, url, "USUARIO-FICTICIO", "CLAVE-FICTICIA");
    assert!(matches!(
        client.get_status("TICKET-FICTICIO").await,
        Err(WsError::Transport)
    ));
}

#[tokio::test]
async fn soap_incomplete_fault_and_redacted_debug() -> Result<(), WsError> {
    let response = HttpResponse {
        status: 200,
        reason: "OK".into(),
        headers: vec![],
        body: b"<x/>".to_vec(),
    };
    let client = SoapClient::new(
        StaticTransport { response },
        "http://127.0.0.1:1",
        "USUARIO-FICTICIO",
        "CLAVE-FICTICIA",
    );
    let request = client.request("sendBill", &[("fileName", "A<&.zip".into())])?;
    let body = String::from_utf8_lossy(&request.body);
    assert!(body.contains("A&lt;&amp;.zip"));
    assert!(!format!("{client:?}").contains("CLAVE-FICTICIA"));
    assert!(!format!("{request:?}").contains("CLAVE-FICTICIA"));
    // Un XML que no es envelope: el fault `VersionMismatch` de ext/soap.
    assert!(matches!(
        client.send_bill("A.zip", b"x").await,
        Err(WsError::SoapFault { code, .. }) if code == "VersionMismatch"
    ));

    let fault = "<soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\"><soap:Body><soap:Fault><faultcode>0156</faultcode><faultstring>CLAVE-FICTICIA</faultstring><detail><m>CLAVE-FICTICIA</m></detail></soap:Fault></soap:Body></soap:Envelope>";
    let response = HttpResponse {
        status: 500,
        reason: "Internal Server Error".into(),
        headers: vec![("Content-Type".into(), "text/xml".into())],
        body: fault.as_bytes().to_vec(),
    };
    let client = SoapClient::new(StaticTransport { response }, "http://127.0.0.1:1", "u", "p");
    let error = client.send_bill("A.zip", b"x").await.err();
    assert!(matches!(
        error,
        Some(WsError::SoapFault {
            detail: Some(_),
            ..
        })
    ));
    assert!(!format!("{error:?}").contains("CLAVE-FICTICIA"));
    assert!(
        !error
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("CLAVE-FICTICIA")
    );
    Ok(())
}

/// Respuesta HTTP sin red para `SoapClient`.
fn reply(status: u16, reason: &str, content_type: Option<&str>, body: &[u8]) -> HttpResponse {
    HttpResponse {
        status,
        reason: reason.into(),
        headers: content_type
            .map(|value| vec![("Content-Type".to_owned(), value.to_owned())])
            .unwrap_or_default(),
        body: body.to_vec(),
    }
}

#[tokio::test]
async fn http_errors_follow_reference_soap_client() {
    const FAULT: &str = "<soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\"><soap:Body><soap:Fault><faultcode>soap:Client.0156</faultcode><faultstring>El archivo esta vacio</faultstring></soap:Fault></soap:Body></soap:Envelope>";
    let declared = format!("<?xml version=\"1.0\"?>{FAULT}");
    let bom = format!("\u{feff}{declared}");
    let http = |message: &str| {
        Some(WsError::SoapFault {
            code: "HTTP".into(),
            message: message.into(),
            detail: None,
        })
    };
    let soap = Some(WsError::SoapFault {
        code: "soap:Client.0156".into(),
        message: "El archivo esta vacio".into(),
        detail: None,
    });
    let cases = [
        // SUNAT BETA ante envíos seguidos: página HTML de error.
        (
            reply(
                401,
                "Unauthorized",
                Some("text/html"),
                b"<html><body>401</body></html>",
            ),
            http("Unauthorized"),
        ),
        (reply(401, "Unauthorized", None, b""), http("Unauthorized")),
        (
            reply(
                500,
                "Internal Server Error",
                Some("text/xml; charset=utf-8"),
                FAULT.as_bytes(),
            ),
            soap.clone(),
        ),
        (
            reply(
                500,
                "Internal Server Error",
                Some("application/soap+xml"),
                FAULT.as_bytes(),
            ),
            soap.clone(),
        ),
        // `strncmp` con la longitud del valor: un prefijo de `text/xml` cuenta como XML.
        (
            reply(
                500,
                "Internal Server Error",
                Some("text/"),
                FAULT.as_bytes(),
            ),
            soap.clone(),
        ),
        (
            reply(
                500,
                "Internal Server Error",
                Some("text/xmlx"),
                FAULT.as_bytes(),
            ),
            http("Internal Server Error"),
        ),
        (
            reply(
                503,
                "Service Unavailable",
                Some("text/plain"),
                FAULT.as_bytes(),
            ),
            http("Service Unavailable"),
        ),
        (
            reply(
                503,
                "Service Unavailable",
                Some("text/plain"),
                declared.as_bytes(),
            ),
            soap.clone(),
        ),
        (
            reply(503, "Service Unavailable", None, bom.as_bytes()),
            soap.clone(),
        ),
        // Vacío aunque se declare XML.
        (
            reply(500, "Internal Server Error", Some("text/xml"), b""),
            http("Internal Server Error"),
        ),
        // Por debajo de 400 no se aplica la regla.
        (
            reply(200, "OK", Some("text/html"), b"<html/>"),
            Some(WsError::SoapFault {
                code: "VersionMismatch".into(),
                message: "Wrong Version".into(),
                detail: None,
            }),
        ),
    ];
    for (index, (response, expected)) in cases.into_iter().enumerate() {
        let client = SoapClient::new(StaticTransport { response }, "http://127.0.0.1:1", "u", "p");
        assert_eq!(
            client.send_bill("A.zip", b"x").await.err(),
            expected,
            "caso {index}"
        );
    }
}

#[test]
fn cdr_zip_with_directory_entry() -> Result<(), WsError> {
    // Forma de los CDR de SUNAT: un directorio `dummy/` y luego el XML.
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let zipped = writer
        .add_directory("dummy/", options)
        .and_then(|()| writer.start_file("R-20000000001-01-F001-1.xml", options))
        .ok()
        .and_then(|()| writer.write_all(b"<R/>").ok())
        .and_then(|()| writer.finish().ok())
        .map(Cursor::into_inner)
        .unwrap_or_default();
    let entries = decompress(&zipped)?;
    let names: Vec<&str> = entries.iter().map(|e| e.filename.as_str()).collect();
    assert_eq!(names, ["dummy/", "R-20000000001-01-F001-1.xml"]);
    assert_eq!(entries.first().map(|e| e.content.len()), Some(0));
    for unsafe_dir in ["../", "/", "a//", "./"] {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let zipped = writer
            .add_directory(unsafe_dir, options)
            .ok()
            .and_then(|()| writer.finish().ok())
            .map(Cursor::into_inner)
            .unwrap_or_default();
        assert!(decompress(&zipped).is_err(), "{unsafe_dir}");
    }
    Ok(())
}

/// Casos de `scripts/ws_http_probe.sh` con diferencia registrada del perfil
/// seguro en tests/reference/ws/differences.json (ids
/// `scripts/ws_http_probe.sh#<caso>`). Una entrada con `only_when`
/// (`{"libxml_cdata": …}`) solo cuenta si la sonda de la referencia detectó ese perfil de
/// libxml2. Cada una debe seguir existiendo.
fn deliberate_probe_cases(expected: &serde_json::Value) -> Vec<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/reference/ws/differences.json"
    );
    let registry: serde_json::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        // Sin registro legible la lista queda vacía y la prueba lo rechaza.
        .unwrap_or_default();
    registry["differences"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["classification"] == "DIFERENCIA_DELIBERADA")
        .filter(|d| {
            d["only_when"]
                .as_object()
                .into_iter()
                .flatten()
                .all(|(key, value)| expected[key] == *value)
        })
        .filter_map(|d| d["id"].as_str()?.strip_prefix("scripts/ws_http_probe.sh#"))
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
#[ignore = "scripts/ws_http_probe.sh levanta el servidor HTTP local y ejecuta el oráculo de referencia"]
async fn http_status_probe_matches_reference() {
    let base = std::env::var("WS_HTTP_URL").unwrap_or_default();
    assert!(base.starts_with("http://127.0.0.1:"), "solo loopback");
    let expected_path = std::env::var("WS_HTTP_EXPECTED").unwrap_or_default();
    let expected: serde_json::Value = std::fs::read_to_string(&expected_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| panic!("no se pudo leer {expected_path}"));
    let cases = expected["cases"].as_array().cloned().unwrap_or_default();
    assert!(!cases.is_empty());
    let deliberate = deliberate_probe_cases(&expected);
    assert!(
        !deliberate.is_empty(),
        "sin diferencias en tests/reference/ws/differences.json"
    );
    for id in &deliberate {
        assert!(
            cases.iter().any(|case| case["id"] == id.as_str()),
            "diferencia registrada sin caso: {id}"
        );
    }
    let transport = ReqwestTransport::new().unwrap_or_else(|error| panic!("{error}"));
    let error_json = |error: Option<ResponseError>| {
        error.map(|e| serde_json::json!({"code": e.code, "message": e.message}))
    };
    for case in cases {
        let id = case["id"].as_str().unwrap_or_default();
        let client = SoapClient::new(
            transport.clone(),
            format!("{base}/{id}"),
            "USUARIO-FICTICIO",
            "CLAVE-FICTICIA",
        );
        // Una excepción que escapa del sender en la referencia es un `Err` en Rust.
        let actual = if id.starts_with("summary-") {
            match SummarySender::new(client)
                .send("20000000001-RC-20261017-1", b"<SummaryDocuments/>")
                .await
            {
                Ok(result) => serde_json::json!({
                    "id": id,
                    "success": result.base.success == Some(true),
                    "error": error_json(result.base.error),
                    "ticket": result.ticket,
                }),
                Err(_) => serde_json::json!({"id": id, "exception": true}),
            }
        } else {
            match BillSender::new(client)
                .send("20000000001-01-F001-1", b"<Invoice/>")
                .await
            {
                Ok(result) => serde_json::json!({
                    "id": id,
                    "success": result.base.success == Some(true),
                    "error": error_json(result.base.error),
                    "cdr": result.cdr_response.map(|c| serde_json::json!({
                        "id": c.id,
                        "code": c.code,
                        "description": c.description,
                        "notes": c.notes.unwrap_or_default(),
                    })),
                }),
                Err(_) => serde_json::json!({"id": id, "exception": true}),
            }
        };
        if deliberate.iter().any(|d| d == id) {
            assert_ne!(actual, case, "{id}: diferencia registrada que ya no existe");
        } else {
            assert_eq!(actual, case, "{id}");
        }
    }
}
