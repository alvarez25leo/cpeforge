//! Benchmark local de `cpeforge-report`, sin red ni dependencias extra.
//!
//! Mide el render HTML de las siete plantillas con los documentos del corpus
//! de paridad (`tests/reference/report/cases/html.json`), facturas de 10 a
//! 10 000 líneas, logos grandes, el QR, `XmlUtils` y el uso concurrente de un
//! mismo `HtmlReport`.
//!
//! ```text
//! cargo bench -p cpeforge-report --bench render               # todo
//! cargo bench -p cpeforge-report --bench render -- lines      # filtro
//! cargo bench -p cpeforge-report --bench render -- --dump DIR # salidas
//! ```
//!
//! `--dump DIR` no mide: escribe en `DIR` el HTML o SVG de cada escenario
//! para comparar byte a byte dos versiones del crate (`diff -r`).
//! `REPORT_BENCH_MS` fija el tiempo de muestreo por caso (400 ms).
//!
//! La memoria máxima se mide por fuera del proceso, por ejemplo con
//! `/usr/bin/time -l` (macOS) o `/usr/bin/time -v` (Linux) sobre un filtro.

// Las estadísticas de tiempo usan `f64`; no hay importes en este binario.
#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    reason = "binario de medición con datos sintéticos propios"
)]

#[path = "../../core/tests/support/hydrate.rs"]
#[allow(
    dead_code,
    unreachable_pub,
    reason = "módulo compartido con las pruebas"
)]
mod hydrate;

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::Invoice;
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::Voided;
use cpeforge_report::{
    HtmlReport, ReportDocument, ReportOptions, ReportParameters, TrustedHtml, XmlUtils, qr,
};
use jiff::Timestamp;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const NOW: i64 = 1_710_515_430;

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
    std::env::var("REPORT_BENCH_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(Duration::from_millis(400), Duration::from_millis)
}

/// Contexto de ejecución: filtro por nombre y modo `--dump`.
struct Run {
    filter: Vec<String>,
    dump: Option<PathBuf>,
}

impl Run {
    fn selected(&self, name: &str) -> bool {
        self.filter.is_empty() || self.filter.iter().any(|f| name.contains(f.as_str()))
    }

    /// Mide `op` o, con `--dump`, escribe su salida.
    fn bench(&self, name: &str, bytes: Option<usize>, mut op: impl FnMut() -> String) {
        if !self.selected(name) {
            return;
        }
        if let Some(dir) = &self.dump {
            let path = dir.join(name.replace('/', "__"));
            std::fs::write(&path, op()).unwrap();
            return;
        }
        let warm = Instant::now();
        while warm.elapsed() < budget() / 4 {
            black_box(op());
        }
        let mut samples = Vec::new();
        let mut out_len = 0;
        let start = Instant::now();
        while (start.elapsed() < budget() || samples.len() < 10) && samples.len() < 200_000 {
            let t = Instant::now();
            let out = black_box(op());
            samples.push(t.elapsed());
            out_len = out.len();
        }
        report(name, bytes, out_len, &Stats { samples });
    }
}

fn report(name: &str, bytes: Option<usize>, out_len: usize, stats: &Stats) {
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
        "{name:<34} n={:<6} p50={:>10} p95={:>10} p99={:>10} {:>9.0} op/s  out={:>9} B{throughput}",
        stats.samples.len(),
        fmt(p50),
        fmt(stats.percentile(0.95)),
        fmt(stats.percentile(0.99)),
        ops,
        out_len,
    );
}

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/report")
}

fn cases() -> Vec<Value> {
    let path = reference_dir().join("cases/html.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let root: Value = serde_json::from_str(&text).unwrap();
    root["cases"].as_array().unwrap().clone()
}

fn case<'a>(cases: &'a [Value], id: &str) -> &'a Value {
    cases.iter().find(|c| c["id"] == id).unwrap()
}

