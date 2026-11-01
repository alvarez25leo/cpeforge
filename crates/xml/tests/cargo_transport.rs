//! Factura con detracción por servicio de transporte de carga (operación
//! `1004`): información de la línea del anexo N.º 9-A de la
//! R. S. 114-2019/SUNAT (datos 113 a 118) en `cac:InvoiceLine/cac:Delivery`,
//! validada con `xmllint` contra el XSD UBL 2.1 de `Invoice`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::Direction;
use cpeforge_core::model::sale::{
    BaseSale, CargoTransport, Detraction, Invoice, Legend, Note, PaymentTerms, SaleDetail,
};
use cpeforge_xml::{InvoiceBuilder, NoteBuilder, RenderOptions};
use roxmltree::{Document, Node};

const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const CAC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2";

fn s(value: &str) -> Option<String> {
    Some(value.to_owned())
}

fn d(value: i64, scale: u32) -> Option<Decimal> {
    Some(Decimal::new(value, scale))
}

fn point(ubigeo: &str, direccion: &str) -> Direction {
    Direction {
        ubigueo: s(ubigeo),
        direccion: s(direccion),
        ..Direction::default()
    }
}

fn cargo() -> CargoTransport {
    CargoTransport {
        origen: Some(point("150101", "AV. DEMO 123, LIMA")),
        destino: Some(point("040101", "CALLE DESTINO 456, AREQUIPA")),
        detalle_viaje: s("Carga general en bodega, Lima - Arequipa"),
        valor_ref_servicio: d(110_000, 2),
        valor_ref_carga_efectiva: d(105_000, 2),
        valor_ref_carga_util: d(120_000, 2),
    }
}

/// Servicio de transporte de carga: S/ 1 000 + IGV 18 %.
fn line(transporte_carga: Option<CargoTransport>) -> SaleDetail {
    SaleDetail {
        unidad: s("ZZ"),
        cantidad: d(1, 0),
        cod_producto: s("TRC01"),
        descripcion: s("Servicio de transporte de carga"),
        mto_valor_unitario: d(100_000, 2),
        mto_valor_venta: d(100_000, 2),
        mto_base_igv: d(100_000, 2),
        porcentaje_igv: d(18, 0),
        igv: d(18_000, 2),
        tip_afe_igv: s("10"),
        total_impuestos: d(18_000, 2),
        mto_precio_unitario: d(118_000, 2),
        transporte_carga,
        ..SaleDetail::default()
    }
}

