//! Carga, concurrencia y contenido de reportes grandes.
//!
//! Complementan la paridad referencia↔Rust con documentos que el corpus no cubre:
//! miles de líneas con caracteres especiales, logos de varios MiB, el QR
//! incrustado en el HTML y plantillas propias con el dialecto admitido.
//! Ninguna prueba usa red ni ejecuta comandos.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, SaleDetail};
use cpeforge_report::qr;
use cpeforge_report::{
    HtmlReport, RenderCause, ReportError, ReportOptions, ReportParameters, TrustedHtml,
};
use jiff::Timestamp;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Texto con todo lo que el escape HTML debe tratar, acentos y emoji.
fn tricky(i: usize) -> String {
    format!("<b>\"Ítem\" {i}</b> & 'ñandú' 🧾 <script>alert({i})</script>")
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

fn invoice(lines: usize) -> Invoice {
    let details = (0..lines)
        .map(|i| {
            let i64v = i64::try_from(i).unwrap();
            SaleDetail {
                unidad: Some("NIU".into()),
                cod_producto: Some(format!("P{i:05}")),
                descripcion: Some(tricky(i)),
                cantidad: Some(Decimal::new(i64v % 97 + 1, 0)),
                mto_valor_unitario: Some(Decimal::new(i64v * 7 + 5, 3)),
                mto_valor_venta: Some(Decimal::new(i64v * 1_000 + 995, 3)),
                ..SaleDetail::default()
            }
        })
        .collect();
    Invoice {
        base: BaseSale {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("123".into()),
            fecha_emision: Some("2024-03-15T10:20:30-05:00[America/Lima]".parse().unwrap()),
            tipo_moneda: Some("PEN".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                razon_social: Some("EMISOR <S.A.C.> & 'Cía'".into()),
                ..Company::default()
            }),
            client: Some(Client {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000002".into()),
                rzn_social: Some("CLIENTE \"Ñ\" <img src=x onerror=alert(1)>".into()),
                ..Client::default()
            }),
            mto_oper_gravadas: Some(Decimal::new(100_000_000, 2)),
            mto_igv: Some(Decimal::new(18_000_000, 2)),
            mto_imp_venta: Some(Decimal::new(118_000_000, 2)),
            details: Some(details),
            legends: Some(vec![Legend {
                code: Some("1000".into()),
                value: Some("SON UN MILLÓN CIENTO OCHENTA MIL CON 00/100 SOLES".into()),
            }]),
            ..BaseSale::default()
        },
        ..Invoice::default()
    }
}

fn params() -> ReportParameters {
    let mut p = ReportParameters::default();
    p.system.logo = Some(PNG.to_vec());
    p.system.hash = Some("q1w2e3r4t5y6u7i8o9p0asdfghj=".into());
    p.user.header = Some(TrustedHtml::new("<b>Telf: 01 000-0000</b>"));
    p
}

fn report() -> HtmlReport {
    let mut options = ReportOptions::default();
    options.now = Some(Timestamp::from_second(1_710_515_430).unwrap());
    let mut report = HtmlReport::new(options);
    report.set_template("invoice.html.twig");
    report
}

/// Bytes del primer `data:image/<mime>;base64,` del HTML.
fn data_uri(html: &str, mime: &str) -> Vec<u8> {
    let prefix = format!("data:image/{mime};base64,");
    let start = html.find(&prefix).unwrap() + prefix.len();
    let end = start + html[start..].find('"').unwrap();
    STANDARD.decode(&html[start..end]).unwrap()
}

#[test]
fn thousands_of_lines_are_complete_escaped_and_exact() {
    let lines = 5_000;
    let doc = invoice(lines);
    let html = report().render_html(&doc, &params()).unwrap();

    let rows = |html: &str| html.matches("<tr class=\"border_top\">").count();
    let empty = report().render_html(&invoice(0), &params()).unwrap();
    assert_eq!(rows(&html) - rows(&empty), lines);
    for i in [0, 1, 999, 2_500, lines - 1] {
        assert!(html.contains(&escaped(&tricky(i))), "línea {i}");
        assert!(html.contains(&format!("P{i:05}")), "código {i}");
        let i64v = i64::try_from(i).unwrap();
        // `n_format` (2 decimales, half-up) del importe exacto de la línea.
        let venta = Decimal::new(i64v * 1_000 + 995, 3)
            .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        assert!(html.contains(&format!("{venta:.2}")), "importe {i}");
    }
    // Ningún dato del documento sale sin escapar.
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<img src=x"));
    assert!(html.contains("EMISOR &lt;S.A.C.&gt; &amp; &#039;Cía&#039;"));
    // La cabecera confiable sí se inserta tal cual.
    assert!(html.contains("<b>Telf: 01 000-0000</b>"));
    // Totales y moneda del documento, sin recalcular.
    assert!(html.contains("1180000.00"));
    assert!(html.contains("180000.00"));
    // El render es determinista.
    assert_eq!(html, report().render_html(&doc, &params()).unwrap());
}

