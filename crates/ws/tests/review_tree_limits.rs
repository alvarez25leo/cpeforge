//! Regresiones de la revisión de seguridad: atributos por elemento y ámbitos
//! de espacios de nombres antes de `roxmltree`, y segmentos de ruta `.`/`..`
//! en la API GRE. Sin red.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cpeforge_gre_api::{
    Configuration, CpeApi, HttpRequest, HttpResponse, HttpTransport, TokenRequest, TransportError,
};
use cpeforge_ws::WsError;
use cpeforge_ws::api::{ApiFactory, Clock, GreSender, InMemoryStore};
use cpeforge_ws::cdr::read_cdr;
use cpeforge_ws::soap::{SoapClient, WsClient};

/// Responde siempre lo mismo y cuenta las solicitudes.
#[derive(Clone)]
struct Fixed(&'static str, Arc<AtomicUsize>);

impl HttpTransport for Fixed {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.1.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![("Content-Type".into(), "text/xml".into())],
            body: self.0.as_bytes().to_vec(),
        })
    }
}

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }
}

fn many_attributes(n: usize) -> String {
    let attributes: String = (0..n).map(|i| format!(" a{i}=''")).collect();
    format!("<ApplicationResponse{attributes}/>")
}

/// 250 prefijos en la raíz y `n` hijos que redeclaran uno: cada hijo copia
/// los 251 espacios en ámbito.
fn namespace_amplification(n: usize) -> String {
    let declarations: String = (0..250).map(|i| format!(" xmlns:p{i}='urn:{i}'")).collect();
    format!(
        "<ApplicationResponse{declarations}>{}</ApplicationResponse>",
        "<x xmlns:z='u'/>".repeat(n)
    )
}

#[test]
fn cdr_with_many_attributes_is_rejected_before_parsing() {
    // Sin el preescaneo, 40 000 atributos tardan cerca de un segundo en
    // release (la búsqueda de duplicados es cuadrática).
    let xml = many_attributes(40_000);
    let started = Instant::now();
    assert!(matches!(read_cdr(&xml), Err(WsError::Xml(_))));
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(read_cdr(&many_attributes(200)).is_ok());
}

#[test]
fn cdr_namespace_amplification_is_rejected() {
    assert!(matches!(
        read_cdr(&namespace_amplification(20_000)),
        Err(WsError::Xml(_))
    ));
    // Redeclarar los prefijos en cada elemento, como hacen algunos
    // generadores, sigue siendo válido.
    let cbc = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
    let notes = format!("<cbc:Note xmlns:cbc='{cbc}'>n</cbc:Note>").repeat(5_000);
    let xml = format!(
        "<ApplicationResponse xmlns='urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2' xmlns:cbc='{cbc}'>{notes}</ApplicationResponse>"
    );
    assert_eq!(read_cdr(&xml).unwrap().notes.map(|n| n.len()), Some(5_000));
}

#[tokio::test]
async fn soap_reply_with_many_attributes_is_a_limit_error() {
    let body: &'static str = Box::leak(
        format!(
            "<soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/'><soap:Body>{}</soap:Body></soap:Envelope>",
            many_attributes(40_000)
        )
        .into_boxed_str(),
    );
    let client = SoapClient::new(
        Fixed(body, Arc::default()),
        "http://127.0.0.1:1",
        "USUARIO-FICTICIO",
        "CLAVE-FICTICIA",
    );
    let started = Instant::now();
    assert!(matches!(
        client.send_bill("A.zip", b"x").await,
        Err(WsError::Soap(_))
    ));
    assert!(started.elapsed() < Duration::from_millis(500));
}

#[tokio::test]
async fn gre_dot_segments_never_reach_the_transport() {
    let calls = Arc::new(AtomicUsize::new(0));
    let api = CpeApi::new(
        Fixed("{}", calls.clone()),
        Configuration::cpe("TOKEN-FICTICIO").with_host("http://127.0.0.1:1/v1"),
    );
    let sender = GreSender::new(api);
    for value in [".", ".."] {
        assert!(matches!(
            sender.send(value, b"<x/>").await,
            Err(WsError::Security(_))
        ));
        assert!(matches!(
            sender.status(value).await,
            Err(WsError::Security(_))
        ));
    }

    let factory = ApiFactory::new(
        Fixed("{}", calls.clone()),
        Configuration::auth().with_host("http://127.0.0.1:1/v1"),
        "http://127.0.0.1:1/v1",
        Arc::new(InMemoryStore::default()),
        FixedClock,
    );
    let request = TokenRequest::password_grant("..", "SECRETO-FICTICIO", "USUARIO", "CLAVE");
    assert!(matches!(
        factory.create(&request).await,
        Err(WsError::Security(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
