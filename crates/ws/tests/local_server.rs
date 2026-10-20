//! Integración local: `SoapClient` con el transporte HTTP real
//! (`ReqwestTransport`) contra un servidor SOAP en proceso que responde como
//! los servicios de SUNAT. Corre en `cargo test` sin red externa ni oráculo:
//! solo escucha en 127.0.0.1.
//!
//! Cubre las cuatro operaciones (`sendBill`, `sendSummary`, `getStatus`,
//! `getStatusCdr`), los resultados tributarios (aceptado, observado,
//! rechazado, excepción, en proceso), los faults SOAP y los fallos HTTP y de
//! transporte, y comprueba la solicitud que recibe el servidor (operación,
//! WS-Security y ZIP enviado).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use cpeforge_gre_api::ReqwestTransport;
use cpeforge_ws::WsError;
use cpeforge_ws::outcome::TaxStatus;
use cpeforge_ws::services::{BillSender, ConsultCdrService, ExtService, SummarySender};
use cpeforge_ws::soap::SoapClient;
use cpeforge_ws::zip::{compress, decompress};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const SOAP: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const SERVICE: &str = "http://service.sunat.gob.pe";
const AR: &str = "urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2";
const CAC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2";
const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const USER: &str = "20000000001MODDATOS";
const PASSWORD: &str = "CLAVE-FICTICIA";

/// Respuesta guionada para una ruta.
#[derive(Clone)]
enum Reply {
    Http {
        status: u16,
        content_type: &'static str,
        body: Vec<u8>,
    },
    /// Cierra la conexión sin responder.
    Close,
    /// Responde 200 vacío después de la pausa.
    Delay(Duration),
}

/// Solicitud recibida por el servidor.
#[derive(Clone, Debug)]
struct Received {
    path: String,
    headers: String,
    body: String,
}

#[derive(Clone, Default)]
struct Server {
    replies: Arc<Mutex<HashMap<String, Reply>>>,
    received: Arc<Mutex<Vec<Received>>>,
}

impl Server {
    async fn start(routes: Vec<(&str, Reply)>) -> (Self, String) {
        let server = Self::default();
        {
            let mut replies = server.replies.lock().unwrap();
            for (path, reply) in routes {
                replies.insert(format!("/{path}"), reply);
            }
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = server.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = state.clone();
                tokio::spawn(async move { state.handle(stream).await });
            }
        });
        (server, base)
    }

    async fn handle(&self, mut stream: TcpStream) {
        let mut data = Vec::new();
        let mut buf = [0u8; 8192];
        let header_end = loop {
            let Ok(n) = stream.read(&mut buf).await else {
                return;
            };
            if n == 0 {
                return;
            }
            data.extend_from_slice(&buf[..n]);
            if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
        };
        let head = String::from_utf8_lossy(&data[..header_end]).into_owned();
        let length = head
            .lines()
            .find_map(|l| {
                let (name, value) = l.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        while data.len() < header_end + length {
            let Ok(n) = stream.read(&mut buf).await else {
                return;
            };
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
        }
        let path = head
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_owned();
        self.received.lock().unwrap().push(Received {
            path: path.clone(),
            headers: head.clone(),
            body: String::from_utf8_lossy(&data[header_end..]).into_owned(),
        });
        let reply = self.replies.lock().unwrap().get(&path).cloned();
        let (status, content_type, body) = match reply {
            Some(Reply::Http {
                status,
                content_type,
                body,
            }) => (status, content_type, body),
            Some(Reply::Close) => return,
            Some(Reply::Delay(pause)) => {
                tokio::time::sleep(pause).await;
                (200, "text/xml; charset=utf-8", Vec::new())
            }
            None => (404, "text/plain", b"sin ruta".to_vec()),
        };
        let reason = match status {
            200 => "OK",
            401 => "Unauthorized",
            404 => "Not Found",
            _ => "Internal Server Error",
        };
        let head = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes()).await;
        let _ = stream.write_all(&body).await;
        let _ = stream.shutdown().await;
    }

    fn received(&self, path: &str) -> Vec<Received> {
        self.received
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.path == format!("/{path}"))
            .cloned()
            .collect()
    }
}

