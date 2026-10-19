//! Benchmark local de `cpeforge-xml-parser`, sin dependencias extra.
//!
//! Usa los XML sintéticos de `tests/reference/xml-parser/cases.json` (un caso
//! `*-full` por parser) y una factura escalada a 50 KB, 500 KB y 5 MB
//! replicando sus líneas `cac:InvoiceLine`.
//!
//! ```text
//! cargo bench -p cpeforge-xml-parser --bench parse              # todo
//! cargo bench -p cpeforge-xml-parser --bench parse -- invoice   # filtro
//! ```

// Las estadísticas de tiempo usan `f64`; no hay importes en este binario.
#![allow(clippy::float_arithmetic)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use cpeforge_core::ports::DocumentParser;
use cpeforge_xml_parser::{
    DespatchParser, InvoiceParser, Limits, NoteParser, ParseOptions, PerceptionParser,
    ReceiptParser, RetentionParser, SummaryParser, XmlDocument, XmlReader,
};

fn case(cases: &serde_json::Value, id: &str) -> Vec<u8> {
    cases["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["id"] == id)
        .and_then(|c| c["input"]["xml"].as_str())
        .unwrap_or_default()
        .as_bytes()
        .to_vec()
}

/// `invoice-full` con sus líneas repetidas hasta al menos `target` bytes.
fn scaled_invoice(base: &[u8], target: usize) -> Vec<u8> {
    let text = String::from_utf8_lossy(base);
    let (Some(start), Some(end)) = (
        text.find("<cac:InvoiceLine>"),
        text.rfind("</cac:InvoiceLine>"),
    ) else {
        return base.to_vec();
    };
    let end = end + "</cac:InvoiceLine>".len();
    let lines = &text[start..end];
    let mut out = String::with_capacity(target + text.len());
    out.push_str(&text[..end]);
    while out.len() + text.len() - end < target {
        out.push_str(lines);
    }
    out.push_str(&text[end..]);
    out.into_bytes()
}

fn budget() -> Duration {
    std::env::var("XML_PARSER_BENCH_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(Duration::from_millis(400), Duration::from_millis)
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

fn bench(filter: &[String], name: &str, bytes: usize, mut op: impl FnMut()) {
    if !filter.is_empty() && !filter.iter().any(|f| name.contains(f.as_str())) {
        return;
    }
    let warm = Instant::now();
    while warm.elapsed() < budget() / 4 {
        op();
    }
    let mut samples = Vec::new();
    let start = Instant::now();
    while (start.elapsed() < budget() || samples.len() < 10) && samples.len() < 200_000 {
        let t = Instant::now();
        op();
        samples.push(t.elapsed());
    }
    samples.sort_unstable();
    let at = |p: f64| {
        let rank = ((samples.len() as f64 - 1.0) * p).round() as usize;
        samples.get(rank).copied().unwrap_or_default()
    };
    let p50 = at(0.5);
    println!(
        "{name:<34} n={:<6} p50={:>10} p95={:>10} p99={:>10} {:>9.1} MiB/s",
        samples.len(),
        fmt(p50),
        fmt(at(0.95)),
        fmt(at(0.99)),
        bytes as f64 / p50.as_secs_f64().max(1e-12) / 1_048_576.0,
    );
}

fn main() {
    let filter: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/reference/xml-parser/cases.json"
    );
    let Some(cases) = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        eprintln!("no se pudo leer {path}");
        return;
    };
    // Reloj y zona fijos: la fecha actual no interviene en la medición.
    let options = ParseOptions::default();

    macro_rules! parser_case {
        ($name:literal, $id:literal, $parser:ty) => {{
            let xml = case(&cases, $id);
            let parser = <$parser>::with_options(options.clone());
            bench(&filter, $name, xml.len(), || {
                black_box(parser.parse(black_box(&xml)).ok());
            });
        }};
    }
    parser_case!("parse/invoice-full", "invoice-full", InvoiceParser);
    parser_case!("parse/note-credit-full", "note-credit-full", NoteParser);
    parser_case!("parse/despatch-full", "despatch-full", DespatchParser);
    parser_case!("parse/receipt-full", "receipt-full", ReceiptParser);
    parser_case!("parse/summary-full", "summary-full", SummaryParser);
    parser_case!("parse/retention-full", "retention-full", RetentionParser);
    parser_case!("parse/perception-full", "perception-full", PerceptionParser);

    let base = case(&cases, "invoice-full");
    let big = Limits {
        max_bytes: 64 * 1024 * 1024,
        max_nodes: 10_000_000,
        ..Limits::default()
    };
    let invoices = InvoiceParser::with_options(options.clone().with_limits(big));
    for (label, target) in [
        ("10KB", 0),
        ("50KB", 50 * 1024),
        ("500KB", 500 * 1024),
        ("5MB", 5 * 1024 * 1024),
    ] {
        let xml = scaled_invoice(&base, target);
        bench(&filter, &format!("dom/parse/{label}"), xml.len(), || {
            black_box(XmlDocument::parse_with_limits(black_box(&xml), &big).ok());
        });
        bench(
            &filter,
            &format!("invoice/parse/{label}"),
            xml.len(),
            || {
                black_box(invoices.parse(black_box(&xml)).ok());
            },
        );
        let Ok(reader) = XmlReader::load_xml_with_limits(&xml, &big) else {
            continue;
        };
        bench(
            &filter,
            &format!("reader/value_abs/{label}"),
            xml.len(),
            || {
                black_box(reader.value("/*/cbc:ID", None).ok());
            },
        );
        bench(
            &filter,
            &format!("reader/descendant/{label}"),
            xml.len(),
            || {
                black_box(reader.nodes("//cbc:LineExtensionAmount", None).ok());
            },
        );
        bench(&filter, &format!("reader/lines/{label}"), xml.len(), || {
            let lines = reader.nodes("cac:InvoiceLine", None).ok().flatten();
            for line in lines.into_iter().flatten() {
                black_box(reader.value("cac:Item/cbc:Description", Some(line)).ok());
            }
        });
    }
}