fn hydrate<T: DeserializeOwned>(node: &Value) -> T {
    let value = hydrate::to_serde(node).unwrap().unwrap();
    hydrate::deserialize(value).unwrap()
}

fn b64(node: &Value) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(node["$bytes"].as_str().unwrap())
        .unwrap()
}

/// Parámetros del caso: logo, hash, URL del QR, cabecera, pie y extras.
fn params_of(case: &Value) -> ReportParameters {
    let node = &case["params"];
    let mut out = ReportParameters::default();
    if let Some(logo) = node["system"]
        .get("logo")
        .filter(|v| v.get("$bytes").is_some())
    {
        out.system.logo = Some(b64(logo));
    }
    if let Some(hash) = node["system"]["hash"].as_str() {
        out.system.hash = Some(hash.to_owned());
    }
    if let Some(url) = node["system"]["qr"].as_str() {
        out.system.qr = Some(url.to_owned());
    }
    if let Some(header) = node["user"]["header"].as_str() {
        out.user.header = Some(TrustedHtml::new(header));
    }
    if let Some(footer) = node["user"]["footer"].as_str() {
        out.user.footer = Some(TrustedHtml::new(footer));
    }
    out
}

fn report_for(template: &str) -> HtmlReport {
    let mut options = ReportOptions::default();
    options.now = Some(Timestamp::from_second(NOW).unwrap());
    let mut report = HtmlReport::new(options);
    report.set_template(template);
    report
}

/// `invoice-full` con `lines` líneas: textos con caracteres especiales,
/// acentos y emoji, e importes distintos en cada línea.
fn invoice_with_lines(cases: &[Value], lines: usize) -> Invoice {
    let mut node = case(cases, "invoice-full")["document"].clone();
    let template = node["details"][0].clone();
    let details: Vec<Value> = (0..lines)
        .map(|i| {
            let mut d = template.clone();
            d["codProducto"] = json!(format!("P{i:06}"));
            d["descripcion"] = json!(format!(
                "Producto <{i}> & \"comillas\" 'simples' ÁÉÍÓÚ ñ — línea {i} 🧾"
            ));
            d["cantidad"] = json!({"$decimal": format!("{}.{:03}", i % 97 + 1, i % 1000)});
            d["mtoValorUnitario"] =
                json!({"$decimal": format!("{}.{:02}", i * 7 % 10_000, i % 100)});
            d["mtoValorVenta"] =
                json!({"$decimal": format!("{}.{:02}", i * 13 % 100_000, (i * 3) % 100)});
            d
        })
        .collect();
    node["details"] = Value::Array(details);
    hydrate(&node)
}

/// Logo PNG sintético de `size` bytes.
fn logo(size: usize) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    while out.len() < size {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push(state.to_le_bytes()[0]);
    }
    out
}

fn signed_xml(target: usize) -> Vec<u8> {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><ds:Signature><ds:SignedInfo><ds:Reference URI=""><ds:DigestValue>q1w2e3r4t5y6u7i8o9p0asdfghj=</ds:DigestValue></ds:Reference></ds:SignedInfo></ds:Signature></ext:ExtensionContent></ext:UBLExtension></ext:UBLExtensions>"#,
    );
    let mut i = 0;
    while xml.len() < target {
        xml.push_str(&format!(
            "<cbc:Note languageLocaleID=\"{i}\">Línea {i} &amp; texto &lt;escapado&gt;</cbc:Note>"
        ));
        i += 1;
    }
    xml.push_str("</Invoice>");
    xml.into_bytes()
}

fn render_doc(report: &HtmlReport, doc: &dyn ReportDocument, params: &ReportParameters) -> String {
    report.render_html(doc, params).unwrap()
}

