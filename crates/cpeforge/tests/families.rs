//! Las nueve familias de documentos de extremo a extremo por las fachadas, con
//! un transporte local que responde como SUNAT: construir, firmar,
//! comprimir, enviar y leer el CDR o el ticket. Incluye faults, CDR de
//! rechazo y consulta de tickets, sin red.
//!
//! - `See` (SOAP): factura, boleta, notas de crédito y débito, percepción,
//!   retención, liquidación de compra (CDR inmediato) y resumen, baja y
//!   reversión (ticket).
//! - `Api` (GRE): guía de remisión 2022 (ticket y consulta con CDR).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use cpeforge::{Api, DocumentRef, See, SendResult};
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::{Despatch, DespatchDetail, Direction, Shipment, Transportist};
use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use cpeforge_core::model::retention::{Payment, Retention, RetentionDetail};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, Note, SaleDetail};
use cpeforge_core::model::settlement::{
    PurchaseSettlement, SettlementDetail, SettlementPlace, SettlementSeller,
};
use cpeforge_core::model::summary::{Summary, SummaryDetail};
use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};
use cpeforge_core::{Decimal, Zoned};
use cpeforge_gre_api::{Configuration, HttpRequest, HttpResponse, HttpTransport, TransportError};
use cpeforge_ws::api::{ApiFactory, InMemoryStore, SystemClock};
use cpeforge_ws::outcome::TaxStatus;
use cpeforge_ws::zip::{ZipTime, compress, decompress};

const AR: &str = "urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2";
const CAC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2";
const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const SOAP: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const SERVICE: &str = "http://service.sunat.gob.pe";

// --- Modelos sintéticos --------------------------------------------------

fn dec(text: &str) -> Decimal {
    Decimal::from_str(text).unwrap()
}

fn lima(day: &str) -> Zoned {
    format!("{day}T10:00:00-05:00[America/Lima]")
        .parse()
        .unwrap()
}

fn company() -> Company {
    Company {
        ruc: Some("20000000001".into()),
        razon_social: Some("EMPRESA SINTETICA S.A.C.".into()),
        nombre_comercial: Some("COMERCIAL SINTETICO".into()),
        address: Some(Address {
            ubigueo: Some("150101".into()),
            departamento: Some("LIMA".into()),
            provincia: Some("LIMA".into()),
            distrito: Some("LIMA".into()),
            direccion: Some("AV. SINTETICA 123".into()),
            ..Address::default()
        }),
        ..Company::default()
    }
}

fn client() -> Client {
    Client {
        tipo_doc: Some("6".into()),
        num_doc: Some("20000000002".into()),
        rzn_social: Some("CLIENTE SINTETICO S.A.".into()),
        ..Client::default()
    }
}