#[test]
fn embedded_qr_encodes_the_sale_payload() {
    let doc = invoice(3);
    let html = report().render_html(&doc, &params()).unwrap();
    // Contenido calculado aquí a partir del documento, sin pasar por la plantilla.
    let payload = "20000000001|01|F001|123|180000.00|1180000.00|2024-03-15|6|20000000002|";
    let svg = String::from_utf8(data_uri(&html, "svg+xml")).unwrap();
    assert_eq!(svg, qr::svg(payload.as_bytes()).unwrap());
    let symbol = qr::encode(payload.as_bytes()).unwrap();
    assert_eq!(symbol.mode, qr::QrMode::Byte);
    assert_eq!(symbol.size(), usize::from(symbol.version) * 4 + 17);
    assert!(symbol.modules.iter().all(|row| row.len() == symbol.size()));
}

#[test]
fn large_logos_round_trip_and_limit_is_exact() {
    let doc = invoice(1);
    for (signature, mime) in [
        (&b"\x89PNG\r\n\x1a\n"[..], "png"),
        (&b"GIF89a"[..], "gif"),
        (&b"\xff\xd8\xff\xe0"[..], "jpeg"),
    ] {
        let mut logo = signature.to_vec();
        logo.extend((0..3 * 1024 * 1024).map(|i: u32| i.to_le_bytes()[0] ^ 0x5A));
        let mut p = params();
        p.system.logo = Some(logo.clone());
        let html = report().render_html(&doc, &p).unwrap();
        assert_eq!(data_uri(&html, mime), logo, "{mime}");
    }

    let mut options = ReportOptions::default();
    options.max_logo_bytes = Some(1024);
    let limited = HtmlReport::new(options);
    let mut p = params();
    p.system.logo = Some(vec![0xff; 1024]);
    assert!(limited.render_html(&doc, &p).is_ok());
    p.system.logo = Some(vec![0xff; 1025]);
    assert!(matches!(
        limited.render_html(&doc, &p),
        Err(ReportError::Render {
            cause: RenderCause::Limit(_),
            ..
        })
    ));
}

