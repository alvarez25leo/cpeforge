//! Liquidación de compra (tipo 04): plantilla propia `settlement.html.twig`.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "pruebas")]

use cpeforge_core::Decimal;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{DetailAttribute, Legend, RelatedDocument};
use cpeforge_core::model::settlement::{
    PurchaseSettlement, SettlementDetail, SettlementPlace, SettlementSeller,
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

fn address(line: &str) -> Option<Address> {
    Some(Address {
        ubigueo: s("120301"),
        departamento: s("JUNÍN"),
        provincia: s("CHANCHAMAYO"),
        distrito: s("CHANCHAMAYO"),
        direccion: s(line),
        ..Address::default()
    })
}

fn settlement() -> PurchaseSettlement {
    PurchaseSettlement {
        tipo_operacion: s("0503"),
        serie: s("L001"),
        correlativo: s("2"),
        fecha_emision: Some("2026-10-20T09:00:00-05:00[America/Lima]".parse().unwrap()),
        tipo_moneda: s("PEN"),
        company: Some(Company {
            ruc: s("20000000001"),
            razon_social: s("ACOPIADORA DEMO S.A.C."),
            address: address("AV. DEMO 123"),
            ..Company::default()
        }),
        vendedor: Some(SettlementSeller {
            tipo_doc: s("1"),
            num_doc: s("40123456"),
            rzn_social: s("QUISPE MAMANI JUAN"),
            address: address("SECTOR SAN RAMÓN"),
            condicion: s("03"),
        }),
        lugar_operacion: Some(SettlementPlace {
            address: address("PLANTA DE ACOPIO KM 12"),
            condicion: s("01"),
        }),
        guias: Some(vec![RelatedDocument {
            tipo_doc: s("09"),
            nro_doc: s("T001-15"),
        }]),
        mto_oper_gravadas: d(1710000, 2),
        mto_igv: d(307800, 2),
        mto_base_ir: d(1710000, 2),
        mto_ir: d(68400, 2),
        total_impuestos: d(307800, 2),
        valor_venta: d(1710000, 2),
        sub_total: d(2017800, 2),
        mto_imp_venta: d(1641600, 2),
        legends: Some(vec![Legend {
            code: s("1000"),
            value: s("SON DIECISEIS MIL CUATROCIENTOS DIECISEIS CON 00/100 SOLES"),
        }]),
        details: Some(vec![SettlementDetail {
            unidad: s("GRM"),
            cantidad: d(60, 0),
            cod_producto: s("ORO01"),
            descripcion: s("ORO EN BRUTO"),
            mto_valor_unitario: d(28500, 2),
            mto_valor_venta: d(1710000, 2),
            tip_afe_igv: s("10"),
            igv: d(307800, 2),
            porcentaje_ir: d(4, 0),
            ir: d(68400, 2),
            atributos: Some(vec![DetailAttribute {
                code: s("6006"),
                name: s("Nombre del derecho minero"),
                value: s("MINA DEMO"),
                ..DetailAttribute::default()
            }]),
            ..SettlementDetail::default()
        }]),
        ..PurchaseSettlement::default()
    }
}

fn render(doc: &PurchaseSettlement) -> String {
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
    params.system.hash = s("abc123");
    report.render_html(doc, &params).unwrap()
}

#[test]
fn resolver_picks_the_settlement_template() {
    assert_eq!(
        DefaultTemplateResolver.template(&settlement()).unwrap(),
        "settlement.html.twig"
    );
}

#[test]
fn settlement_shows_seller_operation_retentions_and_net_amount() {
    let html = render(&settlement());
    for expected in [
        "LIQUIDACIÓN DE COMPRA",
        "R.U.C.: 20000000001",
        "L001-2",
        "QUISPE MAMANI JUAN",
        "DNI:",
        "Extracción",
        "Compra de oro",
        "PLANTA DE ACOPIO KM 12",
        "GUÍA DE REMISIÓN REMITENTE",
        "T001-15",
        "Nombre del derecho minero: MINA DEMO",
        "(4%)",
        "IMPORTE TOTAL NETO",
        "16416.00",
        "SON DIECISEIS MIL",
        "abc123",
    ] {
        assert!(html.contains(expected), "falta {expected}");
    }
}

#[test]
fn unknown_codes_are_printed_as_is() {
    let mut doc = settlement();
    if let Some(v) = doc.vendedor.as_mut() {
        v.condicion = s("99");
    }
    doc.tipo_operacion = s("0599");
    let html = render(&doc);
    assert!(html.contains("99"));
    assert!(html.contains("0599"));
}
