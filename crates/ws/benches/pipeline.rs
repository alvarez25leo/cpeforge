//! Benchmark local de `cpeforge-ws`, sin red ni dependencias extra.
//!
//! Mide por separado cada etapa del envío (ZIP, SHA-256, Base64, SOAP,
//! lectura de CDR, caché OAuth2, GRE) y el flujo completo con transportes
//! simulados, de modo que la latencia de SUNAT nunca entra en la medición.
//!
//! ```text
//! cargo bench -p cpeforge-ws --bench pipeline              # todo
//! cargo bench -p cpeforge-ws --bench pipeline -- zip       # filtro por nombre
//! ```
//!
//! Cada caso calienta, toma muestras durante un tiempo fijo y publica
//! p50/p95/p99 y operaciones por segundo. Los casos `concurrent/*` lanzan
//! N hilos, cada uno con su runtime `current_thread`, y miden throughput
//! agregado.

// Las estadísticas de tiempo usan `f64`; no hay importes en este binario.
#![allow(clippy::float_arithmetic)]

use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use cpeforge_gre_api::{
    Configuration, CpeApi, HttpRequest, HttpResponse, HttpTransport, Secret, TokenRequest,
    TransportError,
};
use cpeforge_ws::api::{ApiFactory, CachedToken, Clock, GreSender, InMemoryStore, TokenStore};
use cpeforge_ws::cdr::{read_cdr, read_cdr_zip};
use cpeforge_ws::services::BillSender;
use cpeforge_ws::soap::{SoapClient, WsClient};
use cpeforge_ws::zip::{DosDateTime, ZipTime, compress_at, decompress, first_xml};
use sha2::{Digest, Sha256};

const SIZES: [(&str, usize); 4] = [
    ("5KB", 5 * 1024),
    ("50KB", 50 * 1024),
    ("500KB", 500 * 1024),
    ("5MB", 5 * 1024 * 1024),
];
const SOAP_NS: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const SERVICE: &str = "http://service.sunat.gob.pe";

fn fixed_time() -> ZipTime {
    ZipTime::Fixed(DosDateTime::new(2026, 10, 17, 12, 0, 0).unwrap_or(DosDateTime::MIN))
}

/// Factura UBL 2.1 sintética de al menos `target` bytes, con líneas variadas
/// para que la compresión se parezca a la de un comprobante real.
fn invoice(target: usize) -> Vec<u8> {
    let mut xml = String::with_capacity(target + 2048);
    xml.push_str(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        "<Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\" ",
        "xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" ",
        "xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\">",
        "<cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:CustomizationID>2.0</cbc:CustomizationID>",
        "<cbc:ID>F001-123</cbc:ID><cbc:IssueDate>2026-10-17</cbc:IssueDate>",
        "<cbc:InvoiceTypeCode listID=\"0101\">01</cbc:InvoiceTypeCode>",
        "<cbc:DocumentCurrencyCode>PEN</cbc:DocumentCurrencyCode>",
        "<cac:AccountingSupplierParty><cac:Party><cac:PartyIdentification>",
        "<cbc:ID schemeID=\"6\">20000000001</cbc:ID></cac:PartyIdentification>",
        "<cac:PartyLegalEntity><cbc:RegistrationName>EMPRESA FICTICIA S.A.C.</cbc:RegistrationName>",
        "</cac:PartyLegalEntity></cac:Party></cac:AccountingSupplierParty>",
    ));
    let mut line = 1u64;
    while xml.len() < target {
        let qty = line % 97 + 1;
        let price = (line * 7919) % 100_000;
        xml.push_str(&format!(
            "<cac:InvoiceLine><cbc:ID>{line}</cbc:ID>\
             <cbc:InvoicedQuantity unitCode=\"NIU\">{qty}</cbc:InvoicedQuantity>\
             <cbc:LineExtensionAmount currencyID=\"PEN\">{}.{:02}</cbc:LineExtensionAmount>\
             <cac:TaxTotal><cbc:TaxAmount currencyID=\"PEN\">{}.{:02}</cbc:TaxAmount></cac:TaxTotal>\
             <cac:Item><cbc:Description>PRODUCTO FICTICIO {line} LOTE {:x}</cbc:Description>\
             <cac:SellersItemIdentification><cbc:ID>P{:06}</cbc:ID></cac:SellersItemIdentification></cac:Item>\
             <cac:Price><cbc:PriceAmount currencyID=\"PEN\">{}.{:02}</cbc:PriceAmount></cac:Price></cac:InvoiceLine>",
            price * qty / 100,
            price * qty % 100,
            price * qty * 18 / 10_000,
            price * qty * 18 / 100 % 100,
            line.wrapping_mul(0x9e37_79b9),
            line * 31 % 1_000_000,
            price / 100,
            price % 100,
        ));
        line += 1;
    }
    xml.push_str("</Invoice>");
    xml.into_bytes()
}

