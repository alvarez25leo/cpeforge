//! API pública: plantilla inicial, resolver, contrato `Report` y plantillas integradas.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas"
)]

use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::sale::{BaseSale, Invoice, Receipt};
use cpeforge_core::ports::Report;
use cpeforge_report::{
    BUILTIN_TEMPLATES, DEFAULT_TEMPLATE, DefaultTemplateResolver, HtmlReport, ReportError,
    ReportParameters, TemplateResolver, catalog_value, image_data_uri, mime_type,
};

#[test]
fn default_template_is_always_invoice() {
    let report = HtmlReport::default();
    assert_eq!(report.template(), DEFAULT_TEMPLATE);
    assert_eq!(DEFAULT_TEMPLATE, "invoice.html.twig");
    // render no consulta el resolver: una guía con la plantilla inicial falla
    // en la plantilla de factura, como en la referencia.
    let err = report
        .render_html(&Despatch::default(), &ReportParameters::default())
        .unwrap_err();
    assert!(matches!(err, ReportError::Render { .. }), "{err:?}");
}

#[test]
fn resolver_maps_classes() {
    let r = DefaultTemplateResolver;
    assert_eq!(
        r.template(&Invoice::default()).unwrap(),
        "invoice.html.twig"
    );
    assert_eq!(
        r.template(&Despatch::default()).unwrap(),
        "despatch.html.twig"
    );
    let err = r.template(&Receipt::default()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Not found template for CPEForge\\Model\\Sale\\Receipt"
    );
    assert!(r.template(&BaseSale::default()).is_err());
}

#[test]
fn report_port_is_implemented() {
    fn takes_port<R: Report<Invoice, Parameters = ReportParameters, Error = ReportError>>(_: &R) {}
    takes_port(&HtmlReport::default());
}

#[test]
fn builtin_templates_and_helpers() {
    let names: Vec<_> = BUILTIN_TEMPLATES.iter().map(|(n, _)| *n).collect();
    assert_eq!(names.len(), 8);
    assert!(names.contains(&"assets/style.css"));
    assert_eq!(catalog_value("07", "01"), "NOTA DE CRÉDITO");
    assert_eq!(catalog_value("99", "01"), "");
    assert_eq!(mime_type(b"GIF89a"), "gif");
    assert_eq!(
        image_data_uri(b"\xff\xd8\xff", None),
        "data:image/jpeg;base64,/9j/"
    );
}
