//! `retention.xml.twig` y `perception.xml.twig`.
//!
//! Las dos plantillas son iguales salvo nombres de nodos y un detalle de
//! escape: en retención el nombre del receptor dentro de `CDATA` se imprime
//! **con** escape (sin `|raw`); en percepción, sin escape.

use cpeforge_core::Decimal;
use cpeforge_core::Zoned;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::{Exchange, Payment, Retention};

use super::invoice20::postal_address_20;
use super::shared::{self, EMPTY_EXTENSIONS, XML_DECLARATION};
use crate::XmlError;
use crate::options::RenderOptions;
use crate::render::{Out, Result, truthy_list, truthy_num, truthy_str};

pub(crate) const RETENTION_TEMPLATE: &str = "retention.xml.twig";
pub(crate) const PERCEPTION_TEMPLATE: &str = "perception.xml.twig";

struct Names {
    root: &'static str,
    system_code: &'static str,
    percent: &'static str,
    total_paid: &'static str,
    reference: &'static str,
    information: &'static str,
    amount: &'static str,
    date: &'static str,
    net_total: &'static str,
    receiver_raw: bool,
}

const RETENTION: Names = Names {
    root: "Retention",
    system_code: "SUNATRetentionSystemCode",
    percent: "SUNATRetentionPercent",
    total_paid: "SUNATTotalPaid",
    reference: "SUNATRetentionDocumentReference",
    information: "SUNATRetentionInformation",
    amount: "SUNATRetentionAmount",
    date: "SUNATRetentionDate",
    net_total: "SUNATNetTotalPaid",
    receiver_raw: false,
};

const PERCEPTION: Names = Names {
    root: "Perception",
    system_code: "SUNATPerceptionSystemCode",
    percent: "SUNATPerceptionPercent",
    total_paid: "SUNATTotalCashed",
    reference: "SUNATPerceptionDocumentReference",
    information: "SUNATPerceptionInformation",
    amount: "SUNATPerceptionAmount",
    date: "SUNATPerceptionDate",
    net_total: "SUNATNetTotalCashed",
    receiver_raw: true,
};

/// Comprobante relacionado, común a ambos detalles.
struct Detail<'a> {
    tipo_doc: Option<&'a str>,
    num_doc: Option<&'a str>,
    fecha_emision: Option<&'a Zoned>,
    imp_total: Option<Decimal>,
    moneda: Option<&'a str>,
    payments: Option<&'a Vec<Payment>>,
    amount: Option<Decimal>,
    net: Option<Decimal>,
    date: Option<&'a Zoned>,
    exchange: Option<&'a Exchange>,
}

/// Cabecera común a ambos documentos.
struct Header<'a> {
    serie: Option<&'a str>,
    correlativo: Option<&'a str>,
    fecha_emision: Option<&'a Zoned>,
    company: Option<&'a Company>,
    proveedor: Option<&'a Client>,
    regimen: Option<&'a str>,
    tasa: Option<Decimal>,
    total: Option<Decimal>,
    paid: Option<Decimal>,
    observacion: Option<&'a str>,
}

pub(crate) fn render_retention(doc: &Retention, options: &RenderOptions) -> Result<String> {
    let header = Header {
        serie: doc.serie.as_deref(),
        correlativo: doc.correlativo.as_deref(),
        fecha_emision: doc.fecha_emision.as_ref(),
        company: doc.company.as_ref(),
        proveedor: doc.proveedor.as_ref(),
        regimen: doc.regimen.as_deref(),
        tasa: doc.tasa,
        total: doc.imp_retenido,
        paid: doc.imp_pagado,
        observacion: doc.observacion.as_deref(),
    };
    let details: Vec<Detail<'_>> = doc
        .details
        .iter()
        .flatten()
        .map(|d| Detail {
            tipo_doc: d.tipo_doc.as_deref(),
            num_doc: d.num_doc.as_deref(),
            fecha_emision: d.fecha_emision.as_ref(),
            imp_total: d.imp_total,
            moneda: d.moneda.as_deref(),
            payments: d.pagos.as_ref(),
            amount: d.imp_retenido,
            net: d.imp_pagar,
            date: d.fecha_retencion.as_ref(),
            exchange: d.tipo_cambio.as_ref(),
        })
        .collect();
    render(&header, &details, &RETENTION, RETENTION_TEMPLATE, options)
}