/// CDR sintético con la forma de los de SUNAT, incluida la firma.
fn cdr_xml() -> String {
    let signature = base64::engine::general_purpose::STANDARD.encode([0x5au8; 256]);
    let certificate = base64::engine::general_purpose::STANDARD.encode([0xa5u8; 1200]);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<ar:ApplicationResponse xmlns:ar=\"urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2\" \
xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" \
xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\" \
xmlns:ext=\"urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2\" \
xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\">\
<ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><ds:Signature Id=\"SignSUNAT\">\
<ds:SignedInfo><ds:CanonicalizationMethod Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\"/>\
<ds:SignatureMethod Algorithm=\"http://www.w3.org/2000/09/xmldsig#rsa-sha1\"/>\
<ds:Reference URI=\"\"><ds:Transforms><ds:Transform Algorithm=\"http://www.w3.org/2000/09/xmldsig#enveloped-signature\"/>\
</ds:Transforms><ds:DigestMethod Algorithm=\"http://www.w3.org/2000/09/xmldsig#sha1\"/>\
<ds:DigestValue>AAAAAAAAAAAAAAAAAAAAAAAAAAA=</ds:DigestValue></ds:Reference></ds:SignedInfo>\
<ds:SignatureValue>{signature}</ds:SignatureValue><ds:KeyInfo><ds:X509Data>\
<ds:X509Certificate>{certificate}</ds:X509Certificate></ds:X509Data></ds:KeyInfo></ds:Signature>\
</ext:ExtensionContent></ext:UBLExtension></ext:UBLExtensions>\
<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>1.0</cbc:CustomizationID>\
<cbc:ID>202600000000001</cbc:ID><cbc:IssueDate>2026-10-17</cbc:IssueDate><cbc:IssueTime>12:00:00</cbc:IssueTime>\
<cbc:ResponseDate>2026-10-17</cbc:ResponseDate><cbc:ResponseTime>12:00:01</cbc:ResponseTime>\
<cbc:Note>4252 - El dato ingresado como atributo @listName es incorrecto.</cbc:Note>\
<cac:Signature><cbc:ID>SignSUNAT</cbc:ID></cac:Signature>\
<cac:SenderParty><cac:PartyIdentification><cbc:ID>20131312955</cbc:ID></cac:PartyIdentification></cac:SenderParty>\
<cac:ReceiverParty><cac:PartyIdentification><cbc:ID>12-20000000001</cbc:ID></cac:PartyIdentification></cac:ReceiverParty>\
<cac:DocumentResponse><cac:Response><cbc:ReferenceID>F001-123</cbc:ReferenceID>\
<cbc:ResponseCode>0</cbc:ResponseCode><cbc:Description>La Factura numero F001-123, ha sido aceptada</cbc:Description>\
</cac:Response><cac:DocumentReference><cbc:ID>F001-123</cbc:ID></cac:DocumentReference>\
<cac:RecipientParty><cac:PartyIdentification><cbc:ID>6-20000000002</cbc:ID></cac:PartyIdentification></cac:RecipientParty>\
</cac:DocumentResponse></ar:ApplicationResponse>"
    )
}

/// ZIP de CDR como lo entrega SUNAT: entrada de directorio `dummy/` y el XML.
fn cdr_zip(xml: &str) -> Vec<u8> {
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let written = writer
        .add_directory("dummy/", options)
        .and_then(|()| writer.start_file("R-20000000001-01-F001-123.xml", options))
        .is_ok()
        && writer.write_all(xml.as_bytes()).is_ok();
    if !written {
        return Vec::new();
    }
    writer
        .finish()
        .map_or_else(|_| Vec::new(), std::io::Cursor::into_inner)
}

fn send_bill_reply(cdr_zip: &[u8]) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><soap:Envelope xmlns:soap=\"{SOAP_NS}\"><soap:Body>\
<ns:sendBillResponse xmlns:ns=\"{SERVICE}\"><applicationResponse>{}</applicationResponse>\
</ns:sendBillResponse></soap:Body></soap:Envelope>",
        base64::engine::general_purpose::STANDARD.encode(cdr_zip)
    )
    .into_bytes()
}

