//! Liquidación de compra (`04`): estructura del anexo N.º 9-A, literal e), de
//! la R. S. 123-2022/SUNAT, validada con `xmllint` contra el XSD UBL 2.1 de
//! `SelfBilledInvoice`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{DetailAttribute, Legend, RelatedDocument};
use cpeforge_core::model::settlement::{
    PurchaseSettlement, SettlementDetail, SettlementPlace, SettlementSeller,
};
use cpeforge_core::{CoreError, Decimal};
use cpeforge_xml::{RenderOptions, SettlementBuilder, XmlError};
use roxmltree::{Document, Node};

const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const CAC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2";
const ROOT_NS: &str = "urn:oasis:names:specification:ubl:schema:xsd:SelfBilledInvoice-2";

fn s(value: &str) -> Option<String> {
    Some(value.to_owned())
}

fn d(value: i64, scale: u32) -> Option<Decimal> {
    Some(Decimal::new(value, scale))
}

fn address(ubigeo: &str, direccion: &str) -> Address {
    Address {
        ubigueo: s(ubigeo),
        departamento: s("LIMA"),
        provincia: s("LIMA"),
        distrito: s("LIMA"),
        direccion: s(direccion),
        ..Address::default()
    }
}

/// Línea gravada: 200 KGM a 3.50 (IGV 18 %, renta 1.5 %).
fn taxed_line() -> SettlementDetail {
    SettlementDetail {
        unidad: s("KGM"),
        cantidad: d(200, 0),
        cod_producto: s("CAF01"),
        cod_prod_sunat: s("10101501"),
        descripcion: s("Café pergamino"),
        mto_valor_unitario: d(350, 2),
        mto_precio_unitario: d(413, 2),
        mto_valor_venta: d(70000, 2),
        tip_afe_igv: s("10"),
        mto_base_igv: d(70000, 2),
        porcentaje_igv: d(18, 0),
        igv: d(12600, 2),
        mto_base_ir: d(70000, 2),
        porcentaje_ir: d(15, 1),
        ir: d(1050, 2),
        total_impuestos: d(12600, 2),
        ..SettlementDetail::default()
    }
}

/// Línea exonerada: 10 NIU a 20.00.
fn exempt_line() -> SettlementDetail {
    SettlementDetail {
        unidad: s("NIU"),
        cantidad: d(10, 0),
        cod_prod_sunat: s("10101501"),
        descripcion: s("Sacos de yute"),
        mto_valor_unitario: d(2000, 2),
        mto_precio_unitario: d(2000, 2),
        mto_valor_venta: d(20000, 2),
        tip_afe_igv: s("20"),
        mto_base_igv: d(20000, 2),
        porcentaje_igv: d(0, 0),
        igv: d(0, 2),
        mto_base_ir: d(20000, 2),
        porcentaje_ir: d(15, 1),
        ir: d(300, 2),
        total_impuestos: d(0, 2),
        ..SettlementDetail::default()
    }
}

/// Liquidación con todos los grupos: dos líneas, guía y documento relacionados.
fn full() -> PurchaseSettlement {
    PurchaseSettlement {
        tipo_operacion: s("0501"),
        serie: s("L001"),
        correlativo: s("1"),
        fecha_emision: Some("2026-10-20T09:30:00-05:00[America/Lima]".parse().unwrap()),
        tipo_moneda: s("PEN"),
        company: Some(Company {
            ruc: s("20000000001"),
            razon_social: s("AGROINDUSTRIAS DEMO S.A.C."),
            nombre_comercial: s("AGRO DEMO"),
            address: Some(address("150101", "AV. DEMO 123")),
            ..Company::default()
        }),
        vendedor: Some(SettlementSeller {
            tipo_doc: s("1"),
            num_doc: s("40123456"),
            rzn_social: s("QUISPE MAMANI JUAN"),
            address: Some(address("040101", "CHACRA LOS ÁLAMOS S/N")),
            condicion: s("02"),
        }),
        lugar_operacion: Some(SettlementPlace {
            address: Some(address("150101", "ALMACÉN CENTRAL")),
            condicion: s("01"),
        }),
        guias: Some(vec![RelatedDocument {
            tipo_doc: s("09"),
            nro_doc: s("T001-15"),
        }]),
        rel_docs: Some(vec![RelatedDocument {
            tipo_doc: s("99"),
            nro_doc: s("OC-2026-77"),
        }]),
        mto_oper_gravadas: d(70000, 2),
        mto_oper_exoneradas: d(20000, 2),
        mto_igv: d(12600, 2),
        mto_base_ir: d(90000, 2),
        mto_ir: d(1350, 2),
        total_impuestos: d(12600, 2),
        valor_venta: d(90000, 2),
        sub_total: d(102600, 2),
        mto_imp_venta: d(88650, 2),
        legends: Some(vec![Legend {
            code: s("1000"),
            value: s("SON OCHOCIENTOS OCHENTA Y SEIS CON 50/100 SOLES"),
        }]),
        details: Some(vec![taxed_line(), exempt_line()]),
        ..PurchaseSettlement::default()
    }
}