#[test]
fn one_engine_is_shared_safely_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<HtmlReport>();

    let report = Arc::new(report());
    let docs: Vec<Arc<Invoice>> = (1..=8).map(|n| Arc::new(invoice(n * 25))).collect();
    let params = Arc::new(params());
    let expected: Vec<String> = docs
        .iter()
        .map(|d| report.render_html(d.as_ref(), &params).unwrap())
        .collect();
    let expected = Arc::new(expected);
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let (report, docs, params, expected) = (
                Arc::clone(&report),
                docs.clone(),
                Arc::clone(&params),
                Arc::clone(&expected),
            );
            std::thread::spawn(move || {
                for round in 0..20 {
                    let index = (t + round) % docs.len();
                    let html = report.render_html(docs[index].as_ref(), &params).unwrap();
                    assert_eq!(html, expected[index]);
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn custom_templates_use_the_minijinja_dialect_with_builtin_helpers() {
    let dir = tempfile::tempdir().unwrap();
    // Equivalentes admitidos de `?:` y `??` de Twig; `|format` existe en
    // MiniJinja (formato `%` de Python, próximo a `sprintf`).
    fs::write(
        dir.path().join("propia.html.twig"),
        "<style>{% include 'assets/style.css' %}</style>\
         {{ 'S' if doc.serie is twig_true else 'N' }}|\
         {{ doc.nada|default('defecto') }}|\
         {{ '%s/%s'|format(doc.serie, doc.correlativo) }}|\
         {{ doc.serie|twig_string ~ '-' ~ doc.correlativo }}|\
         {{ doc.tipoDoc|catalog('01') }}|{{ doc.mtoImpVenta|n_format }}|\
         {{ doc.client.rznSocial }}|{{ params.user.header|raw }}|\
         {{ legend(doc.legends, '1000') }}|{{ doc.fechaEmision|date('d/m/Y H:i') }}|\
         {% for det in doc.details %}{{ loop.index }}:{{ det.cantidad|n_format(0) }};{% endfor %}",
    )
    .unwrap();
    let mut options = ReportOptions::with_template_dir(dir.path());
    options.strict_variables = false;
    let mut report = HtmlReport::new(options);
    report.set_template("propia.html.twig");
    let html = report.render_html(&invoice(2), &params()).unwrap();
    assert!(html.starts_with("<style>"));
    let body = &html[html.find("</style>").unwrap() + "</style>".len()..];
    assert_eq!(
        body,
        "S|defecto|F001/123|F001-123|FACTURA|1180000.00|\
         CLIENTE &quot;Ñ&quot; &lt;img src=x onerror=alert(1)&gt;|\
         <b>Telf: 01 000-0000</b>|SON UN MILLÓN CIENTO OCHENTA MIL CON 00/100 SOLES|\
         15/03/2024 10:20|1:1;2:2;"
    );

    // Operadores y etiquetas exclusivos de Twig: error tipado, nunca un
    // HTML parcial.
    for (name, body) in [
        ("ternario.html.twig", "{{ doc.serie ? 'S' : 'N' }}"),
        ("elvis.html.twig", "{{ doc.serie ?: 'N' }}"),
        ("nulo.html.twig", "{{ doc.nada ?? 'x' }}"),
        ("apply.html.twig", "{% apply upper %}x{% endapply %}"),
        ("with.html.twig", "{% with {a: 1} %}{{ a }}{% endwith %}"),
        ("starts.html.twig", "{{ 'a' starts with 'a' }}"),
    ] {
        fs::write(dir.path().join(name), body).unwrap();
        report.set_template(name);
        let err = report.render_html(&invoice(1), &params()).unwrap_err();
        assert!(matches!(err, ReportError::Syntax { .. }), "{name}: {err:?}");
    }
}

#[test]
fn twig_range_operator_is_silently_undefined() {
    // Diferencia de dialecto que no produce error: `1..3` no es un rango en
    // MiniJinja y se imprime vacío, con o sin `strict_variables`; en las
    // plantillas propias hay que usar `range()`.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("rango.html.twig"),
        "[{{ (1..3)|join(',') }}][{{ range(1, 4)|join(',') }}]",
    )
    .unwrap();
    let mut report = HtmlReport::new(ReportOptions::with_template_dir(dir.path()));
    report.set_template("rango.html.twig");
    assert_eq!(
        report.render_html(&invoice(1), &params()).unwrap(),
        "[][1,2,3]"
    );
    let mut options = ReportOptions::with_template_dir(dir.path());
    options.strict_variables = true;
    let mut strict = HtmlReport::new(options);
    strict.set_template("rango.html.twig");
    // Tampoco falla con `strict_variables`.
    assert_eq!(
        strict.render_html(&invoice(1), &params()).unwrap(),
        "[][1,2,3]"
    );
}

#[test]
fn custom_templates_cannot_reach_files_environment_or_unregistered_filters() {
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in [
        ("tojson.html.twig", "{{ doc|tojson }}"),
        ("loader.html.twig", "{% include '/etc/hosts' %}"),
        ("drive.html.twig", "{% include 'C:/Windows/win.ini' %}"),
        ("parent.html.twig", "{% include '../outside.html.twig' %}"),
    ] {
        fs::write(dir.path().join(name), body).unwrap();
        let mut report = HtmlReport::new(ReportOptions::with_template_dir(dir.path()));
        report.set_template(name);
        assert!(
            report.render_html(&invoice(1), &params()).is_err(),
            "{name}"
        );
    }
    // `debug()` de MiniJinja solo muestra el contexto del render y los
    // nombres registrados en el motor: ni rutas ni variables de entorno.
    fs::write(dir.path().join("debug.html.twig"), "{{ debug() }}").unwrap();
    let mut report = HtmlReport::new(ReportOptions::with_template_dir(dir.path()));
    report.set_template("debug.html.twig");
    let dump = report.render_html(&invoice(1), &params()).unwrap();
    let dir_name = dir.path().to_string_lossy();
    assert!(!dump.contains(dir_name.as_ref()));
    for (key, value) in std::env::vars() {
        if value.len() >= 8 && key != "PWD" && key != "OLDPWD" {
            assert!(!dump.contains(&value), "variable {key}");
        }
    }
}
