//! Generación → parseo: XML generados con cpeforge-xml (UBL 2.0 y 2.1) y
//! leídos de vuelta con los parsers. Los parsers de la referencia leen rutas UBL
//! 2.0; en UBL 2.1 la factura falla como en la referencia (prefijo `sac` no declarado).

use std::str::FromStr;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::{Despatch, DespatchDetail, Direction, Shipment, Transportist};
use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use cpeforge_core::model::retention::{Exchange, Payment, Retention, RetentionDetail};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, Note, SaleDetail};
use cpeforge_core::model::summary::{Summary, SummaryDetail};
use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};
use cpeforge_core::ports::DocumentParser;
use cpeforge_core::{Decimal, Zoned};
use cpeforge_xml::{
    DespatchBuilder, InvoiceBuilder, NoteBuilder, PerceptionBuilder, RenderOptions,
    RetentionBuilder, SummaryBuilder, VoidedBuilder,
};
use cpeforge_xml_parser::{
    DespatchParser, InvoiceParser, NoteParser, PerceptionParser, RetentionParser, SummaryParser,
    VoidedDocument, VoidedParser, XmlParserError,
};

#[path = "support/xsd.rs"]
mod xsd;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn dec(text: &str) -> Decimal {
    Decimal::from_str(text).unwrap_or_default()
}

fn date(text: &str) -> Zoned {
    Zoned::from_str(text).unwrap_or_default()
}

fn lima(day: &str) -> Zoned {
    date(&format!("{day}T00:00:00-05:00[America/Lima]"))
}

fn company() -> Company {
    Company {
        ruc: Some("20000000001".into()),
        razon_social: Some("EMPRESA SINTETICA S.A.C.".into()),
        nombre_comercial: Some("COMERCIAL SINTETICO".into()),
        address: Some(Address {
            ubigueo: Some("150101".into()),
            departamento: Some("LIMA".into()),
            provincia: Some("LIMA".into()),
            distrito: Some("LIMA".into()),
            direccion: Some("AV. SINTETICA 123".into()),
            ..Address::default()
        }),
        ..Company::default()
    }
}

fn client() -> Client {
    Client {
        tipo_doc: Some("6".into()),
        num_doc: Some("20000000002".into()),
        rzn_social: Some("CLIENTE SINTETICO S.A.".into()),
        ..Client::default()
    }
}

fn sale(ubl: &str, tipo_doc: &str, serie: &str) -> BaseSale {
    BaseSale {
        ubl_version: Some(ubl.into()),
        tipo_doc: Some(tipo_doc.into()),
        serie: Some(serie.into()),
        correlativo: Some("123".into()),
        fecha_emision: Some(lima("2024-01-15")),
        tipo_moneda: Some("PEN".into()),
        company: Some(company()),
        client: Some(client()),
        mto_oper_gravadas: Some(dec("100.00")),
        mto_oper_inafectas: Some(dec("20.50")),
        mto_igv: Some(dec("18.00")),
        mto_isc: Some(dec("2.00")),
        total_impuestos: Some(dec("20.00")),
        mto_imp_venta: Some(dec("140.50")),
        legends: Some(vec![Legend {
            code: Some("1000".into()),
            value: Some("SON CIENTO CUARENTA CON 50/100 SOLES".into()),
        }]),
        details: Some(vec![SaleDetail {
            cod_producto: Some("P001".into()),
            cod_prod_sunat: Some("10191509".into()),
            unidad: Some("NIU".into()),
            cantidad: Some(dec("2")),
            descripcion: Some("PRODUCTO <SINTETICO> & «ñ»".into()),
            mto_valor_unitario: Some(dec("50.00")),
            mto_base_igv: Some(dec("100.00")),
            porcentaje_igv: Some(dec("18")),
            igv: Some(dec("18.00")),
            tip_afe_igv: Some("10".into()),
            total_impuestos: Some(dec("18.00")),
            mto_valor_venta: Some(dec("100.00")),
            mto_precio_unitario: Some(dec("59.00")),
            ..SaleDetail::default()
        }]),
        ..BaseSale::default()
    }
}