fn cdr_xml(code: &str, notes: &[&str]) -> String {
    let notes: String = notes
        .iter()
        .map(|n| format!("<cbc:Note>{n}</cbc:Note>"))
        .collect();
    format!(
        "<ApplicationResponse xmlns=\"{AR}\" xmlns:cac=\"{CAC}\" xmlns:cbc=\"{CBC}\">\
         <cbc:ID>123</cbc:ID>{notes}<cac:DocumentResponse><cac:Response>\
         <cbc:ReferenceID>F001-1</cbc:ReferenceID><cbc:ResponseCode>{code}</cbc:ResponseCode>\
         <cbc:Description>Respuesta ficticia {code}</cbc:Description></cac:Response>\
         </cac:DocumentResponse></ApplicationResponse>"
    )
}

fn cdr_b64(code: &str, notes: &[&str]) -> String {
    let zip = compress(
        "R-20000000001-01-F001-1.xml",
        cdr_xml(code, notes).as_bytes(),
    )
    .unwrap();
    base64::engine::general_purpose::STANDARD.encode(zip)
}

fn envelope(body: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><soap:Envelope xmlns:soap=\"{SOAP}\">\
         <soap:Body>{body}</soap:Body></soap:Envelope>"
    )
    .into_bytes()
}

fn ok(body: &str) -> Reply {
    Reply::Http {
        status: 200,
        content_type: "text/xml; charset=utf-8",
        body: envelope(body),
    }
}

fn bill_reply(code: &str, notes: &[&str]) -> Reply {
    ok(&format!(
        "<ns:sendBillResponse xmlns:ns=\"{SERVICE}\"><applicationResponse>{}</applicationResponse></ns:sendBillResponse>",
        cdr_b64(code, notes)
    ))
}

fn status_reply(operation: &str, element: &str, code: &str, content: Option<String>) -> Reply {
    let content = content.map_or(String::new(), |c| format!("<content>{c}</content>"));
    ok(&format!(
        "<ns:{operation}Response xmlns:ns=\"{SERVICE}\"><{element}><statusCode>{code}</statusCode>\
         <statusMessage>Mensaje {code}</statusMessage>{content}</{element}></ns:{operation}Response>"
    ))
}

fn fault(status: u16, code: &str, message: &str) -> Reply {
    Reply::Http {
        status,
        content_type: "text/xml; charset=utf-8",
        body: envelope(&format!(
            "<soap:Fault><faultcode>{code}</faultcode><faultstring>{message}</faultstring></soap:Fault>"
        )),
    }
}

fn client(transport: &ReqwestTransport, base: &str, path: &str) -> SoapClient<ReqwestTransport> {
    SoapClient::new(transport.clone(), format!("{base}/{path}"), USER, PASSWORD)
}

const INVOICE: &[u8] =
    b"<Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\"/>";

