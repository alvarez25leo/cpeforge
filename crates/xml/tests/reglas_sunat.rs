//! Reglas de emisión recientes: dirección del agente de retención y
//! percepción (`cbc:CityName` = provincia, `cbc:CountrySubentity` =
//! departamento), indicador de emisión excepcional de percepción
//! (`sac:ExceptionalIndicator`, campo 7 de «Percepciones1_0»), número de pago
//! (`cac:Payment/cbc:ID`), `TaxAmount` n(12,2) de los tributos sin impuesto
//! (EXO, INA y EXP) y dirección de entrega con partes opcionales (exportación
//! de servicios: solo el país). Los documentos completos se validan con
//! `xmllint` contra los XSD UBL fijados.

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
use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use cpeforge_core::model::retention::{Exchange, Payment, Retention, RetentionDetail};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, Note, PaymentTerms, SaleDetail};
use cpeforge_xml::{
    InvoiceBuilder, NoteBuilder, PerceptionBuilder, RenderOptions, RetentionBuilder,
};
use roxmltree::{Document, Node};

const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const CAC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2";
const SAC: &str = "urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1";

fn s(value: &str) -> Option<String> {
    Some(value.to_owned())
}

fn d(value: i64, scale: u32) -> Option<Decimal> {
    Some(Decimal::new(value, scale))
}

fn lima(text: &str) -> Option<cpeforge_core::Zoned> {
    Some(format!("{text}[America/Lima]").parse().unwrap())
}

fn address(provincia: &str, departamento: &str) -> Address {
    Address {
        ubigueo: s("150801"),
        departamento: s(departamento),
        provincia: s(provincia),
        distrito: s("HUACHO"),
        urbanizacion: s("CERCADO"),
        direccion: s("AV. DEMO 123"),
        codigo_pais: s("PE"),
        cod_local: s("0000"),
    }
}

fn company() -> Company {
    Company {
        ruc: s("20000000001"),
        razon_social: s("AGENTE DEMO S.A.C."),
        nombre_comercial: s("AGENTE DEMO"),
        address: Some(address("HUAURA", "LIMA")),
        ..Company::default()
    }
}

fn provider() -> Client {
    Client {
        tipo_doc: s("6"),
        num_doc: s("20000000002"),
        rzn_social: s("PROVEEDOR DEMO S.A."),
        ..Client::default()
    }
}

fn payment(numero: Option<i64>) -> Payment {
    Payment {
        numero,
        moneda: s("PEN"),
        importe: d(100_000, 2),
        fecha: lima("2026-11-02T00:00:00-05:00"),
    }
}

fn exchange() -> Exchange {
    Exchange {
        moneda_ref: s("PEN"),
        moneda_obj: s("PEN"),
        factor: d(1, 0),
        fecha: lima("2026-11-02T00:00:00-05:00"),
    }
}

fn perception(ind_excepcional: Option<&str>, numero: Option<i64>) -> Perception {
    Perception {
        serie: s("P001"),
        correlativo: s("2"),
        fecha_emision: lima("2026-11-03T10:00:00-05:00"),
        company: Some(company()),
        proveedor: Some(provider()),
        regimen: s("01"),
        tasa: d(2, 0),
        imp_percibido: d(2_000, 2),
        imp_cobrado: d(102_000, 2),
        observacion: s("PERCEPCION DEMO"),
        ind_excepcional: ind_excepcional.map(str::to_owned),
        details: Some(vec![PerceptionDetail {
            tipo_doc: s("01"),
            num_doc: s("F001-1"),
            fecha_emision: lima("2026-11-01T10:00:00-05:00"),
            imp_total: d(100_000, 2),
            moneda: s("PEN"),
            cobros: Some(vec![payment(numero)]),
            fecha_percepcion: lima("2026-11-03T00:00:00-05:00"),
            imp_percibido: d(2_000, 2),
            imp_cobrar: d(102_000, 2),
            tipo_cambio: Some(exchange()),
        }]),
    }
}