fn assert_sale_header(parsed: &BaseSale, source: &BaseSale) {
    assert_eq!(parsed.serie, source.serie);
    assert_eq!(parsed.correlativo, source.correlativo);
    assert_eq!(parsed.tipo_moneda, source.tipo_moneda);
    assert_eq!(parsed.fecha_emision, source.fecha_emision);
    let (pc, sc) = (parsed.company.clone().unwrap_or_default(), company());
    assert_eq!(pc.ruc, sc.ruc);
    assert_eq!(pc.razon_social, sc.razon_social);
    assert_eq!(pc.nombre_comercial, sc.nombre_comercial);
    let address = pc.address.unwrap_or_default();
    assert_eq!(address.ubigueo.as_deref(), Some("150101"));
    assert_eq!(address.direccion.as_deref(), Some("AV. SINTETICA 123"));
    let cl = parsed.client.clone().unwrap_or_default();
    assert_eq!(
        (cl.tipo_doc, cl.num_doc, cl.rzn_social),
        (
            Some("6".into()),
            Some("20000000002".into()),
            Some("CLIENTE SINTETICO S.A.".into())
        )
    );
    assert_eq!(parsed.mto_oper_gravadas, Some(dec("100")));
    assert_eq!(parsed.mto_oper_inafectas, Some(dec("20.5")));
    assert_eq!(parsed.mto_igv, Some(dec("18")));
    assert_eq!(parsed.mto_isc, Some(dec("2")));
    assert_eq!(parsed.mto_imp_venta, Some(dec("140.5")));
    assert_eq!(parsed.legends, source.legends);
    let details = parsed.details.clone().unwrap_or_default();
    assert_eq!(details.len(), 1);
    let (d, s) = (&details[0], &source.details.clone().unwrap_or_default()[0]);
    assert_eq!(d.cod_producto, s.cod_producto);
    assert_eq!(d.cod_prod_sunat, s.cod_prod_sunat);
    assert_eq!(d.unidad, s.unidad);
    assert_eq!(d.cantidad, Some(dec("2")));
    assert_eq!(d.descripcion, s.descripcion);
    assert_eq!(d.mto_valor_unitario, Some(dec("50")));
    assert_eq!(d.mto_valor_venta, Some(dec("100")));
    assert_eq!(d.mto_precio_unitario, Some(dec("59")));
    assert_eq!(d.igv, Some(dec("18")));
    assert_eq!(d.tip_afe_igv.as_deref(), Some("10"));
}

#[test]
fn invoice_ubl20_roundtrip() -> TestResult {
    let invoice = Invoice {
        base: sale("2.0", "01", "F001"),
        tipo_operacion: Some("0101".into()),
        fec_vencimiento: Some(lima("2024-02-15")),
        ..Invoice::default()
    };
    let xml = InvoiceBuilder::new(RenderOptions::default()).render(&invoice)?;
    xsd::assert_valid(&xml, "invoice-ubl20")?;
    let parsed = InvoiceParser::new()
        .parse(xml.as_bytes())?
        .ok_or("sin factura")?;
    assert_sale_header(&parsed.base, &invoice.base);
    assert_eq!(parsed.base.tipo_doc.as_deref(), Some("01"));
    assert_eq!(parsed.tipo_operacion.as_deref(), Some("0101"));
    assert_eq!(parsed.fec_vencimiento, invoice.fec_vencimiento);
    Ok(())
}

#[test]
fn invoice_ubl21_fails_like_reference() -> TestResult {
    let invoice = Invoice {
        base: sale("2.1", "01", "F001"),
        tipo_operacion: Some("0101".into()),
        ..Invoice::default()
    };
    let xml = InvoiceBuilder::new(RenderOptions::default()).render(&invoice)?;
    xsd::assert_valid(&xml, "invoice-ubl21")?;
    let err = InvoiceParser::new()
        .parse(xml.as_bytes())
        .err()
        .ok_or("se esperaba error")?;
    assert_eq!(err.error_class(), Some("Error"));
    assert_eq!(
        err.error_message().as_deref(),
        Some("Call to a member function item() on false")
    );
    Ok(())
}