/// Fault con `detail` de `distinct` nombres distintos, cada uno repetido
/// `repeat` veces: ejercita la agrupación de propiedades del fault.
fn fault_reply(distinct: usize, repeat: usize) -> Vec<u8> {
    let mut detail = String::new();
    for r in 0..repeat {
        for d in 0..distinct {
            detail.push_str(&format!("<e{d}>v{r} &amp; x</e{d}>"));
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><soap:Envelope xmlns:soap=\"{SOAP_NS}\"><soap:Body>\
<soap:Fault><faultcode>soap:Client.0111</faultcode><faultstring>No tiene el perfil</faultstring>\
<detail>{detail}</detail></soap:Fault></soap:Body></soap:Envelope>"
    )
    .into_bytes()
}

#[derive(Clone)]
struct Canned {
    status: u16,
    content_type: &'static str,
    body: Arc<Vec<u8>>,
    calls: Arc<AtomicUsize>,
    /// Espera simulada antes de responder, para que las tareas se intercalen.
    delay: Option<Duration>,
}

impl Canned {
    fn new(status: u16, content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type,
            body: Arc::new(body),
            calls: Arc::new(AtomicUsize::new(0)),
            delay: None,
        }
    }
}

impl HttpTransport for Canned {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        black_box(&request);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        Ok(HttpResponse {
            status: self.status,
            reason: "OK".into(),
            headers: vec![("Content-Type".into(), self.content_type.into())],
            body: self.body.as_ref().clone(),
        })
    }
}

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_792_000_000)
    }
}

struct Stats {
    samples: Vec<Duration>,
}

impl Stats {
    fn percentile(&self, p: f64) -> Duration {
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let rank = ((sorted.len() as f64 - 1.0) * p).round() as usize;
        sorted.get(rank).copied().unwrap_or_default()
    }
}

fn fmt(d: Duration) -> String {
    let ns = d.as_nanos();
    if ns < 10_000 {
        format!("{ns} ns")
    } else if ns < 10_000_000 {
        format!("{:.1} µs", ns as f64 / 1e3)
    } else {
        format!("{:.2} ms", ns as f64 / 1e6)
    }
}

fn budget() -> Duration {
    std::env::var("WS_BENCH_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(Duration::from_millis(400), Duration::from_millis)
}

fn report(name: &str, bytes: Option<usize>, stats: &Stats) {
    let p50 = stats.percentile(0.50);
    let ops = if p50.is_zero() {
        0.0
    } else {
        1.0 / p50.as_secs_f64()
    };
    let throughput = bytes.map_or(String::new(), |b| {
        format!("  {:8.1} MiB/s", b as f64 / p50.as_secs_f64() / 1_048_576.0)
    });
    println!(
        "{name:<40} n={:<6} p50={:>10} p95={:>10} p99={:>10} {:>10.0} op/s{throughput}",
        stats.samples.len(),
        fmt(p50),
        fmt(stats.percentile(0.95)),
        fmt(stats.percentile(0.99)),
        ops,
    );
}

fn bench(filter: &[String], name: &str, bytes: Option<usize>, mut op: impl FnMut()) {
    if !filter.is_empty() && !filter.iter().any(|f| name.contains(f.as_str())) {
        return;
    }
    let warm = Instant::now();
    while warm.elapsed() < budget() / 4 {
        op();
    }
    let mut samples = Vec::new();
    let start = Instant::now();
    while (start.elapsed() < budget() || samples.len() < 20) && samples.len() < 200_000 {
        let t = Instant::now();
        op();
        samples.push(t.elapsed());
    }
    report(name, bytes, &Stats { samples });
}

fn block_on<F: std::future::Future>(rt: &tokio::runtime::Runtime, future: F) -> F::Output {
    rt.block_on(future)
}

/// N hilos ejecutando `iterations` flujos completos cada uno.
fn concurrent(filter: &[String], threads: usize, xml: &[u8], reply: &[u8]) {
    let name = format!("concurrent/bill_flow_50KB/{threads}");
    if !filter.is_empty() && !filter.iter().any(|f| name.contains(f.as_str())) {
        return;
    }
    let iterations = (2000 / threads).max(20);
    let start = Instant::now();
    let mut all: Vec<Duration> = Vec::with_capacity(threads * iterations);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let Ok(rt) = tokio::runtime::Builder::new_current_thread().build() else {
                        return Vec::new();
                    };
                    let transport = Canned::new(200, "text/xml", reply.to_vec());
                    let sender = BillSender::new(SoapClient::new(
                        transport,
                        "http://127.0.0.1:1/billService",
                        "20000000001MODDATOS",
                        "moddatos",
                    ))
                    .with_zip_time(fixed_time());
                    let mut local = Vec::with_capacity(iterations);
                    for _ in 0..iterations {
                        let t = Instant::now();
                        let result = rt.block_on(sender.send("20000000001-01-F001-123", xml));
                        black_box(&result);
                        local.push(t.elapsed());
                    }
                    local
                })
            })
            .collect();
        for handle in handles {
            if let Ok(samples) = handle.join() {
                all.extend(samples);
            }
        }
    });
    let wall = start.elapsed();
    let total = all.len();
    let stats = Stats { samples: all };
    println!(
        "{name:<40} n={total:<6} p50={:>10} p95={:>10} p99={:>10} {:>10.0} op/s agregadas",
        fmt(stats.percentile(0.50)),
        fmt(stats.percentile(0.95)),
        fmt(stats.percentile(0.99)),
        total as f64 / wall.as_secs_f64(),
    );
}