fn retention(numero: Option<i64>) -> Retention {
    Retention {
        serie: s("R001"),
        correlativo: s("2"),
        fecha_emision: lima("2026-11-03T10:00:00-05:00"),
        company: Some(company()),
        proveedor: Some(provider()),
        regimen: s("01"),
        tasa: d(3, 0),
        imp_retenido: d(3_000, 2),
        imp_pagado: d(97_000, 2),
        observacion: s("RETENCION DEMO"),
        details: Some(vec![RetentionDetail {
            tipo_doc: s("01"),
            num_doc: s("F001-1"),
            fecha_emision: lima("2026-11-01T10:00:00-05:00"),
            imp_total: d(100_000, 2),
            moneda: s("PEN"),
            pagos: Some(vec![payment(numero)]),
            fecha_retencion: lima("2026-11-03T00:00:00-05:00"),
            imp_retenido: d(3_000, 2),
            imp_pagar: d(97_000, 2),
            tipo_cambio: Some(exchange()),
        }]),
    }
}

/// Línea con el tipo de afectación indicado y sin impuesto (EXO, INA o EXP).
fn untaxed_line(code: &str, afectacion: &str, valor: i64) -> SaleDetail {
    SaleDetail {
        unidad: s("NIU"),
        cantidad: d(1, 0),
        cod_producto: s(code),
        descripcion: s("Producto sin IGV"),
        mto_valor_unitario: d(valor, 2),
        mto_valor_venta: d(valor, 2),
        mto_base_igv: d(valor, 2),
        porcentaje_igv: d(0, 0),
        igv: d(0, 0),
        tip_afe_igv: s(afectacion),
        total_impuestos: d(0, 0),
        mto_precio_unitario: d(valor, 2),
        ..SaleDetail::default()
    }
}

/// Venta con operaciones exoneradas (S/ 100), inafectas (S/ 50) y, si se
/// pide, de exportación (S/ 30).
fn sale(tipo_doc: &str, serie: &str, exportacion: bool) -> BaseSale {
    let mut details = vec![
        untaxed_line("EXO01", "20", 10_000),
        untaxed_line("INA01", "30", 5_000),
    ];
    let total = if exportacion {
        details.push(untaxed_line("EXP01", "40", 3_000));
        18_000
    } else {
        15_000
    };
    BaseSale {
        ubl_version: s("2.1"),
        tipo_doc: s(tipo_doc),
        serie: s(serie),
        correlativo: s("123"),
        fecha_emision: lima("2026-11-03T10:00:00-05:00"),
        tipo_moneda: s("PEN"),
        company: Some(company()),
        client: Some(provider()),
        mto_oper_exoneradas: d(10_000, 2),
        mto_oper_inafectas: d(5_000, 2),
        mto_oper_exportacion: exportacion.then_some(Decimal::new(3_000, 2)),
        mto_igv: d(0, 0),
        total_impuestos: d(0, 0),
        mto_imp_venta: d(total, 2),
        legends: Some(vec![Legend {
            code: s("1000"),
            value: s("SON CIENTO OCHENTA CON 00/100 SOLES"),
        }]),
        forma_pago: (tipo_doc == "01").then(|| PaymentTerms {
            moneda: s("PEN"),
            tipo: s("Contado"),
            monto: None,
        }),
        details: Some(details),
        ..BaseSale::default()
    }
}

fn invoice(direccion_entrega: Option<Address>) -> Invoice {
    Invoice {
        base: sale("01", "F001", true),
        tipo_operacion: s("0101"),
        valor_venta: d(18_000, 2),
        sub_total: d(18_000, 2),
        direccion_entrega,
        ..Invoice::default()
    }
}

