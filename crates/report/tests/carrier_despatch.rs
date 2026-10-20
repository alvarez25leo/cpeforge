//! Guía de remisión transportista (tipo 31): datos propios en la plantilla.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "pruebas")]

use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{Despatch, Shipment, Transportist, Vehicle};
use cpeforge_report::{
    DefaultTemplateResolver, HtmlReport, ReportOptions, ReportParameters, TemplateResolver,
};

fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}

fn ruc(num: &str, name: &str) -> Option<Client> {
    Some(Client {
        tipo_doc: s("6"),
        num_doc: s(num),
        rzn_social: s(name),
        ..Client::default()
    })
}

fn render(doc: &Despatch) -> String {
    let mut report = HtmlReport::new(ReportOptions::default());
    report.set_template(DefaultTemplateResolver.template(doc).unwrap());
    let mut params = ReportParameters::default();
    // PNG 1×1: la plantilla exige logo.
    params.system.logo = Some(vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60,
        0x00, 0x02, 0x00, 0x00, 0x05, 0x00, 0x01, 0x7A, 0x5E, 0xAB, 0x3F, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]);
    report.render_html(doc, &params).unwrap()
}

fn guia(tipo: &str) -> Despatch {
    Despatch {
        tipo_doc: s(tipo),
        serie: s("V001"),
        correlativo: s("1"),
        remitente: ruc("20000000002", "EMPRESA REMITENTE"),
        destinatario: ruc("20000000003", "EMPRESA DESTINO"),
        pagador_flete: ruc("20000000004", "EMPRESA PAGADORA"),
        envio: Some(Shipment {
            transportista: Some(Transportist {
                nro_mtc: s("MTC0001"),
                ..Transportist::default()
            }),
            vehiculo: Some(Vehicle {
                placa: s("ABC123"),
                nro_circulacion: s("15M21028250E"),
                ..Vehicle::default()
            }),
            subcontratador: ruc("20000000005", "EMPRESA SUBCONTRATADA"),
            ..Shipment::default()
        }),
        ..Despatch::default()
    }
}

#[test]
fn carrier_despatch_shows_sender_mtc_payer_tuc_and_subcontractor() {
    let html = render(&guia("31"));
    for expected in [
        "GUÍA DE REMISIÓN TRANSPORTISTA",
        "REMITENTE",
        "EMPRESA REMITENTE",
        "MTC0001",
        "EMPRESA PAGADORA",
        "15M21028250E",
        "EMPRESA SUBCONTRATADA",
    ] {
        assert!(html.contains(expected), "falta {expected}");
    }
    assert!(!html.contains("Motivo Traslado"), "la 31 no lleva motivo");
}

#[test]
fn sender_despatch_ignores_carrier_fields() {
    let html = render(&guia("09"));
    assert!(html.contains("Motivo Traslado"));
    for absent in [
        "REMITENTE</td>",
        "EMPRESA REMITENTE",
        "EMPRESA PAGADORA",
        "15M21028250E",
        "EMPRESA SUBCONTRATADA",
    ] {
        assert!(!html.contains(absent), "sobra {absent}");
    }
}