#[tokio::test]
async fn send_bill_outcomes_over_http() {
    let (server, base) = Server::start(vec![
        ("accepted", bill_reply("0", &[])),
        (
            "observed",
            bill_reply("0", &["4252 - Observación ficticia"]),
        ),
        ("rejected", bill_reply("2335", &[])),
        (
            "fault",
            fault(500, "soap:Client.0111", "No tiene el perfil"),
        ),
        (
            "no-cdr",
            ok(&format!("<ns:sendBillResponse xmlns:ns=\"{SERVICE}\"/>")),
        ),
    ])
    .await;
    let transport = ReqwestTransport::new().unwrap();
    let send = |path: &'static str| {
        let client = client(&transport, &base, path);
        async move {
            BillSender::new(client)
                .send("20000000001-01-F001-1", INVOICE)
                .await
                .unwrap()
        }
    };

    let accepted = send("accepted").await;
    assert_eq!(accepted.base.success, Some(true));
    assert_eq!(TaxStatus::of_bill(&accepted), TaxStatus::Accepted);
    let cdr = accepted.cdr_response.as_ref().unwrap();
    assert_eq!(cdr.code.as_deref(), Some("0"));
    assert_eq!(cdr.id.as_deref(), Some("F001-1"));

    assert_eq!(
        TaxStatus::of_bill(&send("observed").await),
        TaxStatus::AcceptedWithObservations
    );
    assert_eq!(
        TaxStatus::of_bill(&send("rejected").await),
        TaxStatus::Rejected
    );

    let faulted = send("fault").await;
    assert_ne!(faulted.base.success, Some(true));
    let error = faulted.base.error.as_ref().unwrap();
    assert_eq!(error.code.as_deref(), Some("0111"));
    assert_eq!(TaxStatus::of_bill(&faulted), TaxStatus::Exception);

    let no_cdr = send("no-cdr").await;
    assert_eq!(
        no_cdr.base.error.as_ref().unwrap().code.as_deref(),
        Some("CDR")
    );
    assert_eq!(TaxStatus::of_bill(&no_cdr), TaxStatus::Indeterminate);

    // La solicitud llega con la operación, WS-Security y el ZIP del XML.
    let request = &server.received("accepted")[0];
    assert!(request.headers.starts_with("POST /accepted HTTP/1.1"));
    assert!(request.body.contains("<ns:sendBill>"), "{}", request.body);
    assert!(
        request
            .body
            .contains(&format!("<wsse:Username>{USER}</wsse:Username>"))
    );
    assert!(
        request
            .body
            .contains(&format!(">{PASSWORD}</wsse:Password>"))
    );
    assert!(
        request
            .body
            .contains("<fileName>20000000001-01-F001-1.zip</fileName>")
    );
    let content = request
        .body
        .split("<contentFile>")
        .nth(1)
        .and_then(|rest| rest.split("</contentFile>").next())
        .unwrap();
    let zip = base64::engine::general_purpose::STANDARD
        .decode(content)
        .unwrap();
    let entry = decompress(&zip).unwrap().into_iter().next().unwrap();
    assert_eq!(entry.filename, "20000000001-01-F001-1.xml");
    assert_eq!(entry.content, INVOICE);
}

#[tokio::test]
async fn summary_ticket_status_and_consult_over_http() {
    let (server, base) = Server::start(vec![
        (
            "summary",
            ok(&format!(
                "<ns:sendSummaryResponse xmlns:ns=\"{SERVICE}\"><ticket>1700000000001</ticket></ns:sendSummaryResponse>"
            )),
        ),
        ("pending", status_reply("getStatus", "status", "98", None)),
        (
            "done",
            status_reply("getStatus", "status", "0", Some(cdr_b64("0", &[]))),
        ),
        (
            "done-rejected",
            status_reply("getStatus", "status", "99", Some(cdr_b64("2223", &[]))),
        ),
        ("exception", status_reply("getStatus", "status", "0127", None)),
        (
            "consult",
            status_reply("getStatusCdr", "statusCdr", "0004", Some(cdr_b64("0", &[]))),
        ),
        (
            "consult-missing",
            status_reply("getStatusCdr", "statusCdr", "0127", None),
        ),
    ])
    .await;
    let transport = ReqwestTransport::new().unwrap();

    let summary = SummarySender::new(client(&transport, &base, "summary"))
        .send("20000000001-RC-20240101-1", b"<SummaryDocuments/>")
        .await
        .unwrap();
    assert_eq!(summary.ticket.as_deref(), Some("1700000000001"));
    assert_eq!(TaxStatus::of_summary(&summary), TaxStatus::Pending);
    let request = &server.received("summary")[0];
    assert!(request.body.contains("<ns:sendSummary>"));
    assert!(
        request
            .body
            .contains("<fileName>20000000001-RC-20240101-1.zip</fileName>")
    );

    let status = |path: &'static str| {
        let client = client(&transport, &base, path);
        async move {
            ExtService::new(client)
                .get_status("1700000000001")
                .await
                .unwrap()
        }
    };
    assert_eq!(
        TaxStatus::of_status(&status("pending").await),
        TaxStatus::Pending
    );
    let done = status("done").await;
    assert_eq!(done.bill.base.success, Some(true));
    assert_eq!(TaxStatus::of_status(&done), TaxStatus::Accepted);
    assert_eq!(
        TaxStatus::of_status(&status("done-rejected").await),
        TaxStatus::Rejected
    );
    assert_eq!(
        TaxStatus::of_status(&status("exception").await),
        TaxStatus::Exception
    );
    assert!(
        server.received("pending")[0]
            .body
            .contains("<ticket>1700000000001</ticket>")
    );

    let consult = ConsultCdrService::new(client(&transport, &base, "consult"))
        .get_status_cdr("20000000001", "01", "F001", 1)
        .await
        .unwrap();
    assert_eq!(consult.code.as_deref(), Some("0004"));
    assert_eq!(consult.bill.base.success, Some(true));
    assert_eq!(
        consult.bill.cdr_response.as_ref().unwrap().code.as_deref(),
        Some("0")
    );
    let request = &server.received("consult")[0];
    assert!(
        request.body.contains("<ns:getStatusCdr>"),
        "{}",
        request.body
    );
    for field in [
        "<rucComprobante>20000000001</rucComprobante>",
        "<tipoComprobante>01</tipoComprobante>",
        "<serieComprobante>F001</serieComprobante>",
        "<numeroComprobante>1</numeroComprobante>",
    ] {
        assert!(request.body.contains(field), "{field}: {}", request.body);
    }

    let missing = ConsultCdrService::new(client(&transport, &base, "consult-missing"))
        .get_status_cdr("20000000001", "01", "F001", 9)
        .await
        .unwrap();
    assert_eq!(missing.bill.base.success, Some(false));
    assert_eq!(
        missing.bill.base.error.as_ref().unwrap().code.as_deref(),
        Some("0127")
    );
}