fn base(tipo_doc: &str, serie: &str, details: Vec<SaleDetail>) -> BaseSale {
    BaseSale {
        ubl_version: s("2.1"),
        tipo_doc: s(tipo_doc),
        serie: s(serie),
        correlativo: s("123"),
        fecha_emision: Some("2026-10-31T10:00:00-05:00[America/Lima]".parse().unwrap()),
        tipo_moneda: s("PEN"),
        company: Some(Company {
            ruc: s("20000000001"),
            razon_social: s("TRANSPORTES DEMO S.A.C."),
            nombre_comercial: s("TRANSPORTES DEMO"),
            address: Some(Address {
                ubigueo: s("150101"),
                departamento: s("LIMA"),
                provincia: s("LIMA"),
                distrito: s("LIMA"),
                direccion: s("AV. DEMO 123"),
                cod_local: s("0000"),
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
        legends: Some(vec![
            Legend {
                code: s("1000"),
                value: s("SON MIL CIENTO OCHENTA CON 00/100 SOLES"),
            },
            Legend {
                code: s("2006"),
                value: s("Operación sujeta a detracción"),
            },
        ]),
        forma_pago: Some(PaymentTerms {
            moneda: s("PEN"),
            tipo: s("Contado"),
            monto: None,
        }),
        details: Some(details),
        ..BaseSale::default()
    }
}

fn invoice(details: Vec<SaleDetail>) -> Invoice {
    Invoice {
        base: base("01", "F001", details),
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
    InvoiceBuilder::new(RenderOptions::default())
        .render(doc)
        .unwrap()
}

fn xsd_dir() -> PathBuf {
    std::env::var_os("CPEFORGE_XSD_DIR").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/reference/ubl-validator-v2.3.0/src/xsd")
        },
        PathBuf::from,
    )
}

/// Valida con `xmllint` (con un nodo en `ExtensionContent`, como el resto de
/// pruebas de esquema, porque la firma aún no está).
fn assert_valid_xsd(xml: &str, name: &str) {
    let schema = xsd_dir().join("2.1/maindoc/UBL-Invoice-2.1.xsd");
    assert!(
        schema.is_file(),
        "faltan los XSD: ejecutar scripts/prepare_ubl_validator_reference.sh"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cargo-transport");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.xml"));
    fs::write(
        &path,
        xml.replacen(
            "<ext:ExtensionContent/>",
            "<ext:ExtensionContent><cbc:AccountID>1</cbc:AccountID></ext:ExtensionContent>",
            1,
        ),
    )
    .unwrap();
    let output = Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&path)
        .output()
        .expect("xmllint disponible");
    assert!(
        output.status.success(),
        "{name} no valida contra el XSD:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn child<'a, 'i>(node: Node<'a, 'i>, ns: &str, name: &str) -> Node<'a, 'i> {
    node.children()
        .find(|n| n.has_tag_name((ns, name)))
        .unwrap_or_else(|| panic!("falta {name} en {:?}", node.tag_name().name()))
}

fn path<'a, 'i>(node: Node<'a, 'i>, steps: &[(&str, &str)]) -> Node<'a, 'i> {
    steps
        .iter()
        .fold(node, |current, (ns, name)| child(current, ns, name))
}

fn text<'a>(node: Node<'a, '_>) -> &'a str {
    node.text().unwrap_or_default()
}

fn element_names<'a>(node: Node<'a, '_>) -> Vec<&'a str> {
    node.children()
        .filter(Node::is_element)
        .map(|n| n.tag_name().name())
        .collect()
}

#[test]
fn full_cargo_transport_is_valid_and_places_each_datum() {
    let xml = render(&invoice(vec![line(Some(cargo()))]));
    assert_valid_xsd(&xml, "full");
    let doc = Document::parse(&xml).unwrap();
    let root = doc.root_element();
    assert_eq!(
        child(root, CBC, "InvoiceTypeCode").attribute("listID"),
        Some("1004")
    );

    // La detracción de la cabecera no cambia: bien o servicio 027.
    let detraction = root
        .children()
        .filter(|n| n.has_tag_name((CAC, "PaymentTerms")))
        .find(|n| text(child(*n, CBC, "ID")) == "Detraccion")
        .unwrap();
    assert_eq!(text(child(detraction, CBC, "PaymentMeansID")), "027");

    // `cac:Delivery` va entre `cac:PricingReference` y `cac:TaxTotal` (XSD).
    let line = child(root, CAC, "InvoiceLine");
    let names = element_names(line);
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
    assert!(at("PricingReference") < at("Delivery"));
    assert!(at("Delivery") < at("TaxTotal"));
    let delivery = child(line, CAC, "Delivery");
    assert_eq!(
        element_names(delivery),
        [
            "DeliveryLocation",
            "Despatch",
            "DeliveryTerms",
            "DeliveryTerms",
            "DeliveryTerms"
        ]
    );

    // Dato 114: punto de destino.
    let destino = path(delivery, &[(CAC, "DeliveryLocation"), (CAC, "Address")]);
    let ubigeo = child(destino, CBC, "ID");
    assert_eq!(text(ubigeo), "040101");
    assert_eq!(ubigeo.attribute("schemeAgencyName"), Some("PE:INEI"));
    assert_eq!(ubigeo.attribute("schemeName"), Some("Ubigeos"));
    assert_eq!(
        text(path(destino, &[(CAC, "AddressLine"), (CBC, "Line")])),
        "CALLE DESTINO 456, AREQUIPA"
    );

    // Datos 113 y 115: punto de origen y detalle del viaje.
    let despatch = child(delivery, CAC, "Despatch");
    assert_eq!(element_names(despatch), ["Instructions", "DespatchAddress"]);
    assert_eq!(
        text(child(despatch, CBC, "Instructions")),
        "Carga general en bodega, Lima - Arequipa"
    );
    let origen = child(despatch, CAC, "DespatchAddress");
    assert_eq!(text(child(origen, CBC, "ID")), "150101");
    assert_eq!(
        text(path(origen, &[(CAC, "AddressLine"), (CBC, "Line")])),
        "AV. DEMO 123, LIMA"
    );

    // Datos 116 a 118: un valor de cada tipo referencial, en soles.
    let terms: Vec<(&str, &str, Option<&str>)> = delivery
        .children()
        .filter(|n| n.has_tag_name((CAC, "DeliveryTerms")))
        .map(|n| {
            let amount = child(n, CBC, "Amount");
            (
                text(child(n, CBC, "ID")),
                text(amount),
                amount.attribute("currencyID"),
            )
        })
        .collect();
    assert_eq!(
        terms,
        [
            ("01", "1100.00", Some("PEN")),
            ("02", "1050.00", Some("PEN")),
            ("03", "1200.00", Some("PEN")),
        ]
    );
}

#[test]
fn line_without_cargo_transport_has_no_delivery() {
    let xml = render(&invoice(vec![line(None), line(Some(cargo()))]));
    assert_valid_xsd(&xml, "mixed");
    let doc = Document::parse(&xml).unwrap();
    let lines: Vec<bool> = doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name((CAC, "InvoiceLine")))
        .map(|n| n.children().any(|c| c.has_tag_name((CAC, "Delivery"))))
        .collect();
    assert_eq!(lines, [false, true]);
}

#[test]
fn partial_data_emits_only_the_given_parts() {
    let only_values = CargoTransport {
        valor_ref_servicio: d(110_000, 2),
        ..CargoTransport::default()
    };
    let xml = render(&invoice(vec![line(Some(only_values))]));
    assert_valid_xsd(&xml, "partial");
    let doc = Document::parse(&xml).unwrap();
    let delivery = path(
        doc.root_element(),
        &[(CAC, "InvoiceLine"), (CAC, "Delivery")],
    );
    assert_eq!(element_names(delivery), ["DeliveryTerms"]);

    let only_origin = CargoTransport {
        origen: Some(point("150101", "")),
        ..CargoTransport::default()
    };
    let xml = render(&invoice(vec![line(Some(only_origin))]));
    assert_valid_xsd(&xml, "origin-without-address");
    let doc = Document::parse(&xml).unwrap();
    let despatch = path(
        doc.root_element(),
        &[(CAC, "InvoiceLine"), (CAC, "Delivery"), (CAC, "Despatch")],
    );
    assert_eq!(element_names(despatch), ["DespatchAddress"]);
    assert_eq!(
        element_names(child(despatch, CAC, "DespatchAddress")),
        ["ID"]
    );
}

/// La salida pasa por `spaceless` (espacios entre `>` y `<`), también dentro
/// de CDATA: los textos hostiles no llevan ese patrón.
#[test]
fn texts_are_escaped_and_cdata_stays_well_formed() {
    let hostile = CargoTransport {
        detalle_viaje: s("a ]]>x<cbc:ID>y</cbc:ID> & b"),
        destino: Some(point("040101\"><x>", "]]><cac:Item/>")),
        ..cargo()
    };
    let xml = render(&invoice(vec![line(Some(hostile))]));
    let doc = Document::parse(&xml).unwrap();
    let delivery = path(
        doc.root_element(),
        &[(CAC, "InvoiceLine"), (CAC, "Delivery")],
    );
    assert_eq!(
        path(delivery, &[(CAC, "Despatch"), (CBC, "Instructions")])
            .text()
            .unwrap(),
        "a ]]>x<cbc:ID>y</cbc:ID> & b"
    );
    let destino = path(delivery, &[(CAC, "DeliveryLocation"), (CAC, "Address")]);
    assert_eq!(text(child(destino, CBC, "ID")), "040101\"><x>");
    assert_eq!(
        path(destino, &[(CAC, "AddressLine"), (CBC, "Line")])
            .text()
            .unwrap(),
        "]]><cac:Item/>"
    );
    assert_eq!(element_names(destino), ["ID", "AddressLine"]);
}

#[test]
fn credit_note_never_emits_cargo_transport() {
    let note = Note {
        base: base("07", "F001", vec![line(Some(cargo()))]),
        cod_motivo: s("01"),
        des_motivo: s("ANULACIÓN DE LA OPERACIÓN"),
        tip_doc_afectado: s("01"),
        num_doc_afectado: s("F001-122"),
        ..Note::default()
    };
    let xml = NoteBuilder::new(RenderOptions::default())
        .render(&note)
        .unwrap();
    let doc = Document::parse(&xml).unwrap();
    assert!(!doc.descendants().any(|n| n.has_tag_name((CAC, "Delivery"))));
}
