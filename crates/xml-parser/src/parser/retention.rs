//! Comprobante de retención (`RetentionParser`).

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::retention::{Exchange, Payment, Retention, RetentionDetail};
use rust_decimal::Decimal;

use super::full_address;
use crate::convert::{DateContext, explode_part, to_float};
use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::{Lookup, node_attribute, node_value};

super::parser!(RetentionParser, Retention, "Retention");

fn float<'a>(
    xml: &Lookup<'a>,
    query: &str,
    context: Option<Node<'a>>,
    default: &str,
) -> Result<Decimal, XmlParserError> {
    to_float(&xml.value_def(query, context, default)?)
}

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<Retention, XmlParserError> {
    let root = Some(xml.root());
    // Sin contexto, la consulta parte del elemento raíz.
    let id = xml.value("cbc:ID", None)?;
    let fecha_emision = dates.parse(&xml.value("cbc:IssueDate", None)?)?;
    let company = company(xml)?;
    let proveedor = provider(xml)?;
    let regimen = xml.value("sac:SUNATRetentionSystemCode", None)?;
    let tasa = float(xml, "sac:SUNATRetentionPercent", root, "0")?;
    let observacion = xml.value("cbc:Note", None)?;
    let imp_retenido = float(xml, "cbc:TotalInvoiceAmount", root, "0")?;
    let imp_pagado = float(xml, "sac:SUNATTotalPaid", root, "0")?;
    let mut details = Vec::new();
    for node in xml.nodes("sac:SUNATRetentionDocumentReference", root)? {
        details.push(detail(xml, dates, node)?);
    }
    Ok(Retention {
        serie: explode_part(&id, 0),
        correlativo: explode_part(&id, 1),
        fecha_emision: Some(fecha_emision),
        company: Some(company),
        proveedor: Some(proveedor),
        regimen: Some(regimen),
        tasa: Some(tasa),
        imp_retenido: Some(imp_retenido),
        imp_pagado: Some(imp_pagado),
        observacion: Some(observacion),
        details: Some(details),
    })
}

pub(super) fn company(xml: &Lookup<'_>) -> Result<Company, XmlParserError> {
    let node = xml.node("cac:AgentParty", Some(xml.root()))?;
    Ok(Company {
        ruc: Some(xml.value("cac:PartyIdentification/cbc:ID", node)?),
        nombre_comercial: Some(xml.value("cac:PartyName/cbc:Name", node)?),
        razon_social: Some(xml.value("cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        address: full_address(xml, "cac:PostalAddress", node)?,
        ..Company::default()
    })
}

pub(super) fn provider(xml: &Lookup<'_>) -> Result<Client, XmlParserError> {
    let node = xml.node("cac:ReceiverParty", Some(xml.root()))?;
    let ident = xml.node("cac:PartyIdentification/cbc:ID", node)?;
    let num_doc = node_value(ident);
    let tipo_doc = node_attribute(ident, "schemeID")?;
    Ok(Client {
        num_doc,
        tipo_doc: Some(tipo_doc),
        rzn_social: Some(xml.value("cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        address: full_address(xml, "cac:PostalAddress", node)?,
        ..Client::default()
    })
}

/// Campos comunes del detalle. El orden de lectura es contractual: decide
/// qué error se informa primero cuando faltan varios nodos.
pub(super) struct DetailHead {
    pub(super) tipo_doc: String,
    pub(super) num_doc: Option<String>,
    pub(super) fecha_emision: jiff::Zoned,
    pub(super) imp_total: Decimal,
    pub(super) moneda: String,
    pub(super) payments: Vec<Payment>,
}

pub(super) fn detail_head<'a>(
    xml: &Lookup<'a>,
    dates: &DateContext,
    node: Node<'a>,
) -> Result<DetailHead, XmlParserError> {
    let at = Some(node);
    let temp = xml.node("cbc:ID", at)?;
    let mount = xml.node("cbc:TotalInvoiceAmount", at)?;
    let tipo_doc = node_attribute(temp, "schemeID")?;
    let num_doc = node_value(temp);
    let fecha_emision = dates.parse(&xml.value("cbc:IssueDate", at)?)?;
    let imp_total = to_float(&node_value(mount).unwrap_or_default())?;
    let moneda = node_attribute(mount, "currencyID")?;
    let mut payments = Vec::new();
    for pay in xml.nodes("cac:Payment", at)? {
        let amount = xml.node("cbc:PaidAmount", Some(pay))?;
        let moneda = node_attribute(amount, "currencyID")?;
        let importe = to_float(&node_value(amount).unwrap_or_default())?;
        // `cbc:PaidDate` se consulta sin contexto, es decir, desde el raíz.
        let fecha = dates.parse(&xml.value("cbc:PaidDate", None)?)?;
        payments.push(Payment {
            moneda: Some(moneda),
            importe: Some(importe),
            fecha: Some(fecha),
        });
    }
    Ok(DetailHead {
        tipo_doc,
        num_doc,
        fecha_emision,
        imp_total,
        moneda,
        payments,
    })
}

pub(super) fn exchange<'a>(
    xml: &Lookup<'a>,
    dates: &DateContext,
    info: Node<'a>,
) -> Result<Option<Exchange>, XmlParserError> {
    let Some(cambio) = xml.node("cac:ExchangeRate", Some(info))? else {
        return Ok(None);
    };
    let at = Some(cambio);
    Ok(Some(Exchange {
        moneda_ref: Some(xml.value("cbc:SourceCurrencyCode", at)?),
        moneda_obj: Some(xml.value("cbc:TargetCurrencyCode", at)?),
        factor: Some(float(xml, "cbc:CalculationRate", at, "0")?),
        fecha: Some(dates.parse(&xml.value("cbc:Date", at)?)?),
    }))
}

fn detail<'a>(
    xml: &Lookup<'a>,
    dates: &DateContext,
    node: Node<'a>,
) -> Result<RetentionDetail, XmlParserError> {
    let head = detail_head(xml, dates, node)?;
    let mut det = RetentionDetail {
        tipo_doc: Some(head.tipo_doc),
        num_doc: head.num_doc,
        fecha_emision: Some(head.fecha_emision),
        imp_total: Some(head.imp_total),
        moneda: Some(head.moneda),
        pagos: Some(head.payments),
        ..RetentionDetail::default()
    };
    let Some(info) = xml.node("sac:SUNATRetentionInformation", Some(node))? else {
        det.imp_retenido = Some(Decimal::ZERO);
        det.imp_pagar = Some(Decimal::ZERO);
        det.fecha_retencion = Some(dates.now());
        return Ok(det);
    };
    let at = Some(info);
    det.imp_retenido = Some(float(xml, "sac:SUNATRetentionAmount", at, "")?);
    det.fecha_retencion = Some(dates.parse(&xml.value("sac:SUNATRetentionDate", at)?)?);
    det.imp_pagar = Some(float(xml, "sac:SUNATNetTotalPaid", at, "")?);
    det.tipo_cambio = exchange(xml, dates, info)?;
    Ok(det)
}
