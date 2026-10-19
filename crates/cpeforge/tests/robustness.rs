//! Robustez de las fachadas sin red: concurrencia, procesamiento masivo,
//! certificados inválidos, tokens vencidos, respuestas inesperadas y el orden
//! de errores de `send_xml_file`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use cpeforge::{Api, CpeForgeError, DocumentRef, See, SendResult};
use cpeforge_core::model::company::Company;
use cpeforge_core::model::sale::{BaseSale, Invoice, SaleDetail};
use cpeforge_core::model::summary::Summary;
use cpeforge_gre_api::{Configuration, HttpRequest, HttpResponse, HttpTransport, TransportError};
use cpeforge_ws::WsError;
use cpeforge_ws::api::{ApiFactory, Clock, InMemoryStore};
use cpeforge_ws::zip::ZipTime;

/// PEM efímero: clave RSA y certificado autofirmado generados con el
/// `openssl` del sistema (los mismos que usa `scripts/xmldsig_keys.sh`).
fn ephemeral_pem() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "cpeforge-robustness-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let (key, cert) = (dir.join("k.pem"), dir.join("c.pem"));
    let status = std::process::Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
        ])
        .args(["-subj", "/CN=20000000001", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .stderr(std::process::Stdio::null())
        .status()
        .expect("openssl disponible");
    assert!(status.success());
    let pem = std::fs::read_to_string(&key).unwrap() + &std::fs::read_to_string(&cert).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    pem
}

fn invoice(correlativo: usize) -> Invoice {
    Invoice {
        base: BaseSale {
            ubl_version: Some("2.1".into()),
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some(correlativo.to_string()),
            fecha_emision: Some("2024-01-31T23:30:00-05:00[America/Lima]".parse().unwrap()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                ..Company::default()
            }),
            details: Some(vec![SaleDetail {
                descripcion: Some("PRODUCTO".into()),
                ..SaleDetail::default()
            }]),
            ..BaseSale::default()
        },
        ..Invoice::default()
    }
}

/// Transporte local con respuesta fija y registro de solicitudes.
#[derive(Clone)]
struct Fixed {
    status: u16,
    body: &'static [u8],
    calls: Arc<AtomicUsize>,
}

impl Fixed {
    fn new(status: u16, body: &'static [u8]) -> Self {
        Self {
            status,
            body,
            calls: Arc::default(),
        }
    }
}

impl HttpTransport for Fixed {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: self.status,
            reason: "OK".into(),
            headers: vec![],
            body: self.body.to_vec(),
        })
    }
}

const TICKET: &[u8] = br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><sendSummaryResponse xmlns="http://service.sunat.gob.pe"><ticket>T-1</ticket></sendSummaryResponse></soap:Body></soap:Envelope>"#;

fn see(pem: &str, transport: Fixed) -> See<Fixed> {
    let mut see = See::new(transport);
    see.set_service("http://127.0.0.1:9/soap")
        .set_clave_sol("20000000001", "TEST", "fake")
        .set_zip_time(ZipTime::default());
    see.set_certificate(pem).unwrap();
    see
}

/// Varias fachadas en hilos distintos firman lo mismo que una sola, sin
/// interferir entre sí (la firma RSA PKCS#1 v1.5 es determinista).
#[test]
fn concurrent_facades_produce_identical_signed_xml() {
    let pem = Arc::new(ephemeral_pem());
    let reference = {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut facade = see(&pem, Fixed::new(200, TICKET));
        runtime.block_on(async {
            let doc = invoice(7);
            facade
                .get_xml_signed(DocumentRef::Invoice(&doc))
                .unwrap()
                .to_owned()
        })
    };
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let pem = pem.clone();
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap();
                let transport = Fixed::new(200, TICKET);
                let calls = transport.calls.clone();
                let mut facade = see(&pem, transport);
                let doc = invoice(7);
                let signed = runtime.block_on(async {
                    for _ in 0..5 {
                        facade.send(DocumentRef::Invoice(&doc)).await.unwrap();
                    }
                    facade.factory_ref().last_xml().unwrap().to_owned()
                });
                (signed, calls.load(Ordering::SeqCst))
            })
        })
        .collect();
    for handle in handles {
        let (signed, calls) = handle.join().unwrap();
        assert_eq!(signed, reference);
        assert_eq!(calls, 5);
    }
}