fn note(tipo_doc: &str, serie: &str) -> Note {
    Note {
        base: sale(tipo_doc, serie, false),
        cod_motivo: s(if tipo_doc == "07" { "01" } else { "02" }),
        des_motivo: s("MOTIVO DEMO"),
        tip_doc_afectado: s("01"),
        num_doc_afectado: s("F001-100"),
        valor_venta: d(15_000, 2),
        sub_total: d(15_000, 2),
        ..Note::default()
    }
}

fn render_perception(doc: &Perception) -> String {
    PerceptionBuilder::new(RenderOptions::default())
        .render(doc)
        .unwrap()
}

fn render_retention(doc: &Retention) -> String {
    RetentionBuilder::new(RenderOptions::default())
        .render(doc)
        .unwrap()
}

fn render_invoice(doc: &Invoice) -> String {
    InvoiceBuilder::new(RenderOptions::default())
        .render(doc)
        .unwrap()
}

fn render_note(doc: &Note) -> String {
    NoteBuilder::new(RenderOptions::default())
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

/// Valida con `xmllint` contra el XSD de la raíz y versión UBL del documento.
/// En UBL 2.1 se añade un nodo a `ExtensionContent` (vacío hasta la firma).
fn assert_valid_xsd(xml: &str, name: &str) {
    let doc = Document::parse(xml).unwrap();
    let root = doc.root_element().tag_name().name().to_owned();
    let version = doc
        .descendants()
        .find(|n| n.has_tag_name((CBC, "UBLVersionID")))
        .and_then(|n| n.text())
        .unwrap()
        .to_owned();
    let schema = xsd_dir()
        .join(&version)
        .join("maindoc")
        .join(format!("UBL-{root}-{version}.xsd"));
    assert!(
        schema.is_file(),
        "faltan los XSD: ejecutar scripts/prepare_ubl_validator_reference.sh"
    );
    let copy = if version == "2.1" {
        xml.replacen(
            "<ext:ExtensionContent/>",
            "<ext:ExtensionContent><cbc:AccountID>1</cbc:AccountID></ext:ExtensionContent>",
            1,
        )
    } else {
        xml.to_owned()
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("reglas-sunat");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.xml"));
    fs::write(&path, copy).unwrap();
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

fn child<'a, 'i>(node: Node<'a, 'i>, ns: &str, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.has_tag_name((ns, name)))
}

fn text<'a>(node: Option<Node<'a, '_>>) -> Option<&'a str> {
    node.and_then(|n| n.text())
}

/// Nombres (`prefijo:local`) de los hijos elemento de la raíz, en orden.
fn root_children(xml: &str) -> Vec<String> {
    let doc = Document::parse(xml).unwrap();
    doc.root_element()
        .children()
        .filter(Node::is_element)
        .map(|n| {
            let prefix = n
                .tag_name()
                .namespace()
                .and_then(|ns| n.lookup_prefix(ns))
                .unwrap_or_default();
            format!("{prefix}:{}", n.tag_name().name())
        })
        .collect()
}

/// `(CityName, CountrySubentity)` de `cac:AgentParty/cac:PostalAddress`.
fn agent_city(xml: &str) -> (String, String) {
    let doc = Document::parse(xml).unwrap();
    let agent = child(doc.root_element(), CAC, "AgentParty").unwrap();
    let postal = child(agent, CAC, "PostalAddress").unwrap();
    (
        text(child(postal, CBC, "CityName")).unwrap().to_owned(),
        text(child(postal, CBC, "CountrySubentity"))
            .unwrap()
            .to_owned(),
    )
}

/// Hijos elemento de `cac:Delivery/cac:DeliveryLocation/cac:Address`.
fn delivery_address_children(xml: &str) -> Vec<String> {
    let doc = Document::parse(xml).unwrap();
    let delivery = child(doc.root_element(), CAC, "Delivery").unwrap();
    let location = child(delivery, CAC, "DeliveryLocation").unwrap();
    child(location, CAC, "Address")
        .unwrap()
        .children()
        .filter(Node::is_element)
        .map(|n| n.tag_name().name().to_owned())
        .collect()
}

/// `cbc:ID` de cada `cac:Payment`, en orden.
fn payment_ids(xml: &str) -> Vec<String> {
    let doc = Document::parse(xml).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name((CAC, "Payment")))
        .map(|p| text(child(p, CBC, "ID")).unwrap_or_default().to_owned())
        .collect()
}

/// `TaxAmount` de los subtotales del `cac:TaxTotal` raíz por código de tributo.
fn zero_tax_amounts(xml: &str) -> Vec<(String, String)> {
    let doc = Document::parse(xml).unwrap();
    let total = child(doc.root_element(), CAC, "TaxTotal").unwrap();
    total
        .children()
        .filter(|n| n.has_tag_name((CAC, "TaxSubtotal")))
        .filter_map(|sub| {
            let scheme = child(child(sub, CAC, "TaxCategory")?, CAC, "TaxScheme")?;
            let id = text(child(scheme, CBC, "ID"))?;
            matches!(id, "9995" | "9997" | "9998").then(|| {
                (
                    id.to_owned(),
                    text(child(sub, CBC, "TaxAmount"))
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
        })
        .collect()
}

#[test]
fn agente_usa_provincia_en_city_name_y_departamento_en_country_subentity() {
    let expected = ("HUAURA".to_owned(), "LIMA".to_owned());
    let xml = render_perception(&perception(None, None));
    assert_eq!(agent_city(&xml), expected);
    assert_valid_xsd(&xml, "percepcion-direccion");
    let xml = render_retention(&retention(None));
    assert_eq!(agent_city(&xml), expected);
    assert_valid_xsd(&xml, "retencion-direccion");
}

#[test]
fn percepcion_con_indicador_excepcional_lo_escribe_tras_issue_time() {
    let xml = render_perception(&perception(Some("01"), None));
    assert!(xml.contains("<sac:ExceptionalIndicator>01</sac:ExceptionalIndicator>"));
    let names = root_children(&xml);
    let position = |name: &str| names.iter().position(|n| n == name).unwrap();
    let indicator = position("sac:ExceptionalIndicator");
    assert_eq!(indicator, position("cbc:IssueTime") + 1);
    assert_eq!(indicator + 1, position("cac:AgentParty"));
    assert_valid_xsd(&xml, "percepcion-excepcional");
}

#[test]
fn percepcion_sin_indicador_no_escribe_el_nodo() {
    // Sin valor, vacío o "0" (falso en la plantilla), el nodo condicional no
    // se escribe.
    for (label, value) in [
        ("sin-valor", None),
        ("vacio", Some("")),
        ("cero", Some("0")),
    ] {
        let xml = render_perception(&perception(value, None));
        assert!(!xml.contains("ExceptionalIndicator"), "{label}");
        assert_valid_xsd(&xml, &format!("percepcion-sin-indicador-{label}"));
    }
}

#[test]
fn indicador_excepcional_se_escapa() {
    let xml = render_perception(&perception(Some("<&>"), None));
    assert!(xml.contains("<sac:ExceptionalIndicator>&lt;&amp;&gt;</sac:ExceptionalIndicator>"));
    let doc = Document::parse(&xml).unwrap();
    let node = child(doc.root_element(), SAC, "ExceptionalIndicator");
    assert_eq!(text(node), Some("<&>"));
}

#[test]
fn retencion_no_tiene_indicador_excepcional() {
    let xml = render_retention(&retention(None));
    assert!(!xml.contains("ExceptionalIndicator"));
}

#[test]
fn numero_de_pago_explicito_o_por_posicion() {
    assert_eq!(
        payment_ids(&render_perception(&perception(None, Some(7)))),
        ["7"]
    );
    assert_eq!(
        payment_ids(&render_perception(&perception(None, None))),
        ["1"]
    );
    assert_eq!(payment_ids(&render_retention(&retention(Some(4)))), ["4"]);
    assert_eq!(payment_ids(&render_retention(&retention(None))), ["1"]);
    let xml = render_retention(&retention(Some(12)));
    assert_valid_xsd(&xml, "retencion-pago-numerado");
}

#[test]
fn factura_tributos_sin_impuesto_con_tax_amount_n12_2() {
    let xml = render_invoice(&invoice(None));
    assert_eq!(
        zero_tax_amounts(&xml),
        [
            ("9998".to_owned(), "0.00".to_owned()),
            ("9997".to_owned(), "0.00".to_owned()),
            ("9995".to_owned(), "0.00".to_owned()),
        ]
    );
    assert_valid_xsd(&xml, "factura-tributos-cero");
}

#[test]
fn notas_tributos_sin_impuesto_con_tax_amount_n12_2() {
    for (tipo_doc, serie, name) in [
        ("07", "FC01", "nota-credito"),
        ("08", "FD01", "nota-debito"),
    ] {
        let xml = render_note(&note(tipo_doc, serie));
        assert_eq!(
            zero_tax_amounts(&xml),
            [
                ("9998".to_owned(), "0.00".to_owned()),
                ("9997".to_owned(), "0.00".to_owned()),
            ],
            "{name}"
        );
        assert_valid_xsd(&xml, name);
    }
}

#[test]
fn direccion_de_entrega_solo_con_pais() {
    // Exportación de servicios: el país del uso del servicio sin la línea de
    // dirección de la venta itinerante.
    let entrega = Address {
        codigo_pais: s("US"),
        ..Address::default()
    };
    let xml = render_invoice(&invoice(Some(entrega)));
    let doc = Document::parse(&xml).unwrap();
    let delivery = child(doc.root_element(), CAC, "Delivery").unwrap();
    let address = child(
        child(delivery, CAC, "DeliveryLocation").unwrap(),
        CAC,
        "Address",
    )
    .unwrap();
    let names: Vec<&str> = address
        .children()
        .filter(Node::is_element)
        .map(|n| n.tag_name().name())
        .collect();
    assert_eq!(names, ["Country"]);
    let country = child(address, CAC, "Country").unwrap();
    assert_eq!(text(child(country, CBC, "IdentificationCode")), Some("US"));
    assert_valid_xsd(&xml, "factura-entrega-solo-pais");
}

#[test]
fn direccion_de_entrega_completa_se_escribe_entera() {
    let entrega = Address {
        cod_local: None,
        ..address("LIMA", "LIMA")
    };
    let xml = render_invoice(&invoice(Some(entrega)));
    let doc = Document::parse(&xml).unwrap();
    let delivery = child(doc.root_element(), CAC, "Delivery").unwrap();
    let address = child(
        child(delivery, CAC, "DeliveryLocation").unwrap(),
        CAC,
        "Address",
    )
    .unwrap();
    let names: Vec<&str> = address
        .children()
        .filter(Node::is_element)
        .map(|n| n.tag_name().name())
        .collect();
    assert_eq!(
        names,
        [
            "ID",
            "CitySubdivisionName",
            "CityName",
            "CountrySubentity",
            "District",
            "AddressLine",
            "Country"
        ]
    );
    assert!(xml.contains("<cbc:ID>150801</cbc:ID>"));
    assert!(xml.contains("<cbc:Line><![CDATA[AV. DEMO 123]]></cbc:Line>"));
    assert_valid_xsd(&xml, "factura-entrega-completa");
}

#[test]
fn direccion_de_entrega_omite_partes_vacias() {
    let entrega = Address {
        ubigueo: s(""),
        provincia: s("LIMA"),
        departamento: s(""),
        distrito: None,
        direccion: s(""),
        codigo_pais: s("PE"),
        ..Address::default()
    };
    let xml = render_invoice(&invoice(Some(entrega)));
    assert_eq!(delivery_address_children(&xml), ["CityName", "Country"]);
    assert_valid_xsd(&xml, "factura-entrega-partes-vacias");
}
