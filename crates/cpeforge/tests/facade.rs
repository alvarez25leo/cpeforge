//! Pruebas sin red de orden, selección y estado de las fachadas.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use cpeforge::Api;
use cpeforge::factory::{FeFactory, SendResult, Sender, XmlSigner};
use cpeforge::resolver::{DocumentKind, DocumentRef, SenderKind, XmlBuilder, sender_kind};
use cpeforge::{CpeForgeError, ErrorCodeProvider, See};
use cpeforge_core::model::company::Company;
use cpeforge_core::model::response::SummaryResult;
use cpeforge_core::model::sale::{BaseSale, Invoice};
use cpeforge_gre_api::Configuration;
use cpeforge_gre_api::{
    HttpRequest, HttpResponse, HttpTransport, TransportError, TransportErrorKind,
};
use cpeforge_ws::api::{ApiFactory, InMemoryStore, SystemClock};

fn invoice() -> Invoice {
    Invoice {
        base: BaseSale {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("123".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                ..Company::default()
            }),
            ..BaseSale::default()
        },
        ..Invoice::default()
    }
}

struct Builder {
    fail: bool,
}
impl XmlBuilder for Builder {
    fn build(&self, _: DocumentRef<'_>) -> Result<String, CpeForgeError> {
        if self.fail {
            Err(CpeForgeError::UnsupportedDocument)
        } else {
            Ok("<Invoice/>".into())
        }
    }
}
struct Signer {
    fail: bool,
}
impl XmlSigner for Signer {
    fn sign_xml(&self, xml: &str) -> Result<String, CpeForgeError> {
        if self.fail {
            Err(CpeForgeError::Missing("test signer"))
        } else {
            Ok(format!("signed:{xml}"))
        }
    }
}
type RecordedSends = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

struct Capture {
    requests: RecordedSends,
    fail: bool,
}
impl Sender for Capture {
    fn send<'a>(
        &'a self,
        filename: &'a str,
        xml: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<SendResult, CpeForgeError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests
                .lock()
                .map_err(|_| CpeForgeError::Missing("capture"))?
                .push((filename.into(), xml.into()));
            if self.fail {
                Err(CpeForgeError::Missing("test sender"))
            } else {
                Ok(SendResult::Summary(SummaryResult::default()))
            }
        })
    }
}

#[tokio::test]
async fn factory_order_and_last_xml_after_each_failure() {
    let doc = invoice();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut factory = FeFactory::new();
    assert!(factory.last_xml().is_none());
    factory
        .set_builder(Some(Box::new(Builder { fail: false })))
        .set_signer(Some(Box::new(Signer { fail: false })))
        .set_sender(Some(Box::new(Capture {
            requests: requests.clone(),
            fail: false,
        })));
    assert!(factory.send(DocumentRef::Invoice(&doc)).await.is_ok());
    assert_eq!(factory.last_xml(), Some("signed:<Invoice/>"));
    let saved = factory.last_xml().map(str::to_owned);
    assert!(factory.send_xml("raw", b"<already-signed/>").await.is_ok());
    assert_eq!(factory.last_xml(), saved.as_deref());
    factory.set_builder(Some(Box::new(Builder { fail: true })));
    assert!(factory.send(DocumentRef::Invoice(&doc)).await.is_err());
    assert_eq!(factory.last_xml(), saved.as_deref());
    factory
        .set_builder(Some(Box::new(Builder { fail: false })))
        .set_signer(Some(Box::new(Signer { fail: true })));
    assert!(factory.send(DocumentRef::Invoice(&doc)).await.is_err());
    assert_eq!(factory.last_xml(), saved.as_deref());
    factory
        .set_signer(Some(Box::new(Signer { fail: false })))
        .set_sender(Some(Box::new(Capture {
            requests: requests.clone(),
            fail: true,
        })));
    assert!(factory.send(DocumentRef::Invoice(&doc)).await.is_err());
    assert_eq!(factory.last_xml(), Some("signed:<Invoice/>"));
    let recorded = requests
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        recorded[0],
        (
            "20000000001-01-F001-123".into(),
            b"signed:<Invoice/>".to_vec()
        )
    );
    assert_eq!(recorded[1], ("raw".into(), b"<already-signed/>".to_vec()));
    assert_eq!(recorded.len(), 3);
}

#[test]
fn selection_includes_reversion() {
    assert_eq!(sender_kind(DocumentKind::Summary), SenderKind::Summary);
    assert_eq!(sender_kind(DocumentKind::Voided), SenderKind::Summary);
    assert_eq!(sender_kind(DocumentKind::Reversion), SenderKind::Summary);
    for kind in [
        DocumentKind::Invoice,
        DocumentKind::Note,
        DocumentKind::Despatch,
        DocumentKind::Perception,
        DocumentKind::Retention,
        DocumentKind::Settlement,
    ] {
        assert_eq!(sender_kind(kind), SenderKind::Bill);
    }
    assert_eq!(
        DocumentKind::from_class_name("CPEForge\\Model\\Settlement\\PurchaseSettlement").ok(),
        Some(DocumentKind::Settlement)
    );
    assert!(DocumentKind::from_class_name("Example\\Unknown").is_err());
}