pub(crate) fn render_perception(doc: &Perception, options: &RenderOptions) -> Result<String> {
    let header = Header {
        serie: doc.serie.as_deref(),
        correlativo: doc.correlativo.as_deref(),
        fecha_emision: doc.fecha_emision.as_ref(),
        company: doc.company.as_ref(),
        proveedor: doc.proveedor.as_ref(),
        regimen: doc.regimen.as_deref(),
        tasa: doc.tasa,
        total: doc.imp_percibido,
        paid: doc.imp_cobrado,
        observacion: doc.observacion.as_deref(),
    };
    let details: Vec<Detail<'_>> = doc
        .details
        .iter()
        .flatten()
        .map(|d| Detail {
            tipo_doc: d.tipo_doc.as_deref(),
            num_doc: d.num_doc.as_deref(),
            fecha_emision: d.fecha_emision.as_ref(),
            imp_total: d.imp_total,
            moneda: d.moneda.as_deref(),
            payments: d.cobros.as_ref(),
            amount: d.imp_percibido,
            net: d.imp_cobrar,
            date: d.fecha_percepcion.as_ref(),
            exchange: d.tipo_cambio.as_ref(),
        })
        .collect();
    render(&header, &details, &PERCEPTION, PERCEPTION_TEMPLATE, options)
}

