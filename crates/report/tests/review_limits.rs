//! Límites de la revisión de seguridad: decimales de `n_format`, tamaño del
//! HTML, contenido del QR, MIME de `image_data_uri` y atributos y espacios de
//! nombres de `XmlUtils`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;
use std::time::{Duration, Instant};

use cpeforge_core::model::company::Company;
use cpeforge_core::model::sale::{BaseSale, Invoice};
use cpeforge_report::qr::{self, QrError};
use cpeforge_report::{
    HtmlReport, RenderCause, ReportError, ReportOptions, ReportParameters, XmlUtils, XmlUtilsError,
    image_data_uri,
};
use std::fmt::Write as _;

fn invoice() -> Invoice {
    Invoice {
        base: BaseSale {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("1".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                ..Company::default()
            }),
            ..BaseSale::default()
        },
        ..Invoice::default()
    }
}

fn render(template: &str, options: impl FnOnce(&mut ReportOptions)) -> Result<String, ReportError> {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("t.html.twig"), template).unwrap();
    let mut opts = ReportOptions::with_template_dir(dir.path());
    options(&mut opts);
    let mut report = HtmlReport::new(opts);
    report.set_template("t.html.twig");
    report.render_html(&invoice(), &ReportParameters::default())
}

fn is_limit(result: &Result<String, ReportError>) -> bool {
    matches!(
        result,
        Err(ReportError::Render {
            cause: RenderCause::Limit(_),
            ..
        })
    )
}

#[test]
fn n_format_rejects_absurd_decimals_without_panicking() {
    for decimals in ["70000", "9223372036854775807", "200000000"] {
        let result = render(&format!("{{{{ 1.5|n_format({decimals}) }}}}"), |_| {});
        assert!(is_limit(&result), "{decimals}: {result:?}");
    }
    assert_eq!(render("{{ 1.5|n_format(3) }}", |_| {}).unwrap(), "1.500");
    assert_eq!(
        cpeforge_report::format::number_format(cpeforge_report::format::Number::Int(1), i64::MAX)
            .len(),
        2 + 100
    );
}

#[test]
fn html_output_is_bounded() {
    let template = "{% for i in range(100000) %}0123456789{% endfor %}";
    let result = render(template, |o| o.max_output_bytes = Some(64 * 1024));
    assert!(is_limit(&result), "{result:?}");
    let unbounded = render(template, |o| o.max_output_bytes = None).unwrap();
    assert_eq!(unbounded.len(), 1_000_000);
}

#[test]
fn oversized_qr_content_is_rejected_before_encoding() {
    let huge = "é".repeat(20 * 1024 * 1024);
    let started = Instant::now();
    assert_eq!(
        qr::encode(huge.as_bytes()).unwrap_err(),
        QrError::DataTooBig
    );
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(qr::encode("1".repeat(3000).as_bytes()).is_ok());
}

#[test]
fn image_data_uri_only_takes_simple_mime_subtypes() {
    let png = b"\x89PNG\r\n\x1a\n";
    assert!(image_data_uri(png, Some("svg+xml")).starts_with("data:image/svg+xml;base64,"));
    for hostile in ["png\" onerror=\"x", "png;base64,PHN2Zz4=#", "x/y", "html "] {
        assert!(
            image_data_uri(png, Some(hostile)).starts_with("data:image/png;base64,"),
            "{hostile}"
        );
    }
}

#[test]
fn xml_utils_bounds_attributes_and_namespace_scopes() {
    let attributes: String = (0..40_000).fold(String::new(), |mut out, i| {
        let _ = write!(out, " a{i}=''");
        out
    });
    let started = Instant::now();
    assert!(matches!(
        XmlUtils::default().hash_sign(format!("<Invoice{attributes}/>").as_bytes()),
        Err(XmlUtilsError::Limit(_))
    ));
    assert!(started.elapsed() < Duration::from_millis(500));

    let declarations: String = (0..250).fold(String::new(), |mut out, i| {
        let _ = write!(out, " xmlns:p{i}='urn:{i}'");
        out
    });
    let children = "<x xmlns:z='u'/>".repeat(20_000);
    assert!(matches!(
        XmlUtils::default()
            .hash_sign(format!("<Invoice{declarations}>{children}</Invoice>").as_bytes()),
        Err(XmlUtilsError::Limit(_))
    ));
    assert_eq!(
        XmlUtils::default().hash_sign(b"<Invoice xmlns:p='urn:p' p:a='1'/>"),
        Ok(String::new())
    );
}

#[test]
fn xml_utils_bounds_nodes_and_default_depth() {
    let utils = XmlUtils::default();
    let nested = |depth: usize| format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
    assert!(utils.hash_sign(nested(64).as_bytes()).is_ok());
    assert!(matches!(
        utils.hash_sign(nested(65).as_bytes()),
        Err(XmlUtilsError::Limit(_))
    ));
    let max = usize::try_from(cpeforge_report::xml_utils::MAX_NODES).unwrap();
    let flat = format!("<r>{}</r>", "<a/>".repeat(max));
    assert!(matches!(
        utils.hash_sign(flat.as_bytes()),
        Err(XmlUtilsError::Limit(_))
    ));
}
