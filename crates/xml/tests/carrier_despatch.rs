//! Guía de remisión transportista (`31`): estructura del anexo N.° 14 de la
//! R. S. 123-2022/SUNAT con las rutas de la R. S. 108-2026/SUNAT, validada con
//! `xmllint` contra el XSD UBL 2.1 de `DespatchAdvice`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::{
    AdditionalDoc, Despatch, DespatchDetail, Direction, Driver, Shipment, Transportist, Vehicle,
};
use cpeforge_core::{CoreError, Decimal};
use cpeforge_xml::{DespatchBuilder, RenderOptions, XmlError};
use roxmltree::{Document, Node};

const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";

fn s(value: &str) -> Option<String> {
    Some(value.to_owned())
}

fn party(tipo_doc: &str, num_doc: &str, nombre: &str) -> Option<Client> {
    Some(Client {
        tipo_doc: s(tipo_doc),
        num_doc: s(num_doc),
        rzn_social: s(nombre),
        ..Client::default()
    })
}

fn driver(tipo: &str, dni: &str, licencia: &str) -> Driver {
    Driver {
        tipo: s(tipo),
        tipo_doc: s("1"),
        nro_doc: s(dni),
        nombres: s("JUAN"),
        apellidos: s("PEREZ"),
        licencia: s(licencia),
    }
}

/// Guía transportista con todos los grupos opcionales.
fn full() -> Despatch {
    Despatch {
        version: s("2022"),
        tipo_doc: s("31"),
        serie: s("V001"),
        correlativo: s("1"),
        observacion: s("Carga frágil"),
        fecha_emision: Some("2026-10-20T09:00:00-05:00[America/Lima]".parse().unwrap()),
        company: Some(Company {
            ruc: s("20161515648"),
            razon_social: s("TRANSPORTES DEMO S.A.C."),
            address: Some(Address::default()),
            ..Company::default()
        }),
        remitente: party("6", "20000000002", "REMITENTE DEMO S.A.C."),
        destinatario: party("6", "20000000003", "DESTINO DEMO S.A.C."),
        pagador_flete: party("6", "20000000004", "PAGADOR DEMO S.A.C."),
        add_docs: Some(vec![AdditionalDoc {
            tipo_desc: s("Guía de Remisión Remitente"),
            tipo: s("09"),
            nro: s("T001-121"),
            emisor: s("20000000002"),
        }]),
        envio: Some(Shipment {
            fec_traslado: Some("2026-10-20T11:00:00-05:00[America/Lima]".parse().unwrap()),
            peso_total: Some(Decimal::new(125, 1)),
            und_peso_total: s("KGM"),
            indicadores: Some(vec![
                "SUNAT_Envio_IndicadorTransbordoProgramado".to_owned(),
                "SUNAT_Envio_IndicadorPagadorFlete_Tercero".to_owned(),
            ]),
            transportista: Some(Transportist {
                nro_mtc: s("0001"),
                ..Transportist::default()
            }),
            subcontratador: party("6", "20000000005", "SUBCONTRATA DEMO S.A.C."),
            vehiculo: Some(Vehicle {
                placa: s("ABC123"),
                nro_circulacion: s("15M21028250E"),
                secundarios: Some(vec![Vehicle {
                    placa: s("DEF456"),
                    nro_circulacion: s("15M21028251E"),
                    ..Vehicle::default()
                }]),
                ..Vehicle::default()
            }),
            choferes: Some(vec![
                driver("Principal", "44556677", "Q44556677"),
                driver("Secundario", "77665544", "Q77665544"),
            ]),
            llegada: Some(Direction::new(s("150101"), s("AV. DESTINO 456"))),
            partida: Some(Direction::new(s("150203"), s("AV. ORIGEN 123"))),
            ..Shipment::default()
        }),
        details: Some(vec![DespatchDetail {
            codigo: s("PROD1"),
            descripcion: s("MERCADERIA DEMO"),
            unidad: s("NIU"),
            cantidad: Some(Decimal::new(2, 0)),
            ..DespatchDetail::default()
        }]),
        ..Despatch::default()
    }
}

/// Solo lo obligatorio de la guía transportista.
fn minimal() -> Despatch {
    let mut doc = full();
    doc.observacion = None;
    doc.pagador_flete = None;
    doc.add_docs = None;
    if let Some(envio) = doc.envio.as_mut() {
        envio.indicadores = None;
        envio.subcontratador = None;
        envio.transportista = None;
        envio.llegada = None;
        envio.choferes = Some(vec![driver("Principal", "44556677", "Q44556677")]);
        if let Some(vehiculo) = envio.vehiculo.as_mut() {
            vehiculo.secundarios = None;
            vehiculo.nro_circulacion = None;
        }
    }
    doc
}