#[test]
fn notes_roundtrip() -> TestResult {
    for (tipo, serie, expected) in [("07", "FC01", "07"), ("08", "FD01", "08")] {
        let note = Note {
            base: sale("2.0", tipo, serie),
            cod_motivo: Some("01".into()),
            des_motivo: Some("ANULACION".into()),
            tip_doc_afectado: Some("01".into()),
            num_doc_afectado: Some("F001-1".into()),
            ..Note::default()
        };
        let xml = NoteBuilder::new(RenderOptions::default()).render(&note)?;
        xsd::assert_valid(&xml, "note")?;
        let parsed = NoteParser::new().parse(xml.as_bytes())?.ok_or("sin nota")?;
        assert_sale_header(&parsed.base, &note.base);
        assert_eq!(parsed.base.tipo_doc.as_deref(), Some(expected));
        assert_eq!(parsed.cod_motivo, note.cod_motivo);
        assert_eq!(parsed.des_motivo, note.des_motivo);
        assert_eq!(parsed.tip_doc_afectado, note.tip_doc_afectado);
        assert_eq!(parsed.num_doc_afectado, note.num_doc_afectado);
    }
    Ok(())
}

#[test]
fn despatch_roundtrip() -> TestResult {
    let despatch = Despatch {
        // Plantilla anterior a 2022: la de 2022 escribe `cbc:Description` y
        // `cac:AddressLine`, que el parser de la referencia no lee (lo muestra el caso de
        // paridad despatch-ubl-desp2022-full-default).
        version: None,
        tipo_doc: Some("09".into()),
        serie: Some("T001".into()),
        correlativo: Some("45".into()),
        observacion: Some("OBSERVACION".into()),
        fecha_emision: Some(lima("2024-04-01")),
        company: Some(company()),
        destinatario: Some(client()),
        envio: Some(Shipment {
            cod_traslado: Some("01".into()),
            mod_traslado: Some("01".into()),
            fec_traslado: Some(lima("2024-04-02")),
            peso_total: Some(dec("12.5")),
            und_peso_total: Some("KGM".into()),
            num_bultos: Some(3),
            llegada: Some(Direction {
                ubigueo: Some("150203".into()),
                direccion: Some("JR. LLEGADA 1".into()),
                ..Direction::default()
            }),
            partida: Some(Direction {
                ubigueo: Some("150101".into()),
                direccion: Some("AV. PARTIDA 9".into()),
                ..Direction::default()
            }),
            transportista: Some(Transportist {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000004".into()),
                rzn_social: Some("TRANSPORTES SINTETICOS".into()),
                ..Transportist::default()
            }),
            ..Shipment::default()
        }),
        details: Some(vec![DespatchDetail {
            cantidad: Some(dec("1.5")),
            unidad: Some("NIU".into()),
            descripcion: Some("BIEN SINTETICO".into()),
            codigo: Some("B01".into()),
            ..DespatchDetail::default()
        }]),
        ..Despatch::default()
    };
    let xml = DespatchBuilder::new(RenderOptions::default()).render(&despatch)?;
    xsd::assert_valid(&xml, "despatch")?;
    let parsed = DespatchParser::new()
        .parse(xml.as_bytes())?
        .ok_or("sin guía")?;
    assert_eq!(parsed.serie, despatch.serie);
    assert_eq!(parsed.correlativo, despatch.correlativo);
    assert_eq!(parsed.tipo_doc, despatch.tipo_doc);
    assert_eq!(parsed.fecha_emision, despatch.fecha_emision);
    assert_eq!(parsed.observacion, despatch.observacion);
    let details = parsed.details.unwrap_or_default();
    assert_eq!(details.len(), 1);
    assert_eq!(details[0].cantidad, Some(dec("1.5")));
    assert_eq!(details[0].unidad.as_deref(), Some("NIU"));
    assert_eq!(details[0].descripcion.as_deref(), Some("BIEN SINTETICO"));
    assert_eq!(details[0].codigo.as_deref(), Some("B01"));
    let envio = parsed.envio.unwrap_or_default();
    assert_eq!(envio.cod_traslado.as_deref(), Some("01"));
    assert_eq!(envio.fec_traslado, Some(lima("2024-04-02")));
    assert_eq!(envio.peso_total, Some(dec("12.5")));
    assert_eq!(envio.und_peso_total.as_deref(), Some("KGM"));
    assert_eq!(
        envio.llegada.and_then(|d| d.direccion).as_deref(),
        Some("JR. LLEGADA 1")
    );
    assert_eq!(
        envio.partida.and_then(|d| d.ubigueo).as_deref(),
        Some("150101")
    );
    Ok(())
}