/// Procesamiento masivo con una fachada: cada envío lleva su nombre y
/// `last_xml` corresponde siempre al último documento.
#[tokio::test]
async fn bulk_sending_keeps_names_and_last_xml_in_order() {
    let pem = ephemeral_pem();
    let names = Arc::new(Mutex::new(Vec::new()));
    #[derive(Clone)]
    struct Recorder(Arc<Mutex<Vec<String>>>);
    impl HttpTransport for Recorder {
        async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            let body = String::from_utf8_lossy(&request.body).into_owned();
            let name = body
                .split("<fileName>")
                .nth(1)
                .and_then(|rest| rest.split('<').next())
                .unwrap_or_default()
                .to_owned();
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(name);
            Ok(HttpResponse {
                status: 200,
                reason: "OK".into(),
                headers: vec![],
                body: TICKET.to_vec(),
            })
        }
    }
    let mut facade = See::new(Recorder(names.clone()));
    facade
        .set_service("http://127.0.0.1:9/soap")
        .set_zip_time(ZipTime::default());
    facade.set_certificate(&pem).unwrap();
    for correlativo in 1..=200 {
        let doc = invoice(correlativo);
        facade.send(DocumentRef::Invoice(&doc)).await.unwrap();
        let last = facade.factory_ref().last_xml().unwrap();
        assert!(last.contains(&format!("<cbc:ID>F001-{correlativo}</cbc:ID>")));
    }
    let names = names.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(names.len(), 200);
    for (index, name) in names.iter().enumerate() {
        assert_eq!(name, &format!("20000000001-01-F001-{}.zip", index + 1));
    }
}