fn sale(tipo_doc: &str, serie: &str) -> BaseSale {
    BaseSale {
        ubl_version: Some("2.1".into()),
        tipo_doc: Some(tipo_doc.into()),
        serie: Some(serie.into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(lima("2024-01-15")),
        tipo_moneda: Some("PEN".into()),
        company: Some(company()),
        client: Some(client()),
        mto_oper_gravadas: Some(dec("100.00")),
        mto_igv: Some(dec("18.00")),
        total_impuestos: Some(dec("18.00")),
        mto_imp_venta: Some(dec("118.00")),
        legends: Some(vec![Legend {
            code: Some("1000".into()),
            value: Some("SON CIENTO DIECIOCHO CON 00/100 SOLES".into()),
        }]),
        details: Some(vec![SaleDetail {
            cod_producto: Some("P001".into()),
            unidad: Some("NIU".into()),
            cantidad: Some(dec("1")),
            descripcion: Some("PRODUCTO SINTETICO".into()),
            mto_valor_unitario: Some(dec("100.00")),
            mto_base_igv: Some(dec("100.00")),
            porcentaje_igv: Some(dec("18")),
            igv: Some(dec("18.00")),
            tip_afe_igv: Some("10".into()),
            total_impuestos: Some(dec("18.00")),
            mto_valor_venta: Some(dec("100.00")),
            mto_precio_unitario: Some(dec("118.00")),
            ..SaleDetail::default()
        }]),
        ..BaseSale::default()
    }
}

fn invoice(tipo_doc: &str, serie: &str) -> Invoice {
    Invoice {
        base: sale(tipo_doc, serie),
        tipo_operacion: Some("0101".into()),
        ..Invoice::default()
    }
}

fn note(tipo_doc: &str, serie: &str) -> Note {
    Note {
        base: sale(tipo_doc, serie),
        cod_motivo: Some("01".into()),
        des_motivo: Some("ANULACION".into()),
        tip_doc_afectado: Some("01".into()),
        num_doc_afectado: Some("F001-1".into()),
        ..Note::default()
    }
}

fn summary() -> Summary {
    Summary {
        correlativo: Some("1".into()),
        fec_generacion: Some(lima("2024-05-31")),
        fec_resumen: Some(lima("2024-06-01")),
        company: Some(company()),
        details: Some(vec![SummaryDetail {
            tipo_doc: Some("03".into()),
            serie_nro: Some("B001-1".into()),
            cliente_tipo: Some("1".into()),
            cliente_nro: Some("00000001".into()),
            estado: Some("1".into()),
            total: Some(dec("118")),
            mto_oper_gravadas: Some(dec("100")),
            mto_igv: Some(dec("18")),
            ..SummaryDetail::default()
        }]),
        ..Summary::default()
    }
}

fn voided() -> Voided {
    Voided {
        correlativo: Some("1".into()),
        fec_generacion: Some(lima("2024-05-30")),
        fec_comunicacion: Some(lima("2024-06-01")),
        company: Some(company()),
        details: Some(vec![VoidedDetail {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("1".into()),
            des_motivo_baja: Some("ERROR".into()),
        }]),
    }
}

fn payment() -> Payment {
    Payment {
        moneda: Some("PEN".into()),
        importe: Some(dec("100")),
        fecha: Some(lima("2024-07-01")),
    }
}

fn retention() -> Retention {
    Retention {
        serie: Some("R001".into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(lima("2024-07-03")),
        company: Some(company()),
        proveedor: Some(client()),
        regimen: Some("01".into()),
        tasa: Some(dec("3")),
        imp_retenido: Some(dec("3")),
        imp_pagado: Some(dec("97")),
        details: Some(vec![RetentionDetail {
            tipo_doc: Some("01".into()),
            num_doc: Some("F001-1".into()),
            fecha_emision: Some(lima("2024-06-20")),
            imp_total: Some(dec("100")),
            moneda: Some("PEN".into()),
            pagos: Some(vec![payment()]),
            fecha_retencion: Some(lima("2024-07-02")),
            imp_retenido: Some(dec("3")),
            imp_pagar: Some(dec("97")),
            tipo_cambio: None,
        }]),
        ..Retention::default()
    }
}

fn settlement() -> PurchaseSettlement {
    PurchaseSettlement {
        tipo_operacion: Some("0501".into()),
        serie: Some("L001".into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(lima("2024-07-03")),
        tipo_moneda: Some("PEN".into()),
        company: Some(company()),
        vendedor: Some(SettlementSeller {
            tipo_doc: Some("1".into()),
            num_doc: Some("40123456".into()),
            rzn_social: Some("QUISPE JUAN".into()),
            address: Some(Address::default()),
            condicion: Some("02".into()),
        }),
        lugar_operacion: Some(SettlementPlace {
            address: Some(Address::default()),
            condicion: Some("01".into()),
        }),
        mto_oper_gravadas: Some(dec("100")),
        mto_igv: Some(dec("18")),
        mto_base_ir: Some(dec("100")),
        mto_ir: Some(dec("1.5")),
        total_impuestos: Some(dec("18")),
        valor_venta: Some(dec("100")),
        sub_total: Some(dec("118")),
        mto_imp_venta: Some(dec("98.5")),
        details: Some(vec![SettlementDetail {
            unidad: Some("KGM".into()),
            cantidad: Some(dec("10")),
            cod_prod_sunat: Some("10101501".into()),
            descripcion: Some("CAFÉ".into()),
            mto_valor_unitario: Some(dec("10")),
            mto_precio_unitario: Some(dec("11.8")),
            mto_valor_venta: Some(dec("100")),
            tip_afe_igv: Some("10".into()),
            mto_base_igv: Some(dec("100")),
            porcentaje_igv: Some(dec("18")),
            igv: Some(dec("18")),
            mto_base_ir: Some(dec("100")),
            porcentaje_ir: Some(dec("1.5")),
            ir: Some(dec("1.5")),
            total_impuestos: Some(dec("18")),
            ..SettlementDetail::default()
        }]),
        ..PurchaseSettlement::default()
    }
}

fn perception() -> Perception {
    Perception {
        serie: Some("P001".into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(lima("2024-07-03")),
        company: Some(company()),
        proveedor: Some(client()),
        regimen: Some("01".into()),
        tasa: Some(dec("2")),
        imp_percibido: Some(dec("2")),
        imp_cobrado: Some(dec("102")),
        details: Some(vec![PerceptionDetail {
            tipo_doc: Some("01".into()),
            num_doc: Some("F001-1".into()),
            fecha_emision: Some(lima("2024-06-20")),
            imp_total: Some(dec("100")),
            moneda: Some("PEN".into()),
            cobros: Some(vec![payment()]),
            fecha_percepcion: Some(lima("2024-07-02")),
            imp_percibido: Some(dec("2")),
            imp_cobrar: Some(dec("102")),
            tipo_cambio: None,
        }]),
        ..Perception::default()
    }
}

fn despatch() -> Despatch {
    Despatch {
        version: Some("2022".into()),
        tipo_doc: Some("09".into()),
        serie: Some("T001".into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(lima("2024-04-01")),
        company: Some(company()),
        destinatario: Some(client()),
        envio: Some(Shipment {
            cod_traslado: Some("01".into()),
            mod_traslado: Some("01".into()),
            fec_traslado: Some(lima("2024-04-02")),
            peso_total: Some(dec("12.5")),
            und_peso_total: Some("KGM".into()),
            llegada: Some(Direction {
                ubigueo: Some("150203".into()),
                direccion: Some("JR. LLEGADA 1".into()),
                ..Direction::default()
            }),
            partida: Some(Direction {
                ubigueo: Some("150101".into()),
                direccion: Some("AV. PARTIDA 9".into()),
                ..Direction::default()
            }),
            transportista: Some(Transportist {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000004".into()),
                rzn_social: Some("TRANSPORTES SINTETICOS".into()),
                ..Transportist::default()
            }),
            ..Shipment::default()
        }),
        details: Some(vec![DespatchDetail {
            cantidad: Some(dec("1")),
            unidad: Some("NIU".into()),
            descripcion: Some("BIEN SINTETICO".into()),
            codigo: Some("B01".into()),
            ..DespatchDetail::default()
        }]),
        ..Despatch::default()
    }
}

// --- Certificado efímero -------------------------------------------------

fn ephemeral_pem() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "cpeforge-families-{}-{}",
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

// --- Respuestas de SUNAT -------------------------------------------------

fn cdr_zip_b64(reference: &str, code: &str) -> String {
    let xml = format!(
        "<ApplicationResponse xmlns=\"{AR}\" xmlns:cac=\"{CAC}\" xmlns:cbc=\"{CBC}\">\
         <cbc:ID>1</cbc:ID><cac:DocumentResponse><cac:Response>\
         <cbc:ReferenceID>{reference}</cbc:ReferenceID><cbc:ResponseCode>{code}</cbc:ResponseCode>\
         <cbc:Description>Respuesta {code}</cbc:Description></cac:Response>\
         </cac:DocumentResponse></ApplicationResponse>"
    );
    let zip = compress(&format!("R-{reference}.xml"), xml.as_bytes()).unwrap();
    base64::engine::general_purpose::STANDARD.encode(zip)
}

fn envelope(body: &str) -> Vec<u8> {
    format!("<soap:Envelope xmlns:soap=\"{SOAP}\"><soap:Body>{body}</soap:Body></soap:Envelope>")
        .into_bytes()
}

/// Lo que recibió el transporte en un `sendBill` o `sendSummary`.
#[derive(Clone, Debug)]
struct Sent {
    operation: &'static str,
    file_name: String,
    xml: String,
}

/// Transporte SOAP local: responde según la operación y el código de CDR
/// configurado, y guarda el ZIP recibido.
#[derive(Clone)]
struct Sunat {
    cdr_code: &'static str,
    fault: Option<&'static str>,
    sent: Arc<Mutex<Vec<Sent>>>,
}

impl Sunat {
    fn new(cdr_code: &'static str) -> Self {
        Self {
            cdr_code,
            fault: None,
            sent: Arc::default(),
        }
    }

    fn last(&self) -> Sent {
        self.sent.lock().unwrap().last().cloned().unwrap()
    }
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let rest = text.split_once(start)?.1;
    Some(rest.split_once(end)?.0)
}

impl HttpTransport for Sunat {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = String::from_utf8(request.body.clone()).unwrap();
        let reply = |status, body| {
            Ok(HttpResponse {
                status,
                reason: "OK".into(),
                headers: vec![("content-type".into(), "text/xml; charset=utf-8".into())],
                body,
            })
        };
        if let Some(code) = self.fault {
            return reply(
                500,
                envelope(&format!(
                    "<soap:Fault><faultcode>{code}</faultcode><faultstring>Fault ficticio</faultstring></soap:Fault>"
                )),
            );
        }
        let operation = if body.contains("<ns:sendBill>") {
            "sendBill"
        } else if body.contains("<ns:sendSummary>") {
            "sendSummary"
        } else if body.contains("<ns:getStatus>") {
            return reply(
                200,
                envelope(&format!(
                    "<ns:getStatusResponse xmlns:ns=\"{SERVICE}\"><status><statusCode>99</statusCode>\
                     <content>{}</content></status></ns:getStatusResponse>",
                    cdr_zip_b64("RC-20240601-1", self.cdr_code)
                )),
            );
        } else {
            panic!("operación inesperada: {body}");
        };
        let file_name = between(&body, "<fileName>", "</fileName>")
            .unwrap()
            .to_owned();
        let zip = base64::engine::general_purpose::STANDARD
            .decode(between(&body, "<contentFile>", "</contentFile>").unwrap())
            .unwrap();
        let entry = decompress(&zip).unwrap().into_iter().next().unwrap();
        let xml = String::from_utf8(entry.content).unwrap();
        self.sent.lock().unwrap().push(Sent {
            operation,
            file_name: file_name.clone(),
            xml,
        });
        let base = file_name.trim_end_matches(".zip");
        let response = if operation == "sendBill" {
            format!(
                "<ns:sendBillResponse xmlns:ns=\"{SERVICE}\"><applicationResponse>{}</applicationResponse></ns:sendBillResponse>",
                cdr_zip_b64(base, self.cdr_code)
            )
        } else {
            format!(
                "<ns:sendSummaryResponse xmlns:ns=\"{SERVICE}\"><ticket>T-{base}</ticket></ns:sendSummaryResponse>"
            )
        };
        reply(200, envelope(&response))
    }
}

fn see(pem: &str, transport: Sunat) -> See<Sunat> {
    let mut see = See::new(transport);
    see.set_service("http://127.0.0.1:9/soap")
        .set_clave_sol("20000000001", "MODDATOS", "moddatos")
        .set_zip_time(ZipTime::default());
    see.set_certificate(pem).unwrap();
    see
}

// --- Pruebas ---------------------------------------------------------------

/// Familias SOAP: cada una sale con su nombre de archivo, el XML firmado de su
/// raíz y el resultado que corresponde (CDR o ticket).
#[tokio::test]
async fn every_soap_family_is_signed_sent_and_read() {
    let pem = ephemeral_pem();
    let transport = Sunat::new("0");
    let mut facade = see(&pem, transport.clone());

    let factura = invoice("01", "F001");
    let boleta = invoice("03", "B001");
    let credito = note("07", "FC01");
    let debito = note("08", "FD01");
    let percepcion = perception();
    let retencion = retention();
    let liquidacion = settlement();
    let resumen = summary();
    let baja = voided();
    let reversion = Reversion {
        voided: Voided {
            details: Some(vec![VoidedDetail {
                tipo_doc: Some("20".into()),
                serie: Some("R001".into()),
                correlativo: Some("1".into()),
                des_motivo_baja: Some("ERROR".into()),
            }]),
            ..voided()
        },
    };

    let bills: [(DocumentRef<'_>, &str, &str); 7] = [
        (
            DocumentRef::Invoice(&factura),
            "20000000001-01-F001-1",
            "<Invoice ",
        ),
        (
            DocumentRef::Invoice(&boleta),
            "20000000001-03-B001-1",
            "<Invoice ",
        ),
        (
            DocumentRef::Note(&credito),
            "20000000001-07-FC01-1",
            "<CreditNote ",
        ),
        (
            DocumentRef::Note(&debito),
            "20000000001-08-FD01-1",
            "<DebitNote ",
        ),
        (
            DocumentRef::Perception(&percepcion),
            "20000000001-40-P001-1",
            "<Perception ",
        ),
        (
            DocumentRef::Retention(&retencion),
            "20000000001-20-R001-1",
            "<Retention ",
        ),
        (
            DocumentRef::Settlement(&liquidacion),
            "20000000001-04-L001-1",
            "<SelfBilledInvoice ",
        ),
    ];
    for (document, name, root) in bills {
        let SendResult::Bill(result) = facade.send(document).await.unwrap() else {
            panic!("{name}: se esperaba CDR");
        };
        assert_eq!(TaxStatus::of_bill(&result), TaxStatus::Accepted, "{name}");
        assert_eq!(
            result.cdr_response.as_ref().unwrap().id.as_deref(),
            Some(name),
            "{name}"
        );
        let sent = transport.last();
        assert_eq!(sent.operation, "sendBill", "{name}");
        assert_eq!(sent.file_name, format!("{name}.zip"));
        assert!(sent.xml.contains(root), "{name}: {}", sent.xml);
        assert!(
            sent.xml.contains("<ds:SignatureValue>"),
            "{name}: sin firma"
        );
        assert_eq!(facade_last_xml(&facade), sent.xml, "{name}");
    }

    let summaries: [(DocumentRef<'_>, &str, &str); 3] = [
        (
            DocumentRef::Summary(&resumen),
            "20000000001-RC-20240601-1",
            "<SummaryDocuments ",
        ),
        (
            DocumentRef::Voided(&baja),
            "20000000001-RA-20240601-1",
            "<VoidedDocuments ",
        ),
        (
            DocumentRef::Reversion(&reversion),
            "20000000001-RR-20240601-1",
            "<VoidedDocuments ",
        ),
    ];
    for (document, name, root) in summaries {
        let SendResult::Summary(result) = facade.send(document).await.unwrap() else {
            panic!("{name}: se esperaba ticket");
        };
        assert_eq!(result.ticket.as_deref(), Some(format!("T-{name}").as_str()));
        assert_eq!(TaxStatus::of_summary(&result), TaxStatus::Pending);
        let sent = transport.last();
        assert_eq!(sent.operation, "sendSummary", "{name}");
        assert_eq!(sent.file_name, format!("{name}.zip"));
        assert!(sent.xml.contains(root), "{name}: {}", sent.xml);
        assert!(
            sent.xml.contains("<ds:SignatureValue>"),
            "{name}: sin firma"
        );
    }
    assert_eq!(transport.sent.lock().unwrap().len(), 10);
}

fn facade_last_xml(facade: &See<Sunat>) -> String {
    facade
        .factory_ref()
        .last_xml()
        .unwrap_or_default()
        .to_owned()
}

/// Rechazo por CDR, fault SOAP en envíos con CDR y con ticket, y consulta de
/// un ticket procesado con rechazo.
#[tokio::test]
async fn rejections_faults_and_ticket_status() {
    let pem = ephemeral_pem();
    let factura = invoice("01", "F001");
    let resumen = summary();

    let rejected = Sunat::new("2335");
    let mut facade = see(&pem, rejected);
    let SendResult::Bill(result) = facade.send(DocumentRef::Invoice(&factura)).await.unwrap()
    else {
        panic!("se esperaba CDR");
    };
    assert_eq!(TaxStatus::of_bill(&result), TaxStatus::Rejected);
    let status = facade.get_status("T-1").await.unwrap();
    assert_eq!(status.code.as_deref(), Some("99"));
    assert_eq!(TaxStatus::of_status(&status), TaxStatus::Rejected);

    let mut faulted = Sunat::new("0");
    faulted.fault = Some("soap:Client.0111");
    let mut facade = see(&pem, faulted.clone());
    let SendResult::Bill(bill) = facade.send(DocumentRef::Invoice(&factura)).await.unwrap() else {
        panic!("se esperaba resultado de CDR");
    };
    assert_eq!(
        bill.base.error.as_ref().unwrap().code.as_deref(),
        Some("0111")
    );
    assert_eq!(TaxStatus::of_bill(&bill), TaxStatus::Exception);
    let SendResult::Summary(summary) = facade.send(DocumentRef::Summary(&resumen)).await.unwrap()
    else {
        panic!("se esperaba resultado de ticket");
    };
    assert_eq!(summary.ticket, None);
    assert_eq!(TaxStatus::of_summary(&summary), TaxStatus::Exception);
    // Aunque el envío falle, el XML firmado queda disponible para reintentar.
    assert!(facade_last_xml(&facade).contains("<SummaryDocuments "));
}

/// Transporte GRE local: token, envío y consulta con CDR.
#[derive(Clone, Default)]
struct Gre {
    sent: Arc<Mutex<Vec<String>>>,
}

impl HttpTransport for Gre {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url.contains("/auth") {
            br#"{"access_token":"TOKEN-FICTICIO","expires_in":3600}"#.to_vec()
        } else if !request.url.contains("/comprobantes/envios/") {
            // POST {host}/contribuyente/gem/comprobantes/{archivo}
            let json: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            self.sent.lock().unwrap().push(
                json["archivo"]["nomArchivo"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            );
            br#"{"numTicket":"7b0f0c7e-0000-0000-0000-000000000001","fecRecepcion":"2024-04-01T10:00:00"}"#
                .to_vec()
        } else {
            serde_json::json!({
                "codRespuesta": "0",
                "arcCdr": cdr_zip_b64("T001-1", "0"),
                "indCdrGenerado": "1",
            })
            .to_string()
            .into_bytes()
        };
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![],
            body,
        })
    }
}

/// Guía 2022 por la API GRE: ticket y consulta con CDR aceptado.
#[tokio::test]
async fn despatch_goes_through_gre_api() {
    let pem = ephemeral_pem();
    let transport = Gre::default();
    let factory = ApiFactory::new(
        transport.clone(),
        Configuration::auth().with_host("http://127.0.0.1:9/auth"),
        "http://127.0.0.1:9/cpe",
        InMemoryStore::default(),
        SystemClock,
    );
    let mut api = Api::new(factory);
    api.set_api_credentials("client.test", "secret.test")
        .set_clave_sol("20000000001", "MODDATOS", "moddatos")
        .set_certificate(&pem);
    let guia = despatch();
    let sent = api.send(DocumentRef::Despatch(&guia)).await.unwrap();
    assert_eq!(TaxStatus::of_summary(&sent), TaxStatus::Pending);
    assert_eq!(
        transport.sent.lock().unwrap().as_slice(),
        ["20000000001-09-T001-1.zip"]
    );
    assert!(api.last_xml().unwrap().contains("<DespatchAdvice "));
    let status = api
        .get_status(sent.ticket.as_deref().unwrap())
        .await
        .unwrap();
    assert_eq!(TaxStatus::of_status(&status), TaxStatus::Accepted);
}
