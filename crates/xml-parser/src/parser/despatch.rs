//! Guía de remisión (`DespatchParser`).

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::despatch::{Despatch, DespatchDetail, Direction, Shipment, Transportist};
use cpeforge_core::model::sale::RelatedDocument;

use crate::convert::{DateContext, explode_part, loose_eq, to_float, to_int};
use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::{Lookup, node_attribute, node_value};

super::parser!(DespatchParser, Despatch, "Despatch");

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<Despatch, XmlParserError> {
    let root = Some(xml.root());
    let id = xml.value("cbc:ID", root)?;
    let tipo_doc = xml.value("cbc:DespatchAdviceTypeCode", root)?;
    let observacion = xml.value("cbc:Note", root)?;
    let fecha_emision = dates.parse(&xml.value("cbc:IssueDate", root)?)?;
    let company_node = xml.node("cac:DespatchSupplierParty", root)?;
    let company = Company {
        ruc: Some(xml.value("cbc:CustomerAssignedAccountID", company_node)?),
        razon_social: Some(xml.value(
            "cac:Party/cac:PartyLegalEntity/cbc:RegistrationName",
            company_node,
        )?),
        ..Company::default()
    };
    let destinatario = client(xml, "cac:DeliveryCustomerParty")?;
    let tercero = client(xml, "cac:SellerSupplierParty")?;
    let envio = shipment(xml, dates)?;
    let mut details = Vec::new();
    for node in xml.nodes("cac:DespatchLine", root)? {
        let at = Some(node);
        let quantity = xml.node("cbc:DeliveredQuantity", at)?;
        details.push(DespatchDetail {
            cantidad: Some(to_float(&node_value(quantity).unwrap_or_default())?),
            unidad: Some(node_attribute(quantity, "unitCode")?),
            descripcion: Some(xml.value("cac:Item/cbc:Name", at)?),
            codigo: Some(xml.value("cac:Item/cac:SellersItemIdentification/cbc:ID", at)?),
            cod_prod_sunat: Some(xml.value(
                "cac:Item/cac:CommodityClassification/cbc:ItemClassificationCode",
                at,
            )?),
            ..DespatchDetail::default()
        });
    }
    let mut despatch = Despatch {
        serie: explode_part(&id, 0),
        correlativo: explode_part(&id, 1),
        tipo_doc: Some(tipo_doc),
        observacion: Some(observacion),
        fecha_emision: Some(fecha_emision),
        company: Some(company),
        destinatario,
        tercero,
        envio: Some(envio),
        details: Some(details),
        ..Despatch::default()
    };
    // loadRelDocs()
    if let Some(baja) = xml.node("cac:OrderReference", root)? {
        despatch.doc_baja = Some(related(xml, baja)?);
    }
    if let Some(rel) = xml.node("cac:AdditionalDocumentReference", root)? {
        despatch.rel_doc = Some(related(xml, rel)?);
    }
    Ok(despatch)
}

fn related<'a>(xml: &Lookup<'a>, node: Node<'a>) -> Result<RelatedDocument, XmlParserError> {
    Ok(RelatedDocument {
        tipo_doc: Some(xml.value("cbc:OrderTypeCode", Some(node))?),
        nro_doc: Some(xml.value("cbc:ID", Some(node))?),
    })
}

fn client(xml: &Lookup<'_>, query: &str) -> Result<Option<Client>, XmlParserError> {
    let node = xml.node(query, Some(xml.root()))?;
    let Some(doc) = xml.node("cbc:CustomerAssignedAccountID", node)? else {
        return Ok(None);
    };
    Ok(Some(Client {
        num_doc: Some(doc.value()),
        tipo_doc: Some(doc.attribute("schemeID")),
        rzn_social: Some(xml.value("cac:Party/cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        ..Client::default()
    }))
}

fn direction<'a>(xml: &Lookup<'a>, node: Node<'a>) -> Result<Direction, XmlParserError> {
    Ok(Direction {
        ubigueo: Some(xml.value("cbc:ID", Some(node))?),
        direccion: Some(xml.value("cbc:StreetName", Some(node))?),
        ..Direction::default()
    })
}

fn shipment(xml: &Lookup<'_>, dates: &DateContext) -> Result<Shipment, XmlParserError> {
    let node = xml.node("cac:Shipment", Some(xml.root()))?;
    let mut shp = Shipment {
        cod_traslado: Some(xml.value("cbc:HandlingCode", node)?),
        des_traslado: Some(xml.value("cbc:Information", node)?),
        num_bultos: Some(to_int(&xml.value_def(
            "cbc:TotalTransportHandlingUnitQuantity",
            node,
            "0",
        )?)?),
        ind_transbordo: Some(loose_eq(
            &xml.value_def("cbc:SplitConsignmentIndicator", node, "false")?,
            "true",
        )),
        num_contenedor: Some(xml.value("cac:TransportHandlingUnit/cbc:ID", node)?),
        cod_puerto: Some(xml.value("cac:FirstArrivalPortLocation/cbc:ID", node)?),
        ..Shipment::default()
    };
    if let Some(weight) = xml.node("cbc:GrossWeightMeasure", node)? {
        shp.und_peso_total = Some(weight.attribute("unitCode"));
        shp.peso_total = Some(to_float(&weight.value())?);
    }
    if let Some(llegada) = xml.node("cac:Delivery/cac:DeliveryAddress", node)? {
        shp.llegada = Some(direction(xml, llegada)?);
    }
    if let Some(partida) = xml.node("cac:OriginAddress", node)? {
        shp.partida = Some(direction(xml, partida)?);
    }
    let stage = xml.node("cac:ShipmentStage", node)?;
    shp.mod_traslado = Some(xml.value("cbc:TransportModeCode", stage)?);
    shp.fec_traslado = Some(dates.parse(&xml.value("cac:TransitPeriod/cbc:StartDate", stage)?)?);
    shp.transportista = Some(transportist(xml, stage)?);
    Ok(shp)
}

fn transportist<'a>(
    xml: &Lookup<'a>,
    node: Option<Node<'a>>,
) -> Result<Transportist, XmlParserError> {
    let mut trans = Transportist::default();
    if let Some(carrier) = xml.node("cac:CarrierParty", node)? {
        let id = xml.node("cac:PartyIdentification/cbc:ID", Some(carrier))?;
        trans.tipo_doc = Some(node_attribute(id, "schemeID")?);
        trans.num_doc = node_value(id);
        trans.rzn_social = Some(xml.value("cac:PartyName/cbc:Name", Some(carrier))?);
    }
    trans.placa = Some(xml.value(
        "cac:TransportMeans/cac:RoadTransport/cbc:LicensePlateID",
        node,
    )?);
    if let Some(driver) = xml.node("cac:DriverPerson/cbc:ID", node)? {
        trans.chofer_tipo_doc = Some(driver.attribute("schemeID"));
        trans.chofer_doc = Some(driver.value());
    }
    Ok(trans)
}