#[test]
fn summary_roundtrip() -> TestResult {
    let summary = Summary {
        correlativo: Some("001".into()),
        fec_generacion: Some(lima("2024-05-31")),
        fec_resumen: Some(lima("2024-06-01")),
        company: Some(company()),
        details: Some(vec![SummaryDetail {
            tipo_doc: Some("03".into()),
            serie_nro: Some("B001-1".into()),
            cliente_tipo: Some("1".into()),
            cliente_nro: Some("00000001".into()),
            estado: Some("1".into()),
            total: Some(dec("118")),
            mto_oper_gravadas: Some(dec("100")),
            mto_igv: Some(dec("18")),
            ..SummaryDetail::default()
        }]),
        ..Summary::default()
    };
    let xml = SummaryBuilder::new(RenderOptions::default()).render(&summary)?;
    xsd::assert_valid(&xml, "summary")?;
    let parsed = SummaryParser::new()
        .parse(xml.as_bytes())?
        .ok_or("sin resumen")?;
    assert_eq!(parsed.correlativo, summary.correlativo);
    assert_eq!(parsed.fec_generacion, summary.fec_generacion);
    assert_eq!(parsed.fec_resumen, summary.fec_resumen);
    assert_eq!(
        parsed.company.and_then(|c| c.ruc).as_deref(),
        Some("20000000001")
    );
    let details = parsed.details.unwrap_or_default();
    assert_eq!(details.len(), 1);
    let d = &details[0];
    assert_eq!(d.tipo_doc.as_deref(), Some("03"));
    assert_eq!(d.serie_nro.as_deref(), Some("B001-1"));
    assert_eq!(d.cliente_nro.as_deref(), Some("00000001"));
    assert_eq!(d.estado.as_deref(), Some("1"));
    assert_eq!(d.total, Some(dec("118")));
    assert_eq!(d.mto_oper_gravadas, Some(dec("100")));
    assert_eq!(d.mto_igv, Some(dec("18")));
    Ok(())
}

#[test]
fn voided_and_reversion_roundtrip() -> TestResult {
    let voided = Voided {
        correlativo: Some("7".into()),
        fec_generacion: Some(lima("2024-05-30")),
        fec_comunicacion: Some(lima("2024-06-01")),
        company: Some(company()),
        details: Some(vec![VoidedDetail {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("101".into()),
            des_motivo_baja: Some("ERROR".into()),
        }]),
    };
    let builder = VoidedBuilder::new(RenderOptions::default());
    let parser = VoidedParser::new();
    let xml = builder.render(&voided)?;
    xsd::assert_valid(&xml, "voided")?;
    let parsed = parser.parse(xml.as_bytes())?.ok_or("sin baja")?;
    assert!(matches!(parsed, VoidedDocument::Voided(_)));
    // La plantilla de bajas no incluye nombre comercial ni dirección.
    let expected = Voided {
        company: Some(Company {
            nombre_comercial: Some(String::new()),
            address: None,
            ..company()
        }),
        ..voided.clone()
    };
    assert_eq!(parsed.voided(), &expected);
    let reversion = Reversion {
        voided: voided.clone(),
    };
    let xml = builder.render_reversion(&reversion)?;
    xsd::assert_valid(&xml, "reversion")?;
    let parsed = parser.parse(xml.as_bytes())?.ok_or("sin reversión")?;
    assert!(matches!(parsed, VoidedDocument::Reversion(_)));
    assert_eq!(parsed.voided(), &expected);
    Ok(())
}