#[tokio::test]
async fn http_and_transport_failures_are_typed() {
    let (_server, base) = Server::start(vec![
        (
            "html-401",
            Reply::Http {
                status: 401,
                content_type: "text/html",
                body: b"<html><body>Unauthorized</body></html>".to_vec(),
            },
        ),
        (
            "empty",
            Reply::Http {
                status: 200,
                content_type: "text/xml",
                body: Vec::new(),
            },
        ),
        (
            "garbage",
            Reply::Http {
                status: 200,
                content_type: "text/xml",
                body: b"esto no es XML".to_vec(),
            },
        ),
        ("close", Reply::Close),
        ("slow", Reply::Delay(Duration::from_secs(3))),
    ])
    .await;
    let transport = ReqwestTransport::new().unwrap();
    let send = |transport: ReqwestTransport, path: &'static str| {
        let client = client(&transport, &base, path);
        async move {
            BillSender::new(client)
                .send("20000000001-01-F001-1", INVOICE)
                .await
        }
    };

    // Un 401 con HTML es un fault HTTP del cliente SOAP, no un pánico.
    let unauthorized = send(transport.clone(), "html-401").await.unwrap();
    assert!(unauthorized.base.error.is_some());
    assert_eq!(TaxStatus::of_bill(&unauthorized), TaxStatus::Indeterminate);

    // Respuesta vacía: sin CDR.
    let empty = send(transport.clone(), "empty").await.unwrap();
    assert_eq!(
        empty.base.error.as_ref().unwrap().code.as_deref(),
        Some("CDR")
    );

    // Texto que no es XML: documento ilegible, informado como fault.
    let garbage = send(transport.clone(), "garbage").await.unwrap();
    assert_eq!(
        garbage.base.error.as_ref().unwrap().message.as_deref(),
        Some("looks like we got no XML document")
    );

    // Conexión cerrada y tiempo agotado: error de transporte (el envío pudo
    // llegar a SUNAT; hay que consultar antes de reenviar).
    assert!(matches!(
        send(transport.clone(), "close").await,
        Err(WsError::Transport)
    ));
    let impatient = ReqwestTransport::builder()
        .timeout(Duration::from_millis(300))
        .build()
        .unwrap();
    assert!(matches!(
        send(impatient, "slow").await,
        Err(WsError::Transport)
    ));
}