fn concurrent(run: &Run, threads: usize, doc: &Arc<Invoice>, params: &Arc<ReportParameters>) {
    let name = format!("concurrent/invoice-100-lines/{threads}");
    if !run.selected(&name) || run.dump.is_some() {
        return;
    }
    let report = Arc::new(report_for("invoice.html.twig"));
    let per_thread = 200;
    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (report, doc, params) = (Arc::clone(&report), Arc::clone(doc), Arc::clone(params));
            std::thread::spawn(move || {
                for _ in 0..per_thread {
                    black_box(report.render_html(doc.as_ref(), &params).unwrap());
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let elapsed = start.elapsed();
    let total = threads * per_thread;
    println!(
        "{name:<34} renders={total:<6} total={:>10} {:>9.0} op/s agregadas",
        fmt(elapsed),
        total as f64 / elapsed.as_secs_f64()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut filter = Vec::new();
    let mut dump = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--dump" {
            dump = iter.next().map(PathBuf::from);
        } else if !arg.starts_with("--") {
            filter.push(arg.clone());
        }
    }
    if let Some(dir) = &dump {
        std::fs::create_dir_all(dir).unwrap();
    }
    let run = Run { filter, dump };
    let cases = cases();

    // Siete plantillas con los documentos completos del corpus.
    macro_rules! template_case {
        ($id:literal, $ty:ty) => {{
            let c = case(&cases, $id);
            let doc: $ty = hydrate(&c["document"]);
            let params = params_of(c);
            let report = report_for(c["template"].as_str().unwrap());
            run.bench(concat!("html/", $id), None, || {
                render_doc(&report, &doc, &params)
            });
        }};
    }
    template_case!("invoice-full", Invoice);
    template_case!("invoice2-full", Invoice);
    template_case!("despatch-full", Despatch);
    template_case!("despatch-qr-url", Despatch);
    template_case!("perception-full", Perception);
    template_case!("retention-full", Retention);
    template_case!("summary-full", Summary);
    template_case!("voided-full", Voided);

    // Escalado por número de líneas.
    let base_params = params_of(case(&cases, "invoice-full"));
    let report = report_for("invoice.html.twig");
    for lines in [10, 100, 1_000, 10_000] {
        let doc = invoice_with_lines(&cases, lines);
        run.bench(&format!("html/invoice-lines/{lines}"), None, || {
            render_doc(&report, &doc, &base_params)
        });
    }

    // Logo grande (incrustado en Base64 en el HTML).
    let small = invoice_with_lines(&cases, 10);
    for (label, size) in [
        ("100KB", 100 * 1024),
        ("1MB", 1024 * 1024),
        ("8MB", 8 * 1024 * 1024),
    ] {
        let mut params = base_params.clone();
        params.system.logo = Some(logo(size));
        run.bench(&format!("html/invoice-logo/{label}"), Some(size), || {
            render_doc(&report, &small, &params)
        });
    }

    // Motor nuevo por render (sin reutilizar plantillas compiladas).
    let doc = invoice_with_lines(&cases, 10);
    run.bench("html/cold-new-engine", None, || {
        render_doc(&report_for("invoice.html.twig"), &doc, &base_params)
    });

    // QR.
    let sale = b"20000000001|01|F001|123|18.00|118.00|2024-03-15|6|20000000002|";
    run.bench("qr/svg-sale", None, || qr::svg(sale).unwrap());
    run.bench("qr/encode-sale", None, || {
        format!("{}", qr::encode(sale).unwrap().mask)
    });
    let long = vec![b'x'; 1661];
    run.bench("qr/svg-v40-1661B", None, || qr::svg(&long).unwrap());

    // XmlUtils.
    let xml = signed_xml(1024 * 1024);
    run.bench("xml/hash-sign-1MB", Some(xml.len()), || {
        XmlUtils::default().hash_sign(&xml).unwrap()
    });

    // Concurrencia: un solo `HtmlReport` compartido.
    let doc = Arc::new(invoice_with_lines(&cases, 100));
    let params = Arc::new(base_params);
    for threads in [1, 2, 4, 8] {
        concurrent(&run, threads, &doc, &params);
    }
}