fn render(doc: &PurchaseSettlement) -> Result<String, XmlError> {
    SettlementBuilder::new(RenderOptions::default()).render(doc)
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
    let schema = xsd_dir().join("2.1/maindoc/UBL-SelfBilledInvoice-2.1.xsd");
    assert!(
        schema.is_file(),
        "faltan los XSD: ejecutar scripts/prepare_ubl_validator_reference.sh"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("settlement");
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

#[test]
fn full_settlement_is_valid_and_places_each_party() {
    let xml = render(&full()).unwrap();
    assert_valid_xsd(&xml, "full");
    let doc = Document::parse(&xml).unwrap();
    let root = doc.root_element();
    assert!(root.has_tag_name((ROOT_NS, "SelfBilledInvoice")));
    let type_code = child(root, CBC, "InvoiceTypeCode");
    assert_eq!(text(type_code), "04");
    assert_eq!(type_code.attribute("listID"), Some("0501"));
    assert_eq!(text(child(root, CBC, "ID")), "L001-1");

    // Emisor (comprador): RUC en AccountingCustomerParty.
    let buyer = path(root, &[(CAC, "AccountingCustomerParty"), (CAC, "Party")]);
    let ruc = path(buyer, &[(CAC, "PartyIdentification"), (CBC, "ID")]);
    assert_eq!(
        (text(ruc), ruc.attribute("schemeID")),
        ("20000000001", Some("6"))
    );
    let buyer_address = path(
        buyer,
        &[(CAC, "PartyLegalEntity"), (CAC, "RegistrationAddress")],
    );
    assert_eq!(text(child(buyer_address, CBC, "AddressTypeCode")), "0000");

    // Vendedor: DNI y condición del domicilio (catálogo 60).
    let seller = path(root, &[(CAC, "AccountingSupplierParty"), (CAC, "Party")]);
    let dni = path(seller, &[(CAC, "PartyIdentification"), (CBC, "ID")]);
    assert_eq!(
        (text(dni), dni.attribute("schemeID")),
        ("40123456", Some("1"))
    );
    let seller_address = path(
        seller,
        &[(CAC, "PartyLegalEntity"), (CAC, "RegistrationAddress")],
    );
    assert_eq!(text(child(seller_address, CBC, "AddressTypeCode")), "02");
    assert_eq!(text(child(seller_address, CBC, "ID")), "040101");

    // Lugar de la operación.
    let place = path(root, &[(CAC, "DeliveryTerms"), (CAC, "DeliveryLocation")]);
    assert_eq!(text(child(place, CBC, "LocationTypeCode")), "01");
    assert_eq!(
        text(path(place, &[(CAC, "Address"), (CBC, "ID")])),
        "150101"
    );

    // Totales: el TaxAmount es el IGV; la renta va en su propio subtotal.
    let tax_total = child(root, CAC, "TaxTotal");
    assert_eq!(text(child(tax_total, CBC, "TaxAmount")), "126.00");
    let schemes: Vec<(&str, &str)> = tax_total
        .children()
        .filter(|n| n.has_tag_name((CAC, "TaxSubtotal")))
        .map(|sub| {
            let id = path(
                sub,
                &[(CAC, "TaxCategory"), (CAC, "TaxScheme"), (CBC, "ID")],
            );
            (text(id), text(child(sub, CBC, "TaxAmount")))
        })
        .collect();
    assert_eq!(
        schemes,
        [("1000", "126.00"), ("9997", "0"), ("3000", "13.50")]
    );
    let totals = child(root, CAC, "LegalMonetaryTotal");
    assert_eq!(text(child(totals, CBC, "TaxInclusiveAmount")), "1026.00");
    assert_eq!(text(child(totals, CBC, "PayableAmount")), "886.50");

    // Línea: IGV y renta retenida con su tasa.
    let line = child(root, CAC, "InvoiceLine");
    let subtotals: Vec<Node<'_, '_>> = child(line, CAC, "TaxTotal")
        .children()
        .filter(|n| n.has_tag_name((CAC, "TaxSubtotal")))
        .collect();
    let ir = subtotals[1];
    assert_eq!(text(child(ir, CBC, "TaxAmount")), "10.50");
    assert_eq!(
        text(path(ir, &[(CAC, "TaxCategory"), (CBC, "Percent")])),
        "1.5"
    );
    let scheme = path(ir, &[(CAC, "TaxCategory"), (CAC, "TaxScheme")]);
    assert_eq!(
        (
            text(child(scheme, CBC, "ID")),
            text(child(scheme, CBC, "Name")),
            text(child(scheme, CBC, "TaxTypeCode"))
        ),
        ("3000", "IR", "TOX")
    );
    let refs = child(root, CAC, "DespatchDocumentReference");
    assert_eq!(text(child(refs, CBC, "ID")), "T001-15");
}

#[test]
fn gold_purchase_carries_its_item_properties() {
    let mut doc = full();
    doc.tipo_operacion = s("0503");
    doc.guias = None;
    doc.rel_docs = None;
    let mut line = taxed_line();
    line.porcentaje_ir = d(4, 0);
    line.ir = d(2800, 2);
    line.atributos = Some(
        [
            ("6000", "Código único de concesión minera", "010012718"),
            ("6004", "Ley mineral", "85.50"),
            ("6005", "Naturaleza del mineral", "1"),
            ("6006", "Nombre del derecho minero", "MINA DEMO"),
        ]
        .into_iter()
        .map(|(code, name, value)| DetailAttribute {
            code: s(code),
            name: s(name),
            value: s(value),
            ..DetailAttribute::default()
        })
        .collect(),
    );
    doc.details = Some(vec![line]);
    doc.mto_oper_exoneradas = None;
    let xml = render(&doc).unwrap();
    assert_valid_xsd(&xml, "gold");
    let parsed = Document::parse(&xml).unwrap();
    let item = path(
        parsed.root_element(),
        &[(CAC, "InvoiceLine"), (CAC, "Item")],
    );
    let codes: Vec<&str> = item
        .children()
        .filter(|n| n.has_tag_name((CAC, "AdditionalItemProperty")))
        .map(|p| text(child(p, CBC, "NameCode")))
        .collect();
    assert_eq!(codes, ["6000", "6004", "6005", "6006"]);
}

#[test]
fn missing_parties_are_errors_not_invalid_xml() {
    for (field, clear) in [
        (
            "company",
            (|d: &mut PurchaseSettlement| d.company = None) as fn(&mut _),
        ),
        ("vendedor", |d| d.vendedor = None),
        ("lugarOperacion", |d| d.lugar_operacion = None),
    ] {
        let mut doc = full();
        clear(&mut doc);
        match render(&doc) {
            Err(XmlError::Document { source, .. }) => assert_eq!(
                source,
                CoreError::MissingField {
                    model: "PurchaseSettlement",
                    field
                }
            ),
            other => panic!("{field}: {other:?}"),
        }
    }
}

#[test]
fn text_cannot_break_out_of_cdata() {
    let mut doc = full();
    if let Some(seller) = doc.vendedor.as_mut() {
        seller.rzn_social = s("ROJAS ]]><x/> PEDRO");
    }
    let xml = render(&doc).unwrap();
    let parsed = Document::parse(&xml).unwrap();
    let name = path(
        parsed.root_element(),
        &[
            (CAC, "AccountingSupplierParty"),
            (CAC, "Party"),
            (CAC, "PartyLegalEntity"),
            (CBC, "RegistrationName"),
        ],
    );
    assert_eq!(text(name), "ROJAS ]]><x/> PEDRO");
}
