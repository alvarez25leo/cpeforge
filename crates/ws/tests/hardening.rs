//! Regresiones de la auditoría: estado tributario, CDR estricto, caché OAuth2
//! concurrente, límites XML y ZIP, cancelación y errores de red. Sin red.

use std::io::{Cursor, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use cpeforge_core::model::response::{
    BillResult, CdrResponse, ResponseError, StatusResult, SummaryResult,
};
use cpeforge_gre_api::{
    Configuration, CpeApi, HttpRequest, HttpResponse, HttpTransport, Secret, TokenRequest,
    TransportError, TransportErrorKind,
};
use cpeforge_ws::WsError;
use cpeforge_ws::api::{
    ApiFactory, CacheKey, CachedToken, Clock, GreSender, InMemoryStore, TokenStore,
};
use cpeforge_ws::cdr::{
    MAX_XML_DEPTH, ensure_cdr_reference, read_cdr, read_cdr_checked, read_cdr_zip_checked,
};
use cpeforge_ws::outcome::TaxStatus;
use cpeforge_ws::services::{BillSender, ExtService, SummarySender};
use cpeforge_ws::soap::{MAX_SOAP_DEPTH, SoapClient, SoapStatus, WsClient};
use cpeforge_ws::zip::{MAX_UNCOMPRESSED_BYTES, compress, decompress, first_xml};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const AR: &str = "urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2";
const CAC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2";
const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const SOAP: &str = "http://schemas.xmlsoap.org/soap/envelope/";

fn cdr_xml(code: Option<&str>, notes: &[&str]) -> String {
    let notes: String = notes
        .iter()
        .map(|n| format!("<cbc:Note>{n}</cbc:Note>"))
        .collect();
    let code = code.map_or(String::new(), |c| {
        format!("<cbc:ResponseCode>{c}</cbc:ResponseCode>")
    });
    format!(
        "<ApplicationResponse xmlns=\"{AR}\" xmlns:cac=\"{CAC}\" xmlns:cbc=\"{CBC}\">{notes}\
         <cac:DocumentResponse><cac:Response><cbc:ReferenceID>F001-1</cbc:ReferenceID>{code}\
         <cbc:Description>Descripción ficticia</cbc:Description></cac:Response></cac:DocumentResponse>\
         </ApplicationResponse>"
    )
}

fn cdr_zip(xml: &str) -> Vec<u8> {
    compress("R-20000000001-01-F001-1.xml", xml.as_bytes()).unwrap_or_default()
}

// --- 1, 2: CDR estructuralmente inválido ------------------------------------

#[test]
fn cdr_with_wrong_root_or_missing_fields() {
    // Raíz incorrecta: el lector tolerante sigue devolviendo campos vacíos.
    let other =
        format!("<Invoice xmlns=\"urn:x\" xmlns:cbc=\"{CBC}\"><cbc:ID>1</cbc:ID></Invoice>");
    let lenient = read_cdr(&other).unwrap_or_default();
    assert_eq!((lenient.id, lenient.code), (None, None));
    assert_eq!(
        read_cdr_checked(&other),
        Err(WsError::InvalidResponse(
            "CDR: la raíz no es ApplicationResponse"
        ))
    );
    // Nombre correcto en otro espacio de nombres.
    let wrong_ns = cdr_xml(Some("0"), &[]).replace(AR, "urn:x");
    assert!(matches!(
        read_cdr_checked(&wrong_ns),
        Err(WsError::InvalidResponse(_))
    ));
    // Sin ResponseCode, o con uno en blanco.
    assert_eq!(
        read_cdr_checked(&cdr_xml(None, &[])),
        Err(WsError::InvalidResponse("CDR sin ResponseCode"))
    );
    assert_eq!(
        read_cdr_checked(&cdr_xml(Some("  "), &[])),
        Err(WsError::InvalidResponse("CDR sin ResponseCode"))
    );
    let no_response = format!("<ApplicationResponse xmlns=\"{AR}\" xmlns:cac=\"{CAC}\"/>");
    assert_eq!(
        read_cdr_checked(&no_response),
        Err(WsError::InvalidResponse("CDR sin DocumentResponse"))
    );
    // XML malformado es un error distinto de «no reconocido».
    assert!(matches!(read_cdr_checked("<a>"), Err(WsError::Xml(_))));
    // Completo: mismos campos que el lector tolerante.
    let ok = cdr_xml(Some("0"), &[]);
    assert_eq!(read_cdr_checked(&ok), read_cdr(&ok));
    assert_eq!(
        read_cdr_zip_checked(&cdr_zip(&ok)).map(|c| c.code),
        Ok(Some("0".into()))
    );
}

#[test]
fn cdr_reference_must_match_document() -> Result<(), WsError> {
    let cdr = read_cdr_checked(&cdr_xml(Some("0"), &[]))?;
    ensure_cdr_reference(&cdr, " F001-1 ")?;
    assert_eq!(
        ensure_cdr_reference(&cdr, "F001-2"),
        Err(WsError::InvalidResponse("CDR de otro comprobante"))
    );
    assert!(ensure_cdr_reference(&CdrResponse::default(), "F001-1").is_err());
    Ok(())
}

// --- 3, 4, 7: clasificación tributaria ----------------------------------------

#[derive(Clone)]
struct Soap {
    cdr: Option<Vec<u8>>,
    ticket: Option<String>,
    status: Option<SoapStatus>,
}

impl WsClient for Soap {
    async fn send_bill(&self, _: &str, _: &[u8]) -> Result<Option<Vec<u8>>, WsError> {
        Ok(self.cdr.clone())
    }
    async fn send_summary(&self, _: &str, _: &[u8]) -> Result<Option<String>, WsError> {
        Ok(self.ticket.clone())
    }
    async fn get_status(&self, _: &str) -> Result<Option<SoapStatus>, WsError> {
        Ok(self.status.clone())
    }
    async fn consult(
        &self,
        _: bool,
        _: &str,
        _: &str,
        _: &str,
        _: i32,
    ) -> Result<Option<SoapStatus>, WsError> {
        Ok(self.status.clone())
    }
}

fn soap(cdr: Option<Vec<u8>>) -> Soap {
    Soap {
        cdr,
        ticket: None,
        status: None,
    }
}

#[tokio::test]
async fn bill_outcomes_rejected_observed_and_accepted() -> Result<(), WsError> {
    for (code, notes, expected) in [
        ("0", &[][..], TaxStatus::Accepted),
        (
            "0",
            &["4252 - Observación"][..],
            TaxStatus::AcceptedWithObservations,
        ),
        ("2335", &[][..], TaxStatus::Rejected),
        ("0109", &[][..], TaxStatus::Exception),
    ] {
        let zip = cdr_zip(&cdr_xml(Some(code), notes));
        let result = BillSender::new(soap(Some(zip))).send("F", b"<x/>").await?;
        // Compatibilidad: hubo CDR, así que `success` es verdadero también
        // con un rechazo.
        assert_eq!(result.base.success, Some(true), "{code}");
        assert_eq!(TaxStatus::of_bill(&result), expected, "{code}");
    }
    let missing = BillSender::new(soap(None)).send("F", b"<x/>").await?;
    assert_eq!(missing.base.success, Some(false));
    assert_eq!(TaxStatus::of_bill(&missing), TaxStatus::Indeterminate);
    Ok(())
}

#[tokio::test]
async fn summary_without_ticket_is_success_but_indeterminate() -> Result<(), WsError> {
    let mut client = soap(None);
    let result = SummarySender::new(client.clone())
        .send("RC", b"<x/>")
        .await?;
    assert_eq!(
        (result.base.success, result.ticket.as_deref()),
        (Some(true), None)
    );
    assert_eq!(TaxStatus::of_summary(&result), TaxStatus::Indeterminate);
    client.ticket = Some("1700000000000".into());
    let result = SummarySender::new(client).send("RC", b"<x/>").await?;
    assert_eq!(TaxStatus::of_summary(&result), TaxStatus::Pending);
    let fault = SummaryResult {
        base: cpeforge_core::model::response::BaseResult {
            success: Some(false),
            error: Some(ResponseError::new(Some("0151".into()), None)),
        },
        ticket: None,
    };
    assert_eq!(TaxStatus::of_summary(&fault), TaxStatus::Exception);
    Ok(())
}

#[tokio::test]
async fn ticket_status_codes() -> Result<(), WsError> {
    let status = |code: &str, content: Option<Vec<u8>>| Soap {
        status: Some(SoapStatus {
            code: Some(code.into()),
            message: None,
            content,
        }),
        ..soap(None)
    };
    let pending = ExtService::new(status("98", None)).get_status("T").await?;
    assert_eq!(TaxStatus::of_status(&pending), TaxStatus::Pending);
    let rejected = cdr_zip(&cdr_xml(Some("2800"), &[]));
    let processed = ExtService::new(status("99", Some(rejected)))
        .get_status("T")
        .await?;
    assert_eq!(TaxStatus::of_status(&processed), TaxStatus::Rejected);
    let no_cdr = ExtService::new(status("0", None)).get_status("T").await?;
    assert_eq!(TaxStatus::of_status(&no_cdr), TaxStatus::Indeterminate);
    // Código que SUNAT no documenta: sin éxito, sin error, sin conclusión.
    let unknown = ExtService::new(status("77", None)).get_status("T").await?;
    assert_eq!(
        (unknown.bill.base.success, unknown.bill.base.error.is_none()),
        (Some(false), true)
    );
    assert_eq!(TaxStatus::of_status(&unknown), TaxStatus::Indeterminate);
    assert_eq!(
        TaxStatus::of_status(&StatusResult::default()),
        TaxStatus::Indeterminate
    );
    Ok(())
}

// --- 6: GRE «99» con CDR -----------------------------------------------------

#[derive(Clone)]
struct Json(&'static str, Arc<AtomicUsize>);

impl HttpTransport for Json {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.1.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: self.0.as_bytes().to_vec(),
        })
    }
}

