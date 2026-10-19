//! Medición del flujo completo sin red (no se ejecuta por defecto).
//!
//! ```bash
//! openssl req -x509 -newkey rsa:2048 -nodes -keyout k.pem -out c.pem -days 1 -subj /CN=20000000001
//! cat k.pem c.pem > id.pem
//! CPEFORGE_BENCH_PEM=id.pem cargo test --release -p cpeforge --test bench -- --ignored --nocapture
//! ```
//!
//! El transporte responde en memoria; se mide construir, firmar, comprimir,
//! armar el SOAP y leer la respuesta. Cada cifra es el promedio de N
//! iteraciones tras una de calentamiento.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "../../core/tests/support/hydrate.rs"]
mod hydrate;

use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cpeforge::{DocumentKind, DocumentRef, See};
use cpeforge_core::model::sale::Invoice;
use cpeforge_gre_api::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use cpeforge_ws::zip::ZipTime;
use serde_json::Value;

#[derive(Clone, Default)]
struct Ack(Arc<AtomicUsize>);
impl HttpTransport for Ack {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(request.body.len(), Ordering::Relaxed);
        Ok(HttpResponse { status: 200, reason: "OK".into(), headers: vec![], body: br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><sendSummaryResponse xmlns="http://service.sunat.gob.pe"><ticket>T-1</ticket></sendSummaryResponse></soap:Body></soap:Envelope>"#.to_vec() })
    }
}

fn invoice(lines: usize) -> Invoice {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/reference/xml/cases/invoice.json");
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut input = cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "inv21-full-default")
        .unwrap()["input"]
        .clone();
    let details = input["details"].as_array().unwrap().clone();
    input["details"] = Value::Array(details.iter().cycle().take(lines).cloned().collect());
    hydrate::deserialize(hydrate::to_serde(&input).unwrap().unwrap()).unwrap()
}

fn see(pem: &str, transport: Ack) -> See<Ack> {
    let mut see = See::new(transport);
    see.set_service("http://127.0.0.1:9/soap")
        .set_clave_sol("20000000001", "TEST", "fake")
        .set_zip_time(ZipTime::default());
    see.set_certificate(pem).unwrap();
    see
}

#[tokio::test]
#[ignore = "benchmark: ejecutar con --release --ignored y CPEFORGE_BENCH_PEM"]
async fn end_to_end() {
    let pem = std::fs::read_to_string(std::env::var("CPEFORGE_BENCH_PEM").unwrap()).unwrap();
    for lines in [1usize, 100, 1000] {
        let doc = invoice(lines);
        let iters = if lines >= 1000 { 20 } else { 200 };
        let mut facade = see(&pem, Ack::default());
        facade.send(DocumentRef::Invoice(&doc)).await.unwrap();
        let started = Instant::now();
        for _ in 0..iters {
            black_box(facade.send(DocumentRef::Invoice(&doc)).await.unwrap());
        }
        let per = started.elapsed() / iters;
        let signed = facade.factory_ref().last_xml().unwrap().to_owned();
        println!(
            "send factura {lines:>4} líneas ({:>8} B firmado) {:>8} µs/op",
            signed.len(),
            per.as_micros()
        );

        let started = Instant::now();
        for _ in 0..iters {
            black_box(facade.send_xml_file(&signed).await.unwrap());
        }
        println!(
            "send_xml_file {lines:>4} líneas                     {:>8} µs/op",
            (started.elapsed() / iters).as_micros()
        );
        let started = Instant::now();
        for _ in 0..iters {
            black_box(
                facade
                    .send_xml(
                        DocumentKind::Invoice,
                        "20000000001-01-F001-1",
                        signed.as_bytes(),
                    )
                    .await
                    .unwrap(),
            );
        }
        println!(
            "send_xml      {lines:>4} líneas                     {:>8} µs/op",
            (started.elapsed() / iters).as_micros()
        );
    }

    // Procesamiento masivo: 64 facturas de 100 líneas, una fachada por hilo
    // (la firma es CPU; cada hilo tiene su propio runtime de un hilo).
    let doc = Arc::new(invoice(100));
    let pem = Arc::new(pem);
    for threads in [1usize, 4] {
        let started = Instant::now();
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let (doc, pem) = (doc.clone(), pem.clone());
                std::thread::spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .build()
                        .unwrap();
                    runtime.block_on(async {
                        let mut facade = see(&pem, Ack::default());
                        for _ in 0..64 / threads {
                            facade.send(DocumentRef::Invoice(&doc)).await.unwrap();
                        }
                    });
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let elapsed = started.elapsed();
        println!(
            "masivo 64 facturas, {threads} hilos: {:>6} ms ({:.0} docs/s)",
            elapsed.as_millis(),
            64.0 / elapsed.as_secs_f64()
        );
    }
}
