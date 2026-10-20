//! `despatch.xml.twig` y `despatch2022.xml.twig`.

use cpeforge_core::CoreError;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{Despatch, Direction, Puerto, Vehicle};

use super::shared::{self, EMPTY_EXTENSIONS, XML_DECLARATION};
use crate::options::RenderOptions;
use crate::render::{Ctx, Out, Result, truthy_str};

pub(crate) const TEMPLATE: &str = "despatch.xml.twig";
pub(crate) const TEMPLATE_2022: &str = "despatch2022.xml.twig";

const ROOT: &str = r#"<DespatchAdvice xmlns="urn:oasis:names:specification:ubl:schema:xsd:DespatchAdvice-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2">"#;

/// Tipo de documento de la guía de remisión transportista (catálogo 01).
const CARRIER_TYPE: &str = "31";
const IDENTITY_SCHEME: &str = r#"" schemeName="Documento de Identidad" schemeAgencyName="PE:SUNAT" schemeURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:catalogo06">"#;

fn header(out: &mut Out<'_>, doc: &Despatch, customization: &str) -> Result<()> {
    out.raw(XML_DECLARATION)
        .raw(ROOT)
        .raw(EMPTY_EXTENSIONS)
        .raw("<cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:CustomizationID>")
        .raw(customization)
        .raw("</cbc:CustomizationID><cbc:ID>")
        .text(doc.serie.as_deref())
        .raw("-")
        .text(doc.correlativo.as_deref())
        .raw("</cbc:ID><cbc:IssueDate>")
        .date(doc.fecha_emision.as_ref())?
        .raw("</cbc:IssueDate><cbc:IssueTime>")
        .time(doc.fecha_emision.as_ref())?
        .raw("</cbc:IssueTime>");
    Ok(())
}

fn observation(out: &mut Out<'_>, doc: &Despatch) {
    if truthy_str(doc.observacion.as_deref()) {
        out.raw("<cbc:Note><![CDATA[")
            .cdata(doc.observacion.as_deref())
            .raw("]]></cbc:Note>");
    }
}