#[derive(Clone)]
struct LocalTransport(Arc<Mutex<Vec<HttpRequest>>>);
impl HttpTransport for LocalTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request);
        Ok(HttpResponse { status: 200, reason: "OK".into(), headers: vec![], body: br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><sendSummaryResponse xmlns="http://service.sunat.gob.pe"><ticket>T-123</ticket></sendSummaryResponse></soap:Body></soap:Envelope>"#.to_vec() })
    }
}

#[tokio::test]
async fn see_send_xml_uses_original_bytes_and_never_signs() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut see = See::new(LocalTransport(requests.clone()));
    see.set_service("http://127.0.0.1:9/soap").set_clave_sol(
        "20000000001",
        "TEST",
        "fake-password",
    );
    let result = see
        .send_xml(
            DocumentKind::Summary,
            "20000000001-RC-20240101-1",
            b"<raw/>",
        )
        .await;
    assert!(result.is_ok(), "{result:?}");
    assert!(see.factory_ref().last_xml().is_none());
    let captured = requests
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(captured.len(), 1);
    let soap = String::from_utf8_lossy(&captured[0].body);
    assert!(soap.contains("20000000001TEST"));
    assert!(soap.contains("20000000001-RC-20240101-1.zip"));
    assert!(!soap.contains("Signature"));
}

#[tokio::test]
async fn malformed_xml_and_dtd_are_rejected_before_transport() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut see = See::new(LocalTransport(requests.clone()));
    assert!(see.send_xml_file("<broken").await.is_err());
    assert!(
        see.send_xml_file(
            "<!DOCTYPE x [<!ENTITY e SYSTEM 'file:///etc/passwd'>]><Invoice>&e;</Invoice>"
        )
        .await
        .is_err()
    );
    assert!(
        requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

#[tokio::test]
async fn missing_credentials_do_not_reach_gre_or_leak_to_debug() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = LocalTransport(requests.clone());
    let factory = ApiFactory::new(
        transport,
        Configuration::auth().with_host("http://127.0.0.1:9/auth"),
        "http://127.0.0.1:9/cpe",
        InMemoryStore::default(),
        SystemClock,
    );
    let mut api = Api::new(factory);
    assert!(matches!(
        api.send_xml("fake", b"<raw/>").await,
        Err(CpeForgeError::Missing("client_id"))
    ));
    api.set_api_credentials("client.test", "very-secret")
        .set_clave_sol("20000000001", "TEST", "very-secret-password")
        .set_certificate("very-secret-pem");
    let debug = format!("{api:?}");
    for secret in [
        "very-secret",
        "very-secret-password",
        "very-secret-pem",
        "client.test",
    ] {
        assert!(!debug.contains(secret));
    }
    assert!(
        requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
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
        (code == "0100").then(|| "Mensaje ficticio".into())
    }
}

#[tokio::test]
async fn code_provider_translates_soap_fault_without_exposing_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    let mut see = See::new(FaultTransport);
    see.set_code_provider(Some(Arc::new(Codes)));
    let result = see.send_xml(DocumentKind::Invoice, "fake", b"<raw/>").await;
    let bill = match result {
        Ok(SendResult::Bill(bill)) => bill,
        _ => return Err("se esperaba BillResult".into()),
    };
    let error = bill.base.error.ok_or("fault esperado")?;
    assert_eq!(error.code.as_deref(), Some("0100"));
    assert_eq!(error.message.as_deref(), Some("Mensaje ficticio"));
    Ok(())
}

#[derive(Clone)]
struct BrokenTransport {
    incomplete: bool,
}
impl HttpTransport for BrokenTransport {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        if self.incomplete {
            Ok(HttpResponse { status: 200, reason: "OK".into(), headers: vec![], body: br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body/></soap:Envelope>"#.to_vec() })
        } else {
            Err(TransportError {
                kind: TransportErrorKind::Connect,
                message: "fallo ficticio".into(),
            })
        }
    }
}

#[tokio::test]
async fn network_failure_incomplete_reply_and_unsafe_zip_name() {
    let mut network = See::new(BrokenTransport { incomplete: false });
    assert!(matches!(
        network
            .send_xml(DocumentKind::Summary, "fake", b"<xml/>")
            .await,
        Err(CpeForgeError::Service(_))
    ));
    // `Body` sin `sendSummaryResponse`: como la referencia (`null->ticket` solo avisa),
    // el envío es correcto y sin ticket (caso `summary-missing-response` de
    // scripts/ws_http_probe.sh).
    let mut incomplete = See::new(BrokenTransport { incomplete: true });
    match incomplete
        .send_xml(DocumentKind::Summary, "fake", b"<xml/>")
        .await
    {
        Ok(SendResult::Summary(summary)) => {
            assert_eq!(summary.base.success, Some(true));
            assert_eq!(summary.ticket, None);
        }
        other => panic!("se esperaba SummaryResult sin ticket: {other:?}"),
    }
    let mut unsafe_name = See::new(BrokenTransport { incomplete: false });
    assert!(matches!(
        unsafe_name
            .send_xml(DocumentKind::Summary, "../escape", b"<xml/>")
            .await,
        Err(CpeForgeError::Service(_))
    ));
}