fn render(
    doc: &Header<'_>,
    details: &[Detail<'_>],
    names: &Names,
    template: &'static str,
    options: &RenderOptions,
) -> Result<String> {
    let mut out = Out::new(options, template);
    let c = out.ctx();
    out.raw(XML_DECLARATION)
        .raw("<")
        .raw(names.root)
        .raw(" xmlns=\"urn:sunat:names:specification:ubl:peru:schema:xsd:")
        .raw(names.root)
        .raw(r#"-1" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1">"#)
        .raw(EMPTY_EXTENSIONS)
        .raw("<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>1.0</cbc:CustomizationID>");
    let emp = doc.company;
    shared::signature(&mut out, emp)?;
    out.raw("<cbc:ID>")
        .text(doc.serie)
        .raw("-")
        .text(doc.correlativo)
        .raw("</cbc:ID><cbc:IssueDate>")
        .date(doc.fecha_emision)?
        .raw("</cbc:IssueDate><cbc:IssueTime>")
        .time(doc.fecha_emision)?
        .raw("</cbc:IssueTime><cac:AgentParty><cac:PartyIdentification><cbc:ID schemeID=\"6\">")
        .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
        .raw("</cbc:ID></cac:PartyIdentification><cac:PartyName><cbc:Name><![CDATA[")
        .cdata(c.get(emp, "nombreComercial", |e| e.nombre_comercial.as_deref())?)
        .raw("]]></cbc:Name></cac:PartyName>");
    let addr = c.obj(emp, "address", |e| e.address.as_ref())?;
    postal_address_20(&mut out, c, addr)?;
    out.raw("<cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(emp, "razonSocial", |e| e.razon_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:AgentParty><cac:ReceiverParty><cac:PartyIdentification><cbc:ID schemeID=\"")
        .text(c.get(doc.proveedor, "tipoDoc", |p| p.tipo_doc.as_deref())?)
        .raw("\">")
        .text(c.get(doc.proveedor, "numDoc", |p| p.num_doc.as_deref())?)
        .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[");
    let receiver = c.get(doc.proveedor, "rznSocial", |p| p.rzn_social.as_deref())?;
    if names.receiver_raw {
        out.cdata(receiver);
    } else {
        out.text(receiver);
    }
    out.raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:ReceiverParty><sac:")
        .raw(names.system_code)
        .raw(">")
        .text(doc.regimen)
        .raw("</sac:")
        .raw(names.system_code)
        .raw("><sac:")
        .raw(names.percent)
        .raw(">")
        .amount(doc.tasa)
        .raw("</sac:")
        .raw(names.percent)
        .raw(">");
    if truthy_str(doc.observacion) {
        out.raw("<cbc:Note><![CDATA[")
            .cdata(doc.observacion)
            .raw("]]></cbc:Note>");
    }
    out.raw("<cbc:TotalInvoiceAmount currencyID=\"PEN\">")
        .amount(doc.total)
        .raw("</cbc:TotalInvoiceAmount><sac:")
        .raw(names.total_paid)
        .raw(" currencyID=\"PEN\">")
        .amount(doc.paid)
        .raw("</sac:")
        .raw(names.total_paid)
        .raw(">");
    for (position, det) in details.iter().enumerate() {
        let count = det.payments.map_or(0, Vec::len);
        if count > 1 {
            return Err(XmlError::MultiplePayments {
                template,
                detail: position,
                count,
            });
        }
        out.raw("<sac:")
            .raw(names.reference)
            .raw("><cbc:ID schemeID=\"")
            .text(det.tipo_doc)
            .raw("\">")
            .text(det.num_doc)
            .raw("</cbc:ID><cbc:IssueDate>")
            .date(det.fecha_emision)?
            .raw("</cbc:IssueDate><cbc:TotalInvoiceAmount currencyID=\"")
            .text(det.moneda)
            .raw("\">")
            .amount(det.imp_total)
            .raw("</cbc:TotalInvoiceAmount>");
        if truthy_list(det.payments) {
            for (index, pay) in det.payments.into_iter().flatten().enumerate() {
                out.raw("<cac:Payment><cbc:ID>")
                    .index(index)
                    .raw("</cbc:ID><cbc:PaidAmount currencyID=\"")
                    .text(pay.moneda.as_deref())
                    .raw("\">")
                    .amount(pay.importe)
                    .raw("</cbc:PaidAmount><cbc:PaidDate>")
                    .date(pay.fecha.as_ref())?
                    .raw("</cbc:PaidDate></cac:Payment>");
            }
        }
        if truthy_num(det.amount) && truthy_num(det.net) && det.date.is_some() {
            out.raw("<sac:")
                .raw(names.information)
                .raw("><sac:")
                .raw(names.amount)
                .raw(" currencyID=\"PEN\">")
                .amount(det.amount)
                .raw("</sac:")
                .raw(names.amount)
                .raw("><sac:")
                .raw(names.date)
                .raw(">")
                .date(det.date)?
                .raw("</sac:")
                .raw(names.date)
                .raw("><sac:")
                .raw(names.net_total)
                .raw(" currencyID=\"PEN\">")
                .amount(det.net)
                .raw("</sac:")
                .raw(names.net_total)
                .raw(">");
            if let Some(exchange) = det.exchange {
                out.raw("<cac:ExchangeRate><cbc:SourceCurrencyCode>")
                    .text(exchange.moneda_ref.as_deref())
                    .raw("</cbc:SourceCurrencyCode><cbc:TargetCurrencyCode>")
                    .text(exchange.moneda_obj.as_deref())
                    .raw("</cbc:TargetCurrencyCode><cbc:CalculationRate>")
                    .fmt(exchange.factor, 6)
                    .raw("</cbc:CalculationRate><cbc:Date>")
                    .date(exchange.fecha.as_ref())?
                    .raw("</cbc:Date></cac:ExchangeRate>");
            }
            out.raw("</sac:").raw(names.information).raw(">");
        }
        out.raw("</sac:").raw(names.reference).raw(">");
    }
    out.raw("</").raw(names.root).raw(">");
    Ok(out.finish())
}