/// Guía con la plantilla anterior a 2022.
pub(crate) fn render(doc: &Despatch, options: &RenderOptions) -> Result<String> {
    let mut out = Out::new(options, TEMPLATE);
    let c = out.ctx();
    header(&mut out, doc, "1.0")?;
    out.raw("<cbc:DespatchAdviceTypeCode>")
        .text(doc.tipo_doc.as_deref())
        .raw("</cbc:DespatchAdviceTypeCode>");
    observation(&mut out, doc);
    if let Some(baja) = doc.doc_baja.as_ref() {
        out.raw("<cac:OrderReference><cbc:ID>")
            .text(baja.nro_doc.as_deref())
            .raw(r#"</cbc:ID><cbc:OrderTypeCode listAgencyName="PE:SUNAT" listName="SUNAT:Identificador de Tipo de Documento" listURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:catalogo01">"#)
            .text(baja.tipo_doc.as_deref())
            .raw("</cbc:OrderTypeCode></cac:OrderReference>");
    }
    if let Some(rel) = doc.rel_doc.as_ref() {
        out.raw("<cac:AdditionalDocumentReference><cbc:ID>")
            .text(rel.nro_doc.as_deref())
            .raw(r#"</cbc:ID><cbc:DocumentTypeCode listAgencyName="PE:SUNAT" listName="SUNAT:Identificador de documento relacionado" listURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:catalogo21">"#)
            .text(rel.tipo_doc.as_deref())
            .raw("</cbc:DocumentTypeCode></cac:AdditionalDocumentReference>");
    }
    let emp = doc.company.as_ref();
    shared::signature(&mut out, emp)?;
    out.raw("<cac:DespatchSupplierParty><cbc:CustomerAssignedAccountID schemeID=\"6\">")
        .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
        .raw("</cbc:CustomerAssignedAccountID><cac:Party><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(emp, "razonSocial", |e| e.razon_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:DespatchSupplierParty>");
    legacy_party(
        &mut out,
        c,
        "DeliveryCustomerParty",
        doc.destinatario.as_ref(),
    )?;
    if doc.tercero.is_some() {
        legacy_party(&mut out, c, "SellerSupplierParty", doc.tercero.as_ref())?;
    }
    let envio = doc.envio.as_ref();
    out.raw("<cac:Shipment><cbc:ID>1</cbc:ID><cbc:HandlingCode>")
        .text(c.get(envio, "codTraslado", |e| e.cod_traslado.as_deref())?)
        .raw("</cbc:HandlingCode>");
    let des_traslado = c.get(envio, "desTraslado", |e| e.des_traslado.as_deref())?;
    if truthy_str(des_traslado) {
        out.raw("<cbc:Information>")
            .text(des_traslado)
            .raw("</cbc:Information>");
    }
    out.raw("<cbc:GrossWeightMeasure unitCode=\"")
        .text(c.get(envio, "undPesoTotal", |e| e.und_peso_total.as_deref())?)
        .raw("\">")
        .fmt(c.get(envio, "pesoTotal", |e| e.peso_total)?, 3)
        .raw("</cbc:GrossWeightMeasure>");
    let bultos = c.get(envio, "numBultos", |e| e.num_bultos)?;
    if bultos.is_some_and(|n| n != 0) {
        out.raw("<cbc:TotalTransportHandlingUnitQuantity>")
            .int(bultos)
            .raw("</cbc:TotalTransportHandlingUnitQuantity>");
    }
    let transbordo = c.get(envio, "indTransbordo", |e| e.ind_transbordo)?;
    out.raw("<cbc:SplitConsignmentIndicator>")
        .raw(if transbordo == Some(true) {
            "true"
        } else {
            "false"
        })
        .raw("</cbc:SplitConsignmentIndicator><cac:ShipmentStage><cbc:TransportModeCode>")
        .text(c.get(envio, "modTraslado", |e| e.mod_traslado.as_deref())?)
        .raw("</cbc:TransportModeCode><cac:TransitPeriod><cbc:StartDate>")
        .date(c.obj(envio, "fecTraslado", |e| e.fec_traslado.as_ref())?)?
        .raw("</cbc:StartDate></cac:TransitPeriod>");
    let transportista = c.obj(envio, "transportista", |e| e.transportista.as_ref())?;
    if let Some(t) = transportista {
        out.raw("<cac:CarrierParty><cac:PartyIdentification><cbc:ID schemeID=\"")
            .text(t.tipo_doc.as_deref())
            .raw("\">")
            .text(t.num_doc.as_deref())
            .raw("</cbc:ID></cac:PartyIdentification><cac:PartyName><cbc:Name><![CDATA[")
            .cdata(t.rzn_social.as_deref())
            .raw("]]></cbc:Name></cac:PartyName></cac:CarrierParty><cac:TransportMeans><cac:RoadTransport><cbc:LicensePlateID>")
            .text(t.placa.as_deref())
            .raw("</cbc:LicensePlateID></cac:RoadTransport></cac:TransportMeans><cac:DriverPerson><cbc:ID schemeID=\"")
            .text(t.chofer_tipo_doc.as_deref())
            .raw("\">")
            .text(t.chofer_doc.as_deref())
            .raw("</cbc:ID></cac:DriverPerson>");
    }
    let llegada = c.obj(envio, "llegada", |e| e.llegada.as_ref())?;
    out.raw("</cac:ShipmentStage><cac:Delivery><cac:DeliveryAddress><cbc:ID>")
        .text(c.get(llegada, "ubigueo", |d| d.ubigueo.as_deref())?)
        .raw("</cbc:ID><cbc:StreetName>")
        .text(c.get(llegada, "direccion", |d| d.direccion.as_deref())?)
        .raw("</cbc:StreetName></cac:DeliveryAddress></cac:Delivery>");
    let contenedor = c.get(envio, "numContenedor", |e| e.num_contenedor.as_deref())?;
    if truthy_str(contenedor) {
        out.raw("<cac:TransportHandlingUnit><cbc:ID>")
            .text(contenedor)
            .raw("</cbc:ID></cac:TransportHandlingUnit>");
    }
    let partida = c.obj(envio, "partida", |e| e.partida.as_ref())?;
    out.raw("<cac:OriginAddress><cbc:ID>")
        .text(c.get(partida, "ubigueo", |d| d.ubigueo.as_deref())?)
        .raw("</cbc:ID><cbc:StreetName>")
        .text(c.get(partida, "direccion", |d| d.direccion.as_deref())?)
        .raw("</cbc:StreetName></cac:OriginAddress>");
    let puerto = c.get(envio, "codPuerto", |e| e.cod_puerto.as_deref())?;
    if truthy_str(puerto) {
        out.raw("<cac:FirstArrivalPortLocation><cbc:ID>")
            .text(puerto)
            .raw("</cbc:ID></cac:FirstArrivalPortLocation>");
    }
    out.raw("</cac:Shipment>");
    for (index, det) in doc.details.iter().flatten().enumerate() {
        out.raw("<cac:DespatchLine><cbc:ID>")
            .index(index)
            .raw("</cbc:ID><cbc:DeliveredQuantity unitCode=\"")
            .text(det.unidad.as_deref())
            .raw("\">")
            .num(det.cantidad)
            .raw("</cbc:DeliveredQuantity><cac:OrderLineReference><cbc:LineID>")
            .index(index)
            .raw("</cbc:LineID></cac:OrderLineReference><cac:Item><cbc:Name><![CDATA[")
            .cdata(det.descripcion.as_deref())
            .raw("]]></cbc:Name><cac:SellersItemIdentification><cbc:ID>")
            .text(det.codigo.as_deref())
            .raw("</cbc:ID></cac:SellersItemIdentification>");
        commodity(&mut out, det.cod_prod_sunat.as_deref());
        out.raw("</cac:Item></cac:DespatchLine>");
    }
    out.raw("</DespatchAdvice>");
    Ok(out.finish())
}

fn legacy_party(out: &mut Out<'_>, c: Ctx, tag: &str, party: Option<&Client>) -> Result<()> {
    out.raw("<cac:")
        .raw(tag)
        .raw("><cbc:CustomerAssignedAccountID schemeID=\"")
        .text(c.get(party, "tipoDoc", |p| p.tipo_doc.as_deref())?)
        .raw("\">")
        .text(c.get(party, "numDoc", |p| p.num_doc.as_deref())?)
        .raw("</cbc:CustomerAssignedAccountID><cac:Party><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(party, "rznSocial", |p| p.rzn_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:")
        .raw(tag)
        .raw(">");
    Ok(())
}

fn commodity(out: &mut Out<'_>, code: Option<&str>) {
    if truthy_str(code) {
        out.raw(r#"<cac:CommodityClassification><cbc:ItemClassificationCode listID="UNSPSC" listAgencyName="GS1 US" listName="Item Classification">"#)
            .text(code)
            .raw("</cbc:ItemClassificationCode></cac:CommodityClassification>");
    }
}

/// Guía versión 2022: remitente (`09`) o transportista (`31`, anexo N.° 14
/// de la R. S. 123-2022/SUNAT, con las rutas de la R. S. 108-2026/SUNAT).
pub(crate) fn render_2022(doc: &Despatch, options: &RenderOptions) -> Result<String> {
    let mut out = Out::new(options, TEMPLATE_2022);
    let c = out.ctx();
    let carrier = doc.tipo_doc.as_deref() == Some(CARRIER_TYPE);
    // En la guía transportista el remitente es obligatorio (campos 16 y 17).
    let remitente = if carrier {
        Some(doc.remitente.as_ref().ok_or_else(|| {
            c.document(CoreError::MissingField {
                model: "Despatch",
                field: "remitente",
            })
        })?)
    } else {
        None
    };
    header(&mut out, doc, "2.0")?;
    out.raw(r#"<cbc:DespatchAdviceTypeCode listAgencyName="PE:SUNAT" listName="Tipo de Documento" listURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:catalogo01">"#)
        .text(doc.tipo_doc.as_deref())
        .raw("</cbc:DespatchAdviceTypeCode>");
    observation(&mut out, doc);
    for rel in doc.add_docs.iter().flatten() {
        let nro = rel.nro().map_err(|err| c.document(err))?;
        out.raw("<cac:AdditionalDocumentReference><cbc:ID>")
            .text(Some(nro))
            .raw(r#"</cbc:ID><cbc:DocumentTypeCode listAgencyName="PE:SUNAT" listName="Documento relacionado al transporte" listURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:catalogo61">"#)
            .text(rel.tipo.as_deref())
            .raw("</cbc:DocumentTypeCode><cbc:DocumentType>")
            .text(rel.tipo_desc.as_deref())
            .raw("</cbc:DocumentType>");
        if truthy_str(rel.emisor.as_deref()) {
            out.raw("<cac:IssuerParty><cac:PartyIdentification><cbc:ID schemeID=\"6")
                .raw(IDENTITY_SCHEME)
                .text(rel.emisor.as_deref())
                .raw("</cbc:ID></cac:PartyIdentification></cac:IssuerParty>");
        }
        out.raw("</cac:AdditionalDocumentReference>");
    }
    let emp = doc.company.as_ref();
    shared::signature(&mut out, emp)?;
    out.raw("<cac:DespatchSupplierParty><cac:Party><cac:PartyIdentification><cbc:ID schemeID=\"6")
        .raw(IDENTITY_SCHEME)
        .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
        .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(emp, "razonSocial", |e| e.razon_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:DespatchSupplierParty>");
    party_2022(
        &mut out,
        c,
        "DeliveryCustomerParty",
        doc.destinatario.as_ref(),
    )?;
    if doc.comprador.is_some() {
        party_2022(&mut out, c, "BuyerCustomerParty", doc.comprador.as_ref())?;
    }
    if doc.tercero.is_some() {
        party_2022(&mut out, c, "SellerSupplierParty", doc.tercero.as_ref())?;
    }
    if carrier && doc.pagador_flete.is_some() {
        party_2022(
            &mut out,
            c,
            "OriginatorCustomerParty",
            doc.pagador_flete.as_ref(),
        )?;
    }
    let envio = doc.envio.as_ref();
    out.raw("<cac:Shipment><cbc:ID>SUNAT_Envio</cbc:ID>");
    if !carrier {
        out.raw(r#"<cbc:HandlingCode listAgencyName="PE:SUNAT" listName="Motivo de traslado" listURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:catalogo20">"#)
            .text(c.get(envio, "codTraslado", |e| e.cod_traslado.as_deref())?)
            .raw("</cbc:HandlingCode>");
        let des_traslado = c.get(envio, "desTraslado", |e| e.des_traslado.as_deref())?;
        if truthy_str(des_traslado) {
            out.raw("<cbc:HandlingInstructions>")
                .text(des_traslado)
                .raw("</cbc:HandlingInstructions>");
        }
        let sustento = c.get(envio, "sustentoPeso", |e| e.sustento_peso.as_deref())?;
        if truthy_str(sustento) {
            out.raw("<cbc:Information>")
                .text(sustento)
                .raw("</cbc:Information>");
        }
    }
    out.raw("<cbc:GrossWeightMeasure unitCode=\"")
        .text(c.get(envio, "undPesoTotal", |e| e.und_peso_total.as_deref())?)
        .raw("\">")
        .fmt(c.get(envio, "pesoTotal", |e| e.peso_total)?, 3)
        .raw("</cbc:GrossWeightMeasure>");
    let peso_items = c.get(envio, "pesoItems", |e| e.peso_items)?;
    if !carrier && peso_items.is_some_and(|p| !p.is_zero()) {
        out.raw("<cbc:NetWeightMeasure unitCode=\"KGM\">")
            .fmt(peso_items, 3)
            .raw("</cbc:NetWeightMeasure>");
    }
    let bultos = c.get(envio, "numBultos", |e| e.num_bultos)?;
    if !carrier && bultos.is_some_and(|n| n != 0) {
        out.raw("<cbc:TotalTransportHandlingUnitQuantity>")
            .int(bultos)
            .raw("</cbc:TotalTransportHandlingUnitQuantity>");
    }
    for indicador in c
        .obj(envio, "indicadores", |e| e.indicadores.as_ref())?
        .into_iter()
        .flatten()
    {
        out.raw("<cbc:SpecialInstructions>")
            .text(Some(indicador))
            .raw("</cbc:SpecialInstructions>");
    }
    if carrier {
        if let Some(sub) = c.obj(envio, "subcontratador", |e| e.subcontratador.as_ref())? {
            // Campos 60 y 61, con las rutas de la R. S. 108-2026/SUNAT.
            out.raw("<cac:Consignment><cbc:ID>SUNAT_Envio</cbc:ID><cac:LogisticsOperatorParty><cac:PartyIdentification><cbc:ID schemeID=\"")
                .text(sub.tipo_doc.as_deref())
                .raw("\">")
                .text(sub.num_doc.as_deref())
                .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
                .cdata(sub.rzn_social.as_deref())
                .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:LogisticsOperatorParty></cac:Consignment>");
        }
        out.raw("<cac:ShipmentStage>");
    } else {
        out.raw(r#"<cac:ShipmentStage><cbc:TransportModeCode listName="Modalidad de traslado" listAgencyName="PE:SUNAT" listURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:catalogo18">"#)
            .text(c.get(envio, "modTraslado", |e| e.mod_traslado.as_deref())?)
            .raw("</cbc:TransportModeCode>");
    }
    let fec_traslado = c.obj(envio, "fecTraslado", |e| e.fec_traslado.as_ref())?;
    if fec_traslado.is_some() {
        out.raw("<cac:TransitPeriod><cbc:StartDate>")
            .date(fec_traslado)?
            .raw("</cbc:StartDate></cac:TransitPeriod>");
    }
    let transportista = c.obj(envio, "transportista", |e| e.transportista.as_ref())?;
    if carrier {
        // Campo 10: registro MTC del transportista que emite la guía.
        if let Some(mtc) = transportista
            .and_then(|t| t.nro_mtc.as_deref())
            .filter(|m| truthy_str(Some(m)))
        {
            out.raw("<cac:CarrierParty><cac:PartyLegalEntity><cbc:CompanyID>")
                .text(Some(mtc))
                .raw("</cbc:CompanyID></cac:PartyLegalEntity></cac:CarrierParty>");
        }
    } else if let Some(t) = transportista {
        out.raw("<cac:CarrierParty><cac:PartyIdentification><cbc:ID schemeID=\"")
            .text(t.tipo_doc.as_deref())
            .raw("\">")
            .text(t.num_doc.as_deref())
            .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
            .cdata(t.rzn_social.as_deref())
            .raw("]]></cbc:RegistrationName><cbc:CompanyID>")
            .text(t.nro_mtc.as_deref())
            .raw("</cbc:CompanyID></cac:PartyLegalEntity></cac:CarrierParty>");
    }
    let entrega = c.obj(envio, "fecEntregaBienes", |e| e.fec_entrega_bienes.as_ref())?;
    if !carrier && entrega.is_some() {
        out.raw("<cac:LoadingTransportEvent><cbc:OccurrenceDate>")
            .date(entrega)?
            .raw("</cbc:OccurrenceDate></cac:LoadingTransportEvent>");
    }
    for chofer in c
        .obj(envio, "choferes", |e| e.choferes.as_ref())?
        .into_iter()
        .flatten()
    {
        out.raw("<cac:DriverPerson><cbc:ID schemeID=\"")
            .text(chofer.tipo_doc.as_deref())
            .raw(IDENTITY_SCHEME)
            .text(chofer.nro_doc.as_deref())
            .raw("</cbc:ID><cbc:FirstName>")
            .text(chofer.nombres.as_deref())
            .raw("</cbc:FirstName><cbc:FamilyName>")
            .text(chofer.apellidos.as_deref())
            .raw("</cbc:FamilyName><cbc:JobTitle>")
            .text(chofer.tipo.as_deref())
            .raw("</cbc:JobTitle><cac:IdentityDocumentReference><cbc:ID>")
            .text(chofer.licencia.as_deref())
            .raw("</cbc:ID></cac:IdentityDocumentReference></cac:DriverPerson>");
    }
    out.raw("</cac:ShipmentStage><cac:Delivery><cac:DeliveryAddress>");
    address_2022(
        &mut out,
        c,
        c.obj(envio, "llegada", |e| e.llegada.as_ref())?,
    )?;
    out.raw("</cac:DeliveryAddress><cac:Despatch><cac:DespatchAddress>");
    address_2022(
        &mut out,
        c,
        c.obj(envio, "partida", |e| e.partida.as_ref())?,
    )?;
    out.raw("</cac:DespatchAddress>");
    if let Some(rem) = remitente {
        // Campos 16 y 17, con la ruta de la R. S. 108-2026/SUNAT.
        out.raw("<cac:DespatchParty><cac:PartyIdentification><cbc:ID schemeID=\"")
            .text(rem.tipo_doc.as_deref())
            .raw(IDENTITY_SCHEME)
            .text(rem.num_doc.as_deref())
            .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
            .cdata(rem.rzn_social.as_deref())
            .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:DespatchParty>");
    }
    out.raw("</cac:Despatch></cac:Delivery>");
    for (index, precinto) in c
        .obj(envio, "contenedores", |e| e.contenedores.as_ref())?
        .into_iter()
        .flatten()
        .enumerate()
    {
        out.raw("<cac:TransportHandlingUnit><cac:Package><cbc:ID>")
            .index(index)
            .raw("</cbc:ID><cbc:TraceID>")
            .text(Some(precinto))
            .raw("</cbc:TraceID></cac:Package></cac:TransportHandlingUnit>");
    }
    if let Some(vehiculo) = c.obj(envio, "vehiculo", |e| e.vehiculo.as_ref())? {
        vehicle_2022(&mut out, vehiculo);
    }
    let puerto = c.obj(envio, "puerto", |e| e.puerto.as_ref())?;
    if let Some(puerto) = puerto {
        port(&mut out, puerto, "Puertos", "catalogo63", "1");
    } else if let Some(aeropuerto) = c.obj(envio, "aeropuerto", |e| e.aeropuerto.as_ref())? {
        port(&mut out, aeropuerto, "Aeropuertos", "catalogo64", "2");
    }
    out.raw("</cac:Shipment>");
    for (index, det) in doc.details.iter().flatten().enumerate() {
        out.raw("<cac:DespatchLine><cbc:ID>")
            .index(index)
            .raw("</cbc:ID><cbc:DeliveredQuantity unitCode=\"")
            .text(det.unidad.as_deref())
            .raw("\">")
            .limit(det.cantidad, 10)
            .raw("</cbc:DeliveredQuantity><cac:OrderLineReference><cbc:LineID>")
            .index(index)
            .raw("</cbc:LineID></cac:OrderLineReference><cac:Item><cbc:Description><![CDATA[")
            .cdata(det.descripcion.as_deref())
            .raw("]]></cbc:Description><cac:SellersItemIdentification><cbc:ID>")
            .text(det.codigo.as_deref())
            .raw("</cbc:ID></cac:SellersItemIdentification>");
        commodity(&mut out, det.cod_prod_sunat.as_deref());
        for atr in det.atributos.iter().flatten() {
            out.raw("<cac:AdditionalItemProperty ><cbc:Name>")
                .text(atr.name.as_deref())
                .raw("</cbc:Name><cbc:NameCode>")
                .text(atr.code.as_deref())
                .raw("</cbc:NameCode>");
            if truthy_str(atr.value.as_deref()) {
                out.raw("<cbc:Value>")
                    .text(atr.value.as_deref())
                    .raw("</cbc:Value>");
            }
            out.raw("</cac:AdditionalItemProperty>");
        }
        out.raw("</cac:Item></cac:DespatchLine>");
    }
    out.raw("</DespatchAdvice>");
    Ok(out.finish())
}

fn party_2022(out: &mut Out<'_>, c: Ctx, tag: &str, party: Option<&Client>) -> Result<()> {
    out.raw("<cac:")
        .raw(tag)
        .raw("><cac:Party><cac:PartyIdentification><cbc:ID schemeID=\"")
        .text(c.get(party, "tipoDoc", |p| p.tipo_doc.as_deref())?)
        .raw(IDENTITY_SCHEME)
        .text(c.get(party, "numDoc", |p| p.num_doc.as_deref())?)
        .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(party, "rznSocial", |p| p.rzn_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:")
        .raw(tag)
        .raw(">");
    Ok(())
}

fn address_2022(out: &mut Out<'_>, c: Ctx, dir: Option<&Direction>) -> Result<()> {
    out.raw(r#"<cbc:ID schemeAgencyName="PE:INEI" schemeName="Ubigeos">"#)
        .text(c.get(dir, "ubigueo", |d| d.ubigueo.as_deref())?)
        .raw("</cbc:ID>");
    let cod_local = c.get(dir, "codLocal", |d| d.cod_local.as_deref())?;
    if truthy_str(cod_local) {
        out.raw("<cbc:AddressTypeCode listID=\"")
            .text(c.get(dir, "ruc", |d| d.ruc.as_deref())?)
            .raw("\">")
            .text(cod_local)
            .raw("</cbc:AddressTypeCode>");
    }
    out.raw("<cac:AddressLine><cbc:Line>")
        .text(c.get(dir, "direccion", |d| d.direccion.as_deref())?)
        .raw("</cbc:Line></cac:AddressLine>");
    Ok(())
}

fn vehicle_2022(out: &mut Out<'_>, vehiculo: &Vehicle) {
    out.raw("<cac:TransportHandlingUnit><cac:TransportEquipment><cbc:ID>")
        .text(vehiculo.placa.as_deref())
        .raw("</cbc:ID>");
    if truthy_str(vehiculo.nro_circulacion.as_deref()) {
        out.raw("<cac:ApplicableTransportMeans><cbc:RegistrationNationalityID>")
            .text(vehiculo.nro_circulacion.as_deref())
            .raw("</cbc:RegistrationNationalityID></cac:ApplicableTransportMeans>");
    }
    for sec in vehiculo.secundarios.iter().flatten() {
        out.raw("<cac:AttachedTransportEquipment><cbc:ID>")
            .text(sec.placa.as_deref())
            .raw("</cbc:ID>");
        if truthy_str(sec.nro_circulacion.as_deref()) {
            out.raw("<cac:ApplicableTransportMeans><cbc:RegistrationNationalityID>")
                .text(sec.nro_circulacion.as_deref())
                .raw("</cbc:RegistrationNationalityID></cac:ApplicableTransportMeans>");
        }
        if truthy_str(sec.nro_autorizacion.as_deref()) {
            authorization(out, sec);
        }
        out.raw("</cac:AttachedTransportEquipment>");
    }
    if truthy_str(vehiculo.nro_autorizacion.as_deref()) {
        authorization(out, vehiculo);
    }
    out.raw("</cac:TransportEquipment></cac:TransportHandlingUnit>");
}

fn authorization(out: &mut Out<'_>, vehiculo: &Vehicle) {
    out.raw("<cac:ShipmentDocumentReference><cbc:ID schemeID=\"")
        .text(vehiculo.cod_emisor.as_deref())
        .raw(r#"" schemeName="Entidad Autorizadora" schemeAgencyName="PE:SUNAT">"#)
        .text(vehiculo.nro_autorizacion.as_deref())
        .raw("</cbc:ID></cac:ShipmentDocumentReference>");
}

fn port(out: &mut Out<'_>, puerto: &Puerto, name: &str, catalog: &str, location: &str) {
    out.raw(r#"<cac:FirstArrivalPortLocation><cbc:ID schemeAgencyName="PE:SUNAT" schemeName=""#)
        .raw(name)
        .raw(r#"" schemeURI="urn:pe:gob:sunat:cpe:see:gem:catalogos:"#)
        .raw(catalog)
        .raw("\">")
        .text(puerto.codigo.as_deref())
        .raw("</cbc:ID><cbc:LocationTypeCode>")
        .raw(location)
        .raw("</cbc:LocationTypeCode><cbc:Name>")
        .text(puerto.nombre.as_deref())
        .raw("</cbc:Name></cac:FirstArrivalPortLocation>");
}
