//! Comprobante de percepción (`PerceptionParser`).

use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use rust_decimal::Decimal;

use super::retention::{company, detail_head, exchange, provider};
use crate::convert::{DateContext, explode_part, to_float};
use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::Lookup;

super::parser!(PerceptionParser, Perception, "Perception");

fn float<'a>(
    xml: &Lookup<'a>,
    query: &str,
    context: Option<Node<'a>>,
    default: &str,
) -> Result<Decimal, XmlParserError> {
    to_float(&xml.value_def(query, context, default)?)
}

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<Perception, XmlParserError> {
    let root = Some(xml.root());
    let id = xml.value("cbc:ID", None)?;
    let fecha_emision = dates.parse(&xml.value("cbc:IssueDate", None)?)?;
    let company = company(xml)?;
    let proveedor = provider(xml)?;
    let regimen = xml.value("sac:SUNATPerceptionSystemCode", None)?;
    let tasa = float(xml, "sac:SUNATPerceptionPercent", root, "0")?;
    let observacion = xml.value("cbc:Note", None)?;
    let imp_percibido = float(xml, "cbc:TotalInvoiceAmount", root, "0")?;
    let imp_cobrado = float(xml, "sac:SUNATTotalCashed", root, "0")?;
    let mut details = Vec::new();
    for node in xml.nodes("sac:SUNATPerceptionDocumentReference", root)? {
        details.push(detail(xml, dates, node)?);
    }
    Ok(Perception {
        serie: explode_part(&id, 0),
        correlativo: explode_part(&id, 1),
        fecha_emision: Some(fecha_emision),
        company: Some(company),
        proveedor: Some(proveedor),
        regimen: Some(regimen),
        tasa: Some(tasa),
        imp_percibido: Some(imp_percibido),
        imp_cobrado: Some(imp_cobrado),
        observacion: Some(observacion),
        details: Some(details),
    })
}

fn detail<'a>(
    xml: &Lookup<'a>,
    dates: &DateContext,
    node: Node<'a>,
) -> Result<PerceptionDetail, XmlParserError> {
    let head = detail_head(xml, dates, node)?;
    let mut det = PerceptionDetail {
        tipo_doc: Some(head.tipo_doc),
        num_doc: head.num_doc,
        fecha_emision: Some(head.fecha_emision),
        imp_total: Some(head.imp_total),
        moneda: Some(head.moneda),
        cobros: Some(head.payments),
        ..PerceptionDetail::default()
    };
    let Some(info) = xml.node("sac:SUNATPerceptionInformation", Some(node))? else {
        det.imp_percibido = Some(Decimal::ZERO);
        det.imp_cobrar = Some(Decimal::ZERO);
        det.fecha_percepcion = Some(dates.now());
        return Ok(det);
    };
    let at = Some(info);
    det.imp_percibido = Some(float(xml, "sac:SUNATPerceptionAmount", at, "")?);
    det.fecha_percepcion = Some(dates.parse(&xml.value("sac:SUNATPerceptionDate", at)?)?);
    det.imp_cobrar = Some(float(xml, "sac:SUNATNetTotalCashed", at, "")?);
    det.tipo_cambio = exchange(xml, dates, info)?;
    Ok(det)
}
