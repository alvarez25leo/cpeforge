//! Factura con detracción por servicio de transporte de carga (operación
//! `1004`): la representación impresa muestra los datos de la línea debajo de
//! la cuenta de detracciones, y una factura sin ellos no cambia.

#![allow(clippy::panic, reason = "pruebas: un fallo debe abortar la prueba")]

use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::Direction;
use cpeforge_core::model::sale::{
    BaseSale, CargoTransport, Detraction, Invoice, Legend, SaleDetail,
};
use cpeforge_report::{
    DefaultTemplateResolver, HtmlReport, ReportOptions, ReportParameters, TemplateResolver,
};

fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}

fn d(value: i64, scale: u32) -> Option<Decimal> {
    Some(Decimal::new(value, scale))
}

fn point(ubigeo: &str, direccion: &str) -> Option<Direction> {
    Some(Direction {
        ubigueo: s(ubigeo),
        direccion: s(direccion),
        ..Direction::default()
    })
}

fn invoice(transporte_carga: Option<CargoTransport>) -> Invoice {
    Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("123"),
            fecha_emision: "2026-10-31T10:00:00-05:00[America/Lima]".parse().ok(),
            tipo_moneda: s("PEN"),
            company: Some(Company {
                ruc: s("20000000001"),
                razon_social: s("TRANSPORTES DEMO S.A.C."),
                address: Some(Address {
                    direccion: s("AV. DEMO 123"),
                    ..Address::default()
                }),
                ..Company::default()
            }),
            client: Some(Client {
                tipo_doc: s("6"),
                num_doc: s("20000000002"),
                rzn_social: s("CLIENTE DEMO S.A."),
                ..Client::default()
            }),
            mto_oper_gravadas: d(100_000, 2),
            mto_igv: d(18_000, 2),
            total_impuestos: d(18_000, 2),
            mto_imp_venta: d(118_000, 2),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON MIL CIENTO OCHENTA CON 00/100 SOLES"),
            }]),
            details: Some(vec![SaleDetail {
                unidad: s("ZZ"),
                cantidad: d(1, 0),
                cod_producto: s("TRC01"),
                descripcion: s("Servicio de transporte de carga"),
                mto_valor_unitario: d(100_000, 2),
                mto_valor_venta: d(100_000, 2),
                tip_afe_igv: s("10"),
                igv: d(18_000, 2),
                mto_precio_unitario: d(118_000, 2),
                transporte_carga,
                ..SaleDetail::default()
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("1004"),
        detraccion: Some(Detraction {
            percent: d(4, 0),
            mount: d(4_720, 2),
            cta_banco: s("00-000-000000"),
            cod_medio_pago: s("001"),
            cod_bien_detraccion: s("027"),
            value_ref: None,
        }),
        valor_venta: d(100_000, 2),
        sub_total: d(118_000, 2),
        ..Invoice::default()
    }
}

fn render(doc: &Invoice) -> String {
    let mut report = HtmlReport::new(ReportOptions::default());
    report.set_template(DefaultTemplateResolver.template(doc).unwrap_or_default());
    let mut params = ReportParameters::default();
    // PNG 1×1: la plantilla exige logo.
    params.system.logo = Some(vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60,
        0x00, 0x02, 0x00, 0x00, 0x05, 0x00, 0x01, 0x7A, 0x5E, 0xAB, 0x3F, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]);
    report
        .render_html(doc, &params)
        .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn invoice_shows_cargo_transport_under_the_detraction() {
    let html = render(&invoice(Some(CargoTransport {
        origen: point("150101", "AV. DEMO 123, LIMA"),
        destino: point("040101", "CALLE DESTINO <456>"),
        detalle_viaje: s("Carga general en bodega"),
        valor_ref_servicio: d(110_000, 2),
        valor_ref_carga_efectiva: d(105_000, 2),
        valor_ref_carga_util: d(120_000, 2),
    })));
    let cuenta = html.find("00-000-000000").unwrap_or_default();
    let carga = html.find("TRANSPORTE DE CARGA:").unwrap_or_default();
    assert!(
        cuenta > 0 && cuenta < carga,
        "debajo de la cuenta de detracciones"
    );
    for expected in [
        "Origen: 150101 - AV. DEMO 123, LIMA",
        "Destino: 040101 - CALLE DESTINO &lt;456&gt;",
        "Detalle del viaje: Carga general en bodega",
        "Valor referencial del servicio: 1100.00",
        "Valor referencial sobre la carga efectiva: 1050.00",
        "Valor referencial sobre la carga útil nominal: 1200.00",
    ] {
        assert!(html.contains(expected), "falta {expected:?}");
    }
}

#[test]
fn missing_parts_are_omitted() {
    let html = render(&invoice(Some(CargoTransport {
        valor_ref_servicio: d(110_000, 2),
        ..CargoTransport::default()
    })));
    assert!(html.contains("TRANSPORTE DE CARGA:"));
    assert!(!html.contains("Origen:"));
    assert!(!html.contains("Destino:"));
    assert!(!html.contains("Detalle del viaje:"));
    assert!(!html.contains("carga efectiva:"));
}

#[test]
fn invoice_without_cargo_transport_is_unchanged() {
    let html = render(&invoice(None));
    assert!(html.contains("00-000-000000"));
    assert!(!html.contains("TRANSPORTE DE CARGA"));
}