#[tokio::test]
async fn gre_99_with_rejection_cdr() -> Result<(), WsError> {
    let zip = cdr_zip(&cdr_xml(Some("2335"), &[]));
    let body = format!(
        "{{\"codRespuesta\":\"99\",\"indCdrGenerado\":\"1\",\"arcCdr\":\"{}\",\
         \"error\":{{\"numError\":\"2335\",\"desError\":\"Rechazo ficticio\"}}}}",
        base64::engine::general_purpose::STANDARD.encode(&zip)
    );
    let body: &'static str = Box::leak(body.into_boxed_str());
    let api = CpeApi::new(
        Json(body, Arc::default()),
        Configuration::cpe("TOKEN-FICTICIO").with_host("http://127.0.0.1:1/v1"),
    );
    let result = GreSender::new(api).status("T-1").await?;
    assert_eq!(result.bill.base.success, Some(true));
    assert_eq!(TaxStatus::of_status(&result), TaxStatus::Rejected);
    assert!(!TaxStatus::of_status(&result).is_accepted());
    Ok(())
}

// --- 8, 9, 10, 18: caché OAuth2 ------------------------------------------------

struct FixedClock(SystemTime);
impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

fn clock() -> FixedClock {
    FixedClock(UNIX_EPOCH + Duration::from_secs(1_700_000_000))
}