fn render(doc: &Despatch) -> Result<String, XmlError> {
    DespatchBuilder::new(RenderOptions::default()).render(doc)
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
    let schema = xsd_dir().join("2.1/maindoc/UBL-DespatchAdvice-2.1.xsd");
    assert!(
        schema.is_file(),
        "faltan los XSD: ejecutar scripts/prepare_ubl_validator_reference.sh"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("carrier-despatch");
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
        .expect("xmllint");
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Nodo al final de una ruta de nombres locales desde la raíz.
fn at<'a, 'i>(doc: &'a Document<'i>, path: &[&str]) -> Option<Node<'a, 'i>> {
    let mut node = doc.root_element();
    for name in path {
        node = node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == *name)?;
    }
    Some(node)
}

fn text<'a>(doc: &'a Document<'a>, path: &[&str]) -> Option<&'a str> {
    at(doc, path).and_then(|n| n.text())
}

#[test]
fn full_carrier_despatch_follows_sunat_structure() {
    let xml = render(&full()).unwrap();
    assert_valid_xsd(&xml, "full");
    let doc = Document::parse(&xml).unwrap();

    // Campos 6, 8 y 9: tipo 31 y transportista emisor.
    assert_eq!(text(&doc, &["DespatchAdviceTypeCode"]), Some("31"));
    assert_eq!(
        text(
            &doc,
            &[
                "DespatchSupplierParty",
                "Party",
                "PartyIdentification",
                "ID"
            ]
        ),
        Some("20161515648")
    );
    // Campos 12 a 15: GRE remitente relacionada.
    assert_eq!(
        text(&doc, &["AdditionalDocumentReference", "DocumentTypeCode"]),
        Some("09")
    );
    assert_eq!(
        text(&doc, &["AdditionalDocumentReference", "ID"]),
        Some("T001-121")
    );
    // Campos 16 y 17: remitente en Shipment/Delivery/Despatch/DespatchParty.
    let remitente = ["Shipment", "Delivery", "Despatch", "DespatchParty"];
    let id = at(
        &doc,
        &[&remitente[..], &["PartyIdentification", "ID"]].concat(),
    )
    .unwrap();
    assert_eq!(id.text(), Some("20000000002"));
    assert_eq!(id.attribute("schemeID"), Some("6"));
    assert_eq!(
        text(
            &doc,
            &[&remitente[..], &["PartyLegalEntity", "RegistrationName"]].concat()
        ),
        Some("REMITENTE DEMO S.A.C.")
    );
    // Campos 18 y 19: destinatario.
    assert_eq!(
        text(
            &doc,
            &[
                "DeliveryCustomerParty",
                "Party",
                "PartyIdentification",
                "ID"
            ]
        ),
        Some("20000000003")
    );
    // Campos 62 y 63: tercero que paga el flete.
    assert_eq!(
        text(
            &doc,
            &[
                "OriginatorCustomerParty",
                "Party",
                "PartyIdentification",
                "ID"
            ]
        ),
        Some("20000000004")
    );
    // Campo 10: registro MTC del transportista, sin identificación repetida.
    let carrier = at(&doc, &["Shipment", "ShipmentStage", "CarrierParty"]).unwrap();
    assert_eq!(
        text(
            &doc,
            &[
                "Shipment",
                "ShipmentStage",
                "CarrierParty",
                "PartyLegalEntity",
                "CompanyID"
            ]
        ),
        Some("0001")
    );
    assert!(
        carrier
            .children()
            .all(|n| n.tag_name().name() != "PartyIdentification")
    );
    // Campos 60 y 61: subcontratista.
    assert_eq!(
        text(&doc, &["Shipment", "Consignment", "ID"]),
        Some("SUNAT_Envio")
    );
    assert_eq!(
        text(
            &doc,
            &[
                "Shipment",
                "Consignment",
                "LogisticsOperatorParty",
                "PartyIdentification",
                "ID"
            ]
        ),
        Some("20000000005")
    );
    // Campos 35 a 39: vehículos con TUC.
    assert_eq!(
        text(
            &doc,
            &[
                "Shipment",
                "TransportHandlingUnit",
                "TransportEquipment",
                "ApplicableTransportMeans",
                "RegistrationNationalityID"
            ]
        ),
        Some("15M21028250E")
    );
    // Campos 41 a 48: conductores.
    let drivers: Vec<_> = at(&doc, &["Shipment", "ShipmentStage"])
        .unwrap()
        .children()
        .filter(|n| n.tag_name().name() == "DriverPerson")
        .collect();
    assert_eq!(drivers.len(), 2);
    // Campos 49, 50, 52 y 56: traslado, peso e indicadores.
    assert_eq!(text(&doc, &["Shipment", "ID"]), Some("SUNAT_Envio"));
    assert_eq!(
        text(
            &doc,
            &["Shipment", "ShipmentStage", "TransitPeriod", "StartDate"]
        ),
        Some("2026-10-20")
    );
    assert_eq!(
        text(&doc, &["Shipment", "GrossWeightMeasure"]),
        Some("12.500")
    );
    // La guía transportista no lleva motivo ni modalidad de traslado.
    let shipment = at(&doc, &["Shipment"]).unwrap();
    for absent in ["HandlingCode", "HandlingInstructions"] {
        assert!(
            shipment.children().all(|n| n.tag_name().name() != absent),
            "{absent}"
        );
    }
    assert!(at(&doc, &["Shipment", "ShipmentStage", "TransportModeCode"]).is_none());
    assert!(
        doc.descendants()
            .any(|n| n.tag_name().namespace() == Some(CBC))
    );
}

#[test]
fn minimal_carrier_despatch_is_valid() {
    let xml = render(&minimal()).unwrap();
    assert_valid_xsd(&xml, "minimal");
    let doc = Document::parse(&xml).unwrap();
    assert!(at(&doc, &["OriginatorCustomerParty"]).is_none());
    assert!(at(&doc, &["Shipment", "Consignment"]).is_none());
    assert!(at(&doc, &["Shipment", "ShipmentStage", "CarrierParty"]).is_none());
}

#[test]
fn carrier_despatch_requires_the_sender() {
    let mut doc = minimal();
    doc.remitente = None;
    match render(&doc) {
        Err(XmlError::Document {
            source: CoreError::MissingField { model, field },
            ..
        }) => assert_eq!((model, field), ("Despatch", "remitente")),
        other => panic!("{other:?}"),
    }
}