#[test]
fn retention_and_perception_roundtrip() -> TestResult {
    let payment = Payment {
        moneda: Some("PEN".into()),
        importe: Some(dec("100")),
        fecha: Some(lima("2024-07-01")),
    };
    let exchange = Exchange {
        moneda_ref: Some("USD".into()),
        moneda_obj: Some("PEN".into()),
        factor: Some(dec("3.785")),
        fecha: Some(lima("2024-07-01")),
    };
    let retention = Retention {
        serie: Some("R001".into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(lima("2024-07-03")),
        company: Some(company()),
        proveedor: Some(client()),
        regimen: Some("01".into()),
        tasa: Some(dec("3")),
        imp_retenido: Some(dec("3")),
        imp_pagado: Some(dec("97")),
        observacion: Some("NOTA".into()),
        details: Some(vec![RetentionDetail {
            tipo_doc: Some("01".into()),
            num_doc: Some("F001-1".into()),
            fecha_emision: Some(lima("2024-06-20")),
            imp_total: Some(dec("100")),
            moneda: Some("PEN".into()),
            pagos: Some(vec![payment.clone()]),
            fecha_retencion: Some(lima("2024-07-02")),
            imp_retenido: Some(dec("3")),
            imp_pagar: Some(dec("97")),
            tipo_cambio: Some(exchange.clone()),
        }]),
    };
    let xml = RetentionBuilder::new(RenderOptions::default()).render(&retention)?;
    xsd::assert_valid(&xml, "retention")?;
    let parsed = RetentionParser::new()
        .parse(xml.as_bytes())?
        .ok_or("sin retención")?;
    assert_eq!(parsed.serie, retention.serie);
    assert_eq!(parsed.correlativo, retention.correlativo);
    assert_eq!(parsed.fecha_emision, retention.fecha_emision);
    assert_eq!(parsed.regimen, retention.regimen);
    assert_eq!(parsed.tasa, Some(dec("3")));
    assert_eq!(parsed.imp_retenido, Some(dec("3")));
    assert_eq!(parsed.imp_pagado, Some(dec("97")));
    assert_eq!(
        parsed.proveedor.and_then(|p| p.num_doc).as_deref(),
        Some("20000000002")
    );
    let d = parsed.details.unwrap_or_default();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].num_doc.as_deref(), Some("F001-1"));
    assert_eq!(d[0].imp_retenido, Some(dec("3")));
    assert_eq!(d[0].fecha_retencion, Some(lima("2024-07-02")));
    assert_eq!(
        d[0].tipo_cambio.as_ref().and_then(|e| e.factor),
        Some(dec("3.785"))
    );
    // La referencia lee `cbc:PaidDate` sin contexto (desde el raíz): el pago queda con
    // la fecha actual aunque el XML la tenga.
    assert_eq!(d[0].pagos.as_ref().map(Vec::len), Some(1));

    let perception = Perception {
        serie: Some("P001".into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(lima("2024-07-03")),
        company: Some(company()),
        proveedor: Some(client()),
        regimen: Some("01".into()),
        tasa: Some(dec("2")),
        imp_percibido: Some(dec("2")),
        imp_cobrado: Some(dec("102")),
        observacion: Some("NOTA".into()),
        details: Some(vec![PerceptionDetail {
            tipo_doc: Some("01".into()),
            num_doc: Some("F001-1".into()),
            fecha_emision: Some(lima("2024-06-20")),
            imp_total: Some(dec("100")),
            moneda: Some("PEN".into()),
            cobros: Some(vec![payment]),
            fecha_percepcion: Some(lima("2024-07-02")),
            imp_percibido: Some(dec("2")),
            imp_cobrar: Some(dec("102")),
            tipo_cambio: Some(exchange),
        }]),
    };
    let xml = PerceptionBuilder::new(RenderOptions::default()).render(&perception)?;
    xsd::assert_valid(&xml, "perception")?;
    let parsed = PerceptionParser::new()
        .parse(xml.as_bytes())?
        .ok_or("sin percepción")?;
    assert_eq!(parsed.serie, perception.serie);
    assert_eq!(parsed.imp_percibido, Some(dec("2")));
    assert_eq!(parsed.imp_cobrado, Some(dec("102")));
    let d = parsed.details.unwrap_or_default();
    assert_eq!(d[0].imp_cobrar, Some(dec("102")));
    assert_eq!(d[0].fecha_percepcion, Some(lima("2024-07-02")));
    Ok(())
}

#[test]
fn errors_are_typed() {
    let err = InvoiceParser::new().parse(b"").err();
    assert_eq!(err, Some(XmlParserError::Empty));
    assert_eq!(
        XmlParserError::Empty.error_message().as_deref(),
        Some("DOMDocument::loadXML(): Argument #1 ($source) must not be empty")
    );
}

/// El validador XSD de estas pruebas rechaza un documento inválido (no es
/// una comprobación vacía).
#[test]
fn xsd_check_rejects_invalid_documents() {
    let invalid = r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><cbc:ID>F001-1</cbc:ID></Invoice>"#;
    assert!(xsd::assert_valid(invalid, "invalid").is_err());
}