/// Un certificado inválido falla al firmar: no se envía nada ni cambia
/// `last_xml`.
#[tokio::test]
async fn invalid_certificate_never_reaches_the_transport() {
    let pem = ephemeral_pem();
    let transport = Fixed::new(200, TICKET);
    let calls = transport.calls.clone();
    let mut facade = see(&pem, transport);
    let doc = invoice(1);
    facade.send(DocumentRef::Invoice(&doc)).await.unwrap();
    let saved = facade.factory_ref().last_xml().map(str::to_owned);
    for bad in [
        "",
        "no es un PEM",
        "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n",
    ] {
        facade.set_certificate(bad).unwrap();
        let result = facade.send(DocumentRef::Invoice(&doc)).await;
        assert!(
            matches!(result, Err(CpeForgeError::Signature(_))),
            "{bad:?}: {result:?}"
        );
        assert_eq!(facade.factory_ref().last_xml().map(str::to_owned), saved);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Respuestas que SUNAT no debería dar: no provocan pánicos y nunca se
/// presentan como un ticket o CDR válido.
#[tokio::test]
async fn unexpected_responses_are_errors_not_tickets() {
    let pem = ephemeral_pem();
    let doc = Summary {
        correlativo: Some("1".into()),
        fec_generacion: Some("2024-01-31T10:00:00-05:00[America/Lima]".parse().unwrap()),
        fec_resumen: Some("2024-01-31T10:00:00-05:00[America/Lima]".parse().unwrap()),
        company: Some(Company {
            ruc: Some("20000000001".into()),
            ..Company::default()
        }),
        details: Some(vec![]),
        ..Summary::default()
    };
    for (status, body) in [
        (200, &b"<html><body>Mantenimiento</body></html>"[..]),
        (200, &b""[..]),
        (200, &b"no es XML"[..]),
        (502, &b"Bad Gateway"[..]),
        (200, &br#"<!DOCTYPE x [<!ENTITY e "x">]><x>&e;</x>"#[..]),
        (200, &br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><sendSummaryResponse xmlns="http://service.sunat.gob.pe"><ticket></ticket></sendSummaryResponse></soap:Body></soap:Envelope>"#[..]),
    ] {
        let mut facade = see(&pem, Fixed::new(status, body));
        match facade.send(DocumentRef::Summary(&doc)).await {
            Ok(SendResult::Summary(summary)) => assert!(
                summary.ticket.as_deref().is_none_or(str::is_empty),
                "{status} {:?}: ticket inesperado",
                String::from_utf8_lossy(body)
            ),
            Ok(other) => panic!("resultado inesperado: {other:?}"),
            Err(CpeForgeError::Service(_)) => {}
            Err(other) => panic!("error inesperado: {other:?}"),
        }
        // El XML firmado se conserva aunque falle el envío.
        assert!(facade.factory_ref().last_xml().is_some());
    }
}

/// `send_xml_file` analiza el XML una vez y conserva el orden de errores:
/// XML inválido, después tipo no admitido y después nombre.
#[tokio::test]
async fn send_xml_file_errors_keep_their_order() {
    let transport = Fixed::new(200, TICKET);
    let calls = transport.calls.clone();
    let mut facade = See::new(transport);
    facade.set_service("http://127.0.0.1:9/soap");
    type Check = fn(&CpeForgeError) -> bool;
    let cases: [(&str, Check); 4] = [
        ("<broken", |e| {
            matches!(e, CpeForgeError::Service(WsError::Xml(_)))
        }),
        ("<Desconocido/>", |e| {
            matches!(e, CpeForgeError::UnsupportedDocument)
        }),
        (
            r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><cbc:UBLVersionID>3.0</cbc:UBLVersionID></Invoice>"#,
            |e| matches!(e, CpeForgeError::Service(WsError::XmlReader(_))),
        ),
        ("<!DOCTYPE x><Invoice/>", |e| {
            matches!(e, CpeForgeError::Service(WsError::Xml(_)))
        }),
    ];
    for (xml, expected) in cases {
        let error = facade.send_xml_file(xml).await.unwrap_err();
        assert!(expected(&error), "{xml}: {error:?}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let ok = r#"<SummaryDocuments xmlns="urn:sunat:names:specification:ubl:peru:schema:xsd:SummaryDocuments-1" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"><cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:ID>RC-20240131-1</cbc:ID><cac:AccountingSupplierParty><cbc:CustomerAssignedAccountID>20000000001</cbc:CustomerAssignedAccountID></cac:AccountingSupplierParty></SummaryDocuments>"#;
    assert!(matches!(
        facade.send_xml_file(ok).await,
        Ok(SendResult::Summary(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Reloj controlable para vencer el token GRE.
#[derive(Clone)]
struct ManualClock(Arc<AtomicU64>);
impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.0.load(Ordering::SeqCst))
    }
}

#[derive(Clone, Default)]
struct Gre {
    auth: Arc<AtomicUsize>,
    status: Arc<AtomicUsize>,
}
impl HttpTransport for Gre {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url.contains("/auth") {
            self.auth.fetch_add(1, Ordering::SeqCst);
            br#"{"access_token":"TOKEN-FICTICIO","expires_in":3600}"#.to_vec()
        } else {
            self.status.fetch_add(1, Ordering::SeqCst);
            br#"{"codRespuesta":"98"}"#.to_vec()
        };
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![],
            body,
        })
    }
}

/// El token se reutiliza mientras está vigente y se renueva al vencer.
#[tokio::test]
async fn expired_gre_token_is_renewed_once() {
    let transport = Gre::default();
    let clock = ManualClock(Arc::new(AtomicU64::new(1_700_000_000)));
    let factory = ApiFactory::new(
        transport.clone(),
        Configuration::auth().with_host("http://127.0.0.1:9/auth"),
        "http://127.0.0.1:9/cpe",
        InMemoryStore::default(),
        clock.clone(),
    );
    let mut api = Api::new(factory);
    api.set_api_credentials("client.test", "secret.test")
        .set_clave_sol("20000000001", "TEST", "password.test");
    for _ in 0..3 {
        assert_eq!(
            api.get_status("T-1").await.unwrap().code.as_deref(),
            Some("98")
        );
    }
    assert_eq!(transport.auth.load(Ordering::SeqCst), 1);
    clock.0.fetch_add(3600 * 2, Ordering::SeqCst);
    assert_eq!(
        api.get_status("T-1").await.unwrap().code.as_deref(),
        Some("98")
    );
    assert_eq!(transport.auth.load(Ordering::SeqCst), 2);
    assert_eq!(transport.status.load(Ordering::SeqCst), 4);
}