fn main() {
    let filter: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();
    let Ok(rt) = tokio::runtime::Builder::new_current_thread().build() else {
        eprintln!("no se pudo crear el runtime");
        return;
    };
    let time = fixed_time();
    let cdr = cdr_xml();
    let cdr_zip = cdr_zip(&cdr);
    let reply = send_bill_reply(&cdr_zip);
    println!(
        "# cpeforge-ws pipeline — CDR {} B, ZIP CDR {} B, respuesta SOAP {} B",
        cdr.len(),
        cdr_zip.len(),
        reply.len()
    );

    for (label, size) in SIZES {
        let xml = invoice(size);
        let zip = compress_at("20000000001-01-F001-123.xml", &xml, time).unwrap_or_default();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&zip);
        println!(
            "# {label}: XML {} B → ZIP {} B → Base64 {} B",
            xml.len(),
            zip.len(),
            b64.len()
        );
        bench(
            &filter,
            &format!("zip/compress/{label}"),
            Some(xml.len()),
            || {
                black_box(compress_at("20000000001-01-F001-123.xml", black_box(&xml), time).ok());
            },
        );
        bench(
            &filter,
            &format!("zip/decompress/{label}"),
            Some(xml.len()),
            || {
                black_box(decompress(black_box(&zip)).ok());
            },
        );
        bench(
            &filter,
            &format!("zip/first_xml/{label}"),
            Some(xml.len()),
            || {
                black_box(first_xml(black_box(&zip)).ok());
            },
        );
        bench(&filter, &format!("sha256/{label}"), Some(zip.len()), || {
            black_box(Sha256::digest(black_box(&zip)));
        });
        bench(
            &filter,
            &format!("base64/encode/{label}"),
            Some(zip.len()),
            || {
                black_box(base64::engine::general_purpose::STANDARD.encode(black_box(&zip)));
            },
        );
        // Solo construcción del envelope: el transporte responde 200 vacío.
        let empty = SoapClient::new(
            Canned::new(200, "text/xml", Vec::new()),
            "http://127.0.0.1:1/billService",
            "20000000001MODDATOS",
            "moddatos",
        );
        bench(
            &filter,
            &format!("soap/build_send_bill/{label}"),
            Some(zip.len()),
            || {
                black_box(block_on(&rt, empty.send_bill("20000000001-01-F001-123.zip", &zip)).ok());
            },
        );
        // Envelope + lectura de la respuesta + CDR.
        let client = SoapClient::new(
            Canned::new(200, "text/xml", reply.clone()),
            "http://127.0.0.1:1/billService",
            "20000000001MODDATOS",
            "moddatos",
        );
        bench(
            &filter,
            &format!("soap/send_bill_roundtrip/{label}"),
            Some(zip.len()),
            || {
                black_box(
                    block_on(&rt, client.send_bill("20000000001-01-F001-123.zip", &zip)).ok(),
                );
            },
        );
        let sender = BillSender::new(SoapClient::new(
            Canned::new(200, "text/xml", reply.clone()),
            "http://127.0.0.1:1/billService",
            "20000000001MODDATOS",
            "moddatos",
        ))
        .with_zip_time(time);
        bench(
            &filter,
            &format!("flow/bill_sender/{label}"),
            Some(xml.len()),
            || {
                black_box(block_on(&rt, sender.send("20000000001-01-F001-123", &xml)).ok());
            },
        );
        let gre = GreSender::new(CpeApi::new(
            Canned::new(200, "application/json", br#"{"numTicket":"T-1"}"#.to_vec()),
            Configuration::cpe("TOKEN-FICTICIO").with_host("http://127.0.0.1:1/v1"),
        ))
        .with_zip_time(time);
        bench(
            &filter,
            &format!("flow/gre_send/{label}"),
            Some(xml.len()),
            || {
                black_box(block_on(&rt, gre.send("20000000001-09-T001-1", &xml)).ok());
            },
        );
    }

    bench(&filter, "cdr/read_cdr", Some(cdr.len()), || {
        black_box(read_cdr(black_box(&cdr)).ok());
    });
    bench(&filter, "cdr/read_cdr_zip", Some(cdr.len()), || {
        black_box(read_cdr_zip(black_box(&cdr_zip)).ok());
    });
    bench(&filter, "soap/parse_reply_cdr", Some(reply.len()), || {
        let client = SoapClient::new(
            Canned::new(200, "text/xml", reply.clone()),
            "http://127.0.0.1:1/billService",
            "u",
            "p",
        );
        black_box(block_on(&rt, client.send_bill("A.zip", b"")).ok());
    });
    for (distinct, repeat) in [(10, 10), (2_000, 1), (5_000, 1), (20_000, 1)] {
        let fault = fault_reply(distinct, repeat);
        let client = SoapClient::new(
            Canned::new(500, "text/xml", fault.clone()),
            "http://127.0.0.1:1/billService",
            "u",
            "p",
        );
        bench(
            &filter,
            &format!("soap/fault_detail/{distinct}x{repeat}"),
            Some(fault.len()),
            || {
                black_box(block_on(&rt, client.send_bill("A.zip", b"")).err());
            },
        );
    }

    let request = TokenRequest::password_grant("CLIENTE", "SECRETO", "USUARIO", "CLAVE");
    let auth = Canned::new(
        200,
        "application/json",
        br#"{"access_token":"TOKEN-FICTICIO","expires_in":3600}"#.to_vec(),
    );
    let store = Arc::new(InMemoryStore::default());
    let _ = store.set(
        "CLIENTE",
        CachedToken {
            value: Secret::new("TOKEN-CACHE"),
            expires_at: FixedClock.now() + Duration::from_secs(3600),
        },
    );
    let hit = ApiFactory::new(
        auth.clone(),
        Configuration::auth().with_host("http://127.0.0.1:1/v1"),
        "http://127.0.0.1:1/v1",
        store,
        FixedClock,
    );
    bench(&filter, "oauth/cache_hit", None, || {
        black_box(block_on(&rt, hit.create(&request)).ok());
    });
    bench(&filter, "oauth/cache_miss", None, || {
        let miss = ApiFactory::new(
            auth.clone(),
            Configuration::auth().with_host("http://127.0.0.1:1/v1"),
            "http://127.0.0.1:1/v1",
            InMemoryStore::default(),
            FixedClock,
        );
        black_box(block_on(&rt, miss.create(&request)).ok());
    });

    let xml = invoice(50 * 1024);
    for threads in [1, 10, 50, 100] {
        concurrent(&filter, threads, &xml, &reply);
    }

    // Peticiones de token simultáneas sobre la misma clave y tienda vacía.
    let name = "oauth/concurrent_miss_requests/100";
    if filter.is_empty() || filter.iter().any(|f| name.contains(f.as_str())) {
        let mut auth = Canned::new(
            200,
            "application/json",
            br#"{"access_token":"TOKEN-FICTICIO","expires_in":3600}"#.to_vec(),
        );
        auth.delay = Some(Duration::from_millis(5));
        let calls = auth.calls.clone();
        let factory = Arc::new(ApiFactory::new(
            auth,
            Configuration::auth().with_host("http://127.0.0.1:1/v1"),
            "http://127.0.0.1:1/v1",
            InMemoryStore::default(),
            FixedClock,
        ));
        let Ok(multi) = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        else {
            return;
        };
        multi.block_on(async {
            let tasks: Vec<_> = (0..100)
                .map(|_| {
                    let factory = factory.clone();
                    let request = request.clone();
                    tokio::spawn(async move { factory.create(&request).await.is_ok() })
                })
                .collect();
            for task in tasks {
                black_box(task.await.ok());
            }
        });
        println!(
            "{name:<40} solicitudes de token emitidas = {}",
            calls.load(Ordering::Relaxed)
        );
    }
}