/// Servidor de tokens que tarda en responder, para que las tareas se
/// intercalen mientras esperan.
#[derive(Clone)]
struct SlowAuth {
    calls: Arc<AtomicUsize>,
    delay: Duration,
}

impl HttpTransport for SlowAuth {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![],
            body: br#"{"access_token":"TOKEN-FICTICIO","expires_in":3600}"#.to_vec(),
        })
    }
}

fn token_request(client: &str, user: &str) -> TokenRequest {
    TokenRequest::password_grant(client, "SECRETO-FICTICIO", user, "CLAVE-FICTICIA")
}

fn factory(
    transport: SlowAuth,
    store: Arc<InMemoryStore>,
) -> ApiFactory<SlowAuth, Arc<InMemoryStore>, FixedClock> {
    ApiFactory::new(
        transport,
        Configuration::auth().with_host("http://127.0.0.1:1/v1"),
        "http://127.0.0.1:1/v1",
        store,
        clock(),
    )
}

#[tokio::test]
async fn concurrent_token_requests_share_one_renewal() {
    let calls = Arc::new(AtomicUsize::new(0));
    let auth = SlowAuth {
        calls: calls.clone(),
        delay: Duration::from_millis(20),
    };
    let store = Arc::new(InMemoryStore::default());
    let factory = Arc::new(factory(auth, store.clone()));
    let tasks: Vec<_> = (0..64)
        .map(|_| {
            let factory = factory.clone();
            tokio::spawn(async move { factory.create(&token_request("C", "U")).await.is_ok() })
        })
        .collect();
    for task in tasks {
        assert_eq!(task.await.ok(), Some(true));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    // Otra clave renueva aparte.
    assert!(factory.create(&token_request("OTRO", "U")).await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(store.len(), 2);
}

#[tokio::test]
async fn scoped_keys_isolate_tenants_and_environments() {
    let calls = Arc::new(AtomicUsize::new(0));
    let auth = SlowAuth {
        calls: calls.clone(),
        delay: Duration::ZERO,
    };
    let store = Arc::new(InMemoryStore::default());
    // Clave compatible: mismo client_id con otro usuario reutiliza el token.
    let shared = factory(auth.clone(), store.clone());
    assert!(shared.create(&token_request("C", "U1")).await.is_ok());
    assert!(shared.create(&token_request("C", "U2")).await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(shared.cache_key(&token_request("C", "U1")), "C");

    let scoped = factory(auth.clone(), store.clone()).with_cache_key(CacheKey::Scoped);
    assert!(scoped.create(&token_request("C", "U1")).await.is_ok());
    assert!(scoped.create(&token_request("C", "U2")).await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    let other_env = ApiFactory::new(
        auth,
        Configuration::auth().with_host("http://127.0.0.2:1/v1"),
        "http://127.0.0.1:1/v1",
        store.clone(),
        clock(),
    )
    .with_cache_key(CacheKey::Scoped);
    assert_ne!(
        other_env.cache_key(&token_request("C", "U1")),
        scoped.cache_key(&token_request("C", "U1"))
    );
    // Las claves no contienen secretos.
    let key = scoped.cache_key(&token_request("C", "U1"));
    assert!(!key.contains("SECRETO") && !key.contains("CLAVE"));
}

#[tokio::test]
async fn invalidation_and_expired_tokens() -> Result<(), WsError> {
    let calls = Arc::new(AtomicUsize::new(0));
    let auth = SlowAuth {
        calls: calls.clone(),
        delay: Duration::ZERO,
    };
    let store = Arc::new(InMemoryStore::default());
    let now = clock().now();
    store.set(
        "C",
        CachedToken {
            value: Secret::new("TOKEN-VENCIDO"),
            expires_at: now + Duration::from_secs(600),
        },
    )?;
    let factory = factory(auth, store.clone());
    // Le quedan 600 s exactos: se renueva.
    factory.create(&token_request("C", "U")).await?;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    factory.create(&token_request("C", "U")).await?;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    factory.invalidate(&token_request("C", "U"))?;
    assert!(store.get("C")?.is_none());
    factory.create(&token_request("C", "U")).await?;
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    store.set(
        "VIEJO",
        CachedToken {
            value: Secret::new("X"),
            expires_at: now - Duration::from_secs(1),
        },
    )?;
    assert_eq!(store.remove_expired(now), 1);
    assert_eq!(store.len(), 1);
    Ok(())
}

#[tokio::test]
async fn cancelled_renewal_does_not_block_or_leak() {
    let calls = Arc::new(AtomicUsize::new(0));
    let auth = SlowAuth {
        calls: calls.clone(),
        delay: Duration::from_secs(30),
    };
    let factory = Arc::new(factory(auth, Arc::new(InMemoryStore::default())));
    let request = token_request("C", "U");
    // La primera tarea toma el turno y se cancela durante la solicitud.
    let first = tokio::time::timeout(Duration::from_millis(20), factory.create(&request)).await;
    assert!(first.is_err());
    // Una tarea en espera también puede cancelarse.
    let holder = {
        let factory = factory.clone();
        let request = request.clone();
        tokio::spawn(async move { factory.create(&request).await.is_ok() })
    };
    tokio::time::sleep(Duration::from_millis(10)).await;
    let waiter = tokio::time::timeout(Duration::from_millis(10), factory.create(&request)).await;
    assert!(waiter.is_err());
    holder.abort();
    let _ = holder.await;
    // La espera cancelada no llegó a pedir token: solo la primera y la del
    // `holder` lo hicieron.
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    // Sin turnos retenidos: la misma factoría vuelve a llegar al servidor de
    // tokens enseguida.
    let again = tokio::time::timeout(Duration::from_millis(50), factory.create(&request)).await;
    assert!(again.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

// --- 11, 19: timeout y errores de red consecutivos -------------------------------

#[derive(Clone)]
struct Failing(Arc<AtomicUsize>);

impl HttpTransport for Failing {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(TransportError {
            kind: TransportErrorKind::Timeout,
            message: "tiempo agotado".into(),
        })
    }
}

#[tokio::test]
async fn timeouts_are_errors_and_never_retried() {
    let calls = Arc::new(AtomicUsize::new(0));
    let client = SoapClient::new(Failing(calls.clone()), "http://127.0.0.1:1/b", "u", "p");
    let bill = BillSender::new(client.clone());
    let summary = SummarySender::new(client);
    for attempt in 1..=5 {
        assert_eq!(bill.send("F", b"<x/>").await, Err(WsError::Transport));
        assert_eq!(calls.load(Ordering::SeqCst), 2 * attempt - 1);
        assert_eq!(summary.send("RC", b"<x/>").await, Err(WsError::Transport));
        assert_eq!(calls.load(Ordering::SeqCst), 2 * attempt);
    }
}

// --- 12, 16, 17: respuestas SOAP incompletas, profundas y faults complejos ---------

#[derive(Clone)]
struct Reply(u16, Arc<Vec<u8>>);

impl HttpTransport for Reply {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse {
            status: self.0,
            reason: "OK".into(),
            headers: vec![("Content-Type".into(), "text/xml".into())],
            body: self.1.as_ref().clone(),
        })
    }
}

fn client(status: u16, body: String) -> SoapClient<Reply> {
    SoapClient::new(
        Reply(status, Arc::new(body.into_bytes())),
        "http://127.0.0.1:1/b",
        "u",
        "p",
    )
}

/// Fault cuyo documento completo tiene `depth` niveles (Envelope = 1).
fn deep_fault(depth: usize) -> String {
    let inner = depth - 4;
    format!(
        "<soap:Envelope xmlns:soap=\"{SOAP}\"><soap:Body><soap:Fault><faultcode>soap:Client.0111</faultcode>\
         <faultstring>Profundo</faultstring><detail>{}x{}</detail></soap:Fault></soap:Body></soap:Envelope>",
        "<a>".repeat(inner),
        "</a>".repeat(inner)
    )
}

/// Ejecuta `f` en un hilo con la pila de un worker de Tokio (2 MiB): el
/// límite de profundidad debe caber ahí también en compilación debug.
fn with_worker_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(f)
        .ok()?
        .join()
        .ok()
}

#[test]
fn deep_soap_and_cdr_are_rejected_before_parsing() {
    let runtime = || tokio::runtime::Builder::new_current_thread().build();
    // En el límite: se lee el fault completo, en un hilo de 2 MiB.
    let at_limit = with_worker_stack(move || {
        let rt = runtime().ok()?;
        rt.block_on(client(500, deep_fault(MAX_SOAP_DEPTH)).send_bill("A.zip", b""))
            .err()
    });
    assert!(matches!(
        at_limit.flatten(),
        Some(WsError::SoapFault { code, detail: Some(_), .. }) if code == "soap:Client.0111"
    ));
    // Por encima del límite, y muy por encima (antes abortaba el proceso por
    // desbordamiento de pila): documento ilegible, en el hilo de 2 MiB.
    let no_xml = WsError::SoapFault {
        code: "Client".into(),
        message: "looks like we got no XML document".into(),
        detail: None,
    };
    let rt = runtime().ok();
    for depth in [MAX_SOAP_DEPTH + 1, 5_000, 40_000] {
        let result = rt
            .as_ref()
            .map(|rt| rt.block_on(client(500, deep_fault(depth)).send_bill("A.zip", b"")));
        assert_eq!(result, Some(Err(no_xml.clone())), "{depth}");
    }
    let cdr = |depth: usize| {
        format!(
            "<ApplicationResponse xmlns=\"{AR}\">{}{}</ApplicationResponse>",
            "<a>".repeat(depth - 1),
            "</a>".repeat(depth - 1)
        )
    };
    assert_eq!(
        read_cdr(&cdr(MAX_XML_DEPTH + 1)),
        Err(WsError::Xml("anidamiento demasiado profundo"))
    );
    // En el límite, el CDR se analiza (aunque no tenga datos) sin agotar la
    // pila de 2 MiB.
    let at_limit = with_worker_stack(move || read_cdr(&cdr(MAX_XML_DEPTH)));
    assert!(
        !matches!(
            at_limit,
            Some(Err(WsError::Xml("anidamiento demasiado profundo"))) | None
        ),
        "{at_limit:?}"
    );
}

#[tokio::test]
async fn complex_fault_with_many_names_is_linear() {
    let mut detail = String::new();
    for i in 0..20_000 {
        detail.push_str(&format!("<e{i}>{i}</e{i}>"));
    }
    detail.push_str("<e0>dup</e0>");
    let body = format!(
        "<soap:Envelope xmlns:soap=\"{SOAP}\"><soap:Body><soap:Fault><faultcode>soap:Client.0111</faultcode>\
         <faultstring>Sin perfil</faultstring><detail>{detail}</detail></soap:Fault></soap:Body></soap:Envelope>"
    );
    let started = Instant::now();
    let error = client(500, body).send_bill("A.zip", b"").await.err();
    let elapsed = started.elapsed();
    let Some(WsError::SoapFault {
        detail: Some(json), ..
    }) = error
    else {
        panic!("se esperaba un fault con detail");
    };
    assert!(json.starts_with("{\"e0\":[\"0\",\"dup\"],\"e1\":\"1\","));
    assert!(json.ends_with(",\"e19999\":\"19999\"}"));
    // Antes del índice, agrupar 20 000 nombres tardaba ~180 ms en release y
    // varios segundos en debug.
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[tokio::test]
async fn incomplete_soap_replies() {
    let envelope = |body: &str| {
        format!(
            "<soap:Envelope xmlns:soap=\"{SOAP}\"><soap:Body>{body}</soap:Body></soap:Envelope>"
        )
    };
    // Sin elemento de respuesta, o sin applicationResponse: falta el CDR.
    for body in [
        String::new(),
        "<ns:sendBillResponse xmlns:ns=\"http://service.sunat.gob.pe\"/>".to_owned(),
    ] {
        let result = BillSender::new(client(200, envelope(&body)))
            .send("F", b"<x/>")
            .await;
        let Ok(result) = result else {
            panic!("se esperaba un resultado");
        };
        assert_eq!(
            result.base.error.and_then(|e| e.code).as_deref(),
            Some("CDR")
        );
    }
    // Base64 inválido o CDR que no es ZIP: error tipado, nunca éxito.
    let bad = envelope(
        "<ns:sendBillResponse xmlns:ns=\"http://service.sunat.gob.pe\"><applicationResponse>@@</applicationResponse></ns:sendBillResponse>",
    );
    assert_eq!(
        BillSender::new(client(200, bad)).send("F", b"<x/>").await,
        Err(WsError::Soap("Base64 inválido"))
    );
    let not_zip = envelope(
        "<ns:sendBillResponse xmlns:ns=\"http://service.sunat.gob.pe\"><applicationResponse>bm8temlw</applicationResponse></ns:sendBillResponse>",
    );
    assert!(matches!(
        BillSender::new(client(200, not_zip))
            .send("F", b"<x/>")
            .await,
        Err(WsError::Zip(_))
    ));
}

// --- 13, 14, 15: ZIP grandes, directorios y entradas maliciosas ------------------------

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in entries {
        let ok = if name.ends_with('/') {
            writer.add_directory(*name, options).is_ok()
        } else {
            writer.start_file(*name, options).is_ok() && writer.write_all(content).is_ok()
        };
        if !ok {
            return Vec::new();
        }
    }
    writer
        .finish()
        .map_or_else(|_| Vec::new(), Cursor::into_inner)
}

#[test]
fn large_highly_compressible_zip_roundtrip() -> Result<(), WsError> {
    let content = vec![b'A'; MAX_UNCOMPRESSED_BYTES];
    let zipped = compress("grande.xml", &content)?;
    assert!(zipped.len() < 64 * 1024, "{}", zipped.len());
    assert_eq!(first_xml(&zipped)?.map(|x| x.len()), Some(content.len()));
    // Un byte más que el límite se rechaza al comprimir y al descomprimir.
    let over = vec![b'A'; MAX_UNCOMPRESSED_BYTES + 1];
    assert!(compress("grande.xml", &over).is_err());
    let bomb = zip_of(&[("a.xml", &over)]);
    assert_eq!(
        first_xml(&bomb),
        Err(WsError::Zip("contenido descomprimido demasiado grande"))
    );
    Ok(())
}

#[test]
fn safe_directories_are_accepted() -> Result<(), WsError> {
    let zip = zip_of(&[
        ("dummy/", b""),
        ("a/b/", b""),
        ("a/b/nota.txt", b"texto"),
        ("a/b/R-1.XML", b"<cdr/>"),
        ("R-2.xml", b"<otro/>"),
    ]);
    assert_eq!(first_xml(&zip)?, Some(b"<cdr/>".to_vec()));
    let names: Vec<_> = decompress(&zip)?.into_iter().map(|e| e.filename).collect();
    assert_eq!(
        names,
        ["dummy/", "a/b/", "a/b/nota.txt", "a/b/R-1.XML", "R-2.xml"]
    );
    Ok(())
}

#[test]
fn first_xml_still_validates_entries_it_does_not_return() {
    // Una entrada insegura o corrupta detrás del XML se rechaza igual que en
    // `decompress`, aunque `first_xml` solo devuelva la primera.
    let traversal = zip_of(&[("R-1.xml", b"<cdr/>"), ("../escape.xml", b"x")]);
    assert_eq!(
        first_xml(&traversal),
        Err(WsError::Security("nombre ZIP inseguro"))
    );
    assert_eq!(
        decompress(&traversal).err(),
        Some(WsError::Security("nombre ZIP inseguro"))
    );

    let mut corrupt = zip_of(&[("R-1.xml", b"<cdr/>"), ("otro.bin", &[7u8; 4096])]);
    // Se altera un byte del flujo DEFLATE de la segunda entrada.
    let second = corrupt
        .windows(8)
        .position(|w| w == b"otro.bin")
        .map(|p| p + 8 + 2);
    if let Some(byte) = second.and_then(|p| corrupt.get_mut(p)) {
        *byte ^= 0xff;
    }
    assert!(decompress(&corrupt).is_err());
    assert_eq!(first_xml(&corrupt).is_err(), decompress(&corrupt).is_err());

    let many: Vec<(String, Vec<u8>)> = (0..65).map(|i| (format!("{i}.xml"), vec![])).collect();
    let refs: Vec<(&str, &[u8])> = many
        .iter()
        .map(|(n, c)| (n.as_str(), c.as_slice()))
        .collect();
    assert_eq!(
        first_xml(&zip_of(&refs)),
        Err(WsError::Zip("demasiadas entradas"))
    );
}

// --- 20: carga sostenida sin crecimiento de estado ------------------------------------

#[tokio::test]
async fn sustained_load_keeps_state_bounded() -> Result<(), WsError> {
    let store = Arc::new(InMemoryStore::default());
    let calls = Arc::new(AtomicUsize::new(0));
    let factory = factory(
        SlowAuth {
            calls: calls.clone(),
            delay: Duration::ZERO,
        },
        store.clone(),
    );
    let zip = cdr_zip(&cdr_xml(Some("0"), &[]));
    let sender = BillSender::new(soap(Some(zip)));
    for i in 0..2_000 {
        factory
            .create(&token_request(&format!("C{}", i % 10), "U"))
            .await?;
        let result: BillResult = sender.send("F", b"<x/>").await?;
        assert_eq!(TaxStatus::of_bill(&result), TaxStatus::Accepted);
    }
    // Diez claves, diez renovaciones: ni la tienda ni el registro de
    // renovaciones crecen con el número de solicitudes.
    assert_eq!((store.len(), calls.load(Ordering::SeqCst)), (10, 10));
    Ok(())
}
