//! `summary.xml.twig` y `voided.xml.twig`.

use cpeforge_core::Zoned;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::VoidedDetail;

use super::shared::{self, EMPTY_EXTENSIONS, ICBPER, IGV, ISC, IVAP, OTROS, XML_DECLARATION};
use crate::format::n_format;
use crate::options::RenderOptions;
use crate::render::{Out, Result, truthy_num};

pub(crate) const SUMMARY_TEMPLATE: &str = "summary.xml.twig";
pub(crate) const VOIDED_TEMPLATE: &str = "voided.xml.twig";

/// Identificador, fechas, firma y emisor comunes a resumen y bajas.
fn header(
    out: &mut Out<'_>,
    xml_id: &str,
    generation: Option<&Zoned>,
    issue: Option<&Zoned>,
    emp: Option<&Company>,
) -> Result<()> {
    let c = out.ctx();
    out.raw("<cbc:ID>")
        .text(Some(xml_id))
        .raw("</cbc:ID><cbc:ReferenceDate>")
        .date(generation)?
        .raw("</cbc:ReferenceDate><cbc:IssueDate>")
        .date(issue)?
        .raw("</cbc:IssueDate>");
    shared::signature(out, emp)?;
    out.raw("<cac:AccountingSupplierParty><cbc:CustomerAssignedAccountID>")
        .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
        .raw("</cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID>6</cbc:AdditionalAccountID><cac:Party><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(emp, "razonSocial", |e| e.razon_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:AccountingSupplierParty>");
    Ok(())
}

pub(crate) fn render_summary(doc: &Summary, options: &RenderOptions) -> Result<String> {
    let mut out = Out::new(options, SUMMARY_TEMPLATE);
    let c = out.ctx();
    out.raw(XML_DECLARATION)
        .raw(r#"<SummaryDocuments xmlns="urn:sunat:names:specification:ubl:peru:schema:xsd:SummaryDocuments-1" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1">"#)
        .raw(EMPTY_EXTENSIONS)
        .raw("<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>1.1</cbc:CustomizationID>");
    let xml_id = doc.xml_id().map_err(|err| c.document(err))?;
    header(
        &mut out,
        &xml_id,
        doc.fec_generacion.as_ref(),
        doc.fec_resumen.as_ref(),
        doc.company.as_ref(),
    )?;
    let currency = doc.moneda.as_deref();
    for (index, det) in doc.details.iter().flatten().enumerate() {
        out.raw("<sac:SummaryDocumentsLine><cbc:LineID>")
            .index(index)
            .raw("</cbc:LineID><cbc:DocumentTypeCode>")
            .text(det.tipo_doc.as_deref())
            .raw("</cbc:DocumentTypeCode><cbc:ID>")
            .text(det.serie_nro.as_deref())
            .raw("</cbc:ID><cac:AccountingCustomerParty><cbc:CustomerAssignedAccountID>")
            .text(det.cliente_nro.as_deref())
            .raw("</cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID>")
            .text(det.cliente_tipo.as_deref())
            .raw("</cbc:AdditionalAccountID></cac:AccountingCustomerParty>");
        if let Some(reference) = det.doc_referencia.as_ref() {
            out.raw("<cac:BillingReference><cac:InvoiceDocumentReference><cbc:ID>")
                .text(reference.nro_doc.as_deref())
                .raw("</cbc:ID><cbc:DocumentTypeCode>")
                .text(reference.tipo_doc.as_deref())
                .raw(
                    "</cbc:DocumentTypeCode></cac:InvoiceDocumentReference></cac:BillingReference>",
                );
        }
        if let Some(perc) = det.percepcion.as_ref() {
            out.raw("<sac:SUNATPerceptionSummaryDocumentReference><sac:SUNATPerceptionSystemCode>")
                .text(perc.cod_reg.as_deref())
                .raw("</sac:SUNATPerceptionSystemCode><sac:SUNATPerceptionPercent>")
                .amount(perc.tasa)
                .raw("</sac:SUNATPerceptionPercent><cbc:TotalInvoiceAmount currencyID=\"PEN\">")
                .amount(perc.mto)
                .raw("</cbc:TotalInvoiceAmount><sac:SUNATTotalCashed currencyID=\"PEN\">")
                .amount(perc.mto_total)
                .raw("</sac:SUNATTotalCashed><cbc:TaxableAmount currencyID=\"PEN\">")
                .amount(perc.mto_base)
                .raw("</cbc:TaxableAmount></sac:SUNATPerceptionSummaryDocumentReference>");
        }
        out.raw("<cac:Status><cbc:ConditionCode>")
            .text(det.estado.as_deref())
            .raw("</cbc:ConditionCode></cac:Status><sac:TotalAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(det.total)
            .raw("</sac:TotalAmount>");
        if det.mto_oper_gravadas.is_some() {
            billing_payment(&mut out, currency, det.mto_oper_gravadas, "01");
        }
        for (value, code) in [
            (det.mto_oper_exoneradas, "02"),
            (det.mto_oper_inafectas, "03"),
            (det.mto_oper_exportacion, "04"),
            (det.mto_oper_gratuitas, "05"),
        ] {
            if truthy_num(value) {
                billing_payment(&mut out, currency, value, code);
            }
        }
        if truthy_num(det.mto_otros_cargos) {
            out.raw("<cac:AllowanceCharge><cbc:ChargeIndicator>true</cbc:ChargeIndicator><cbc:Amount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(det.mto_otros_cargos)
                .raw("</cbc:Amount></cac:AllowanceCharge>");
        }
        if truthy_num(det.mto_ivap) {
            shared::tax_total_20(&mut out, currency, &n_format(det.mto_ivap, 2), &IVAP);
        } else {
            let igv = n_format(det.mto_igv, 2);
            out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .text(Some(&igv))
                .raw("</cbc:TaxAmount><cac:TaxSubtotal><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .text(Some(&igv))
                .raw("</cbc:TaxAmount><cac:TaxCategory>");
            if det.porcentaje_igv.is_some() {
                out.raw("<cbc:Percent>")
                    .amount(det.porcentaje_igv)
                    .raw("</cbc:Percent>");
            }
            shared::tax_scheme(&mut out, &IGV);
            out.raw("</cac:TaxCategory></cac:TaxSubtotal></cac:TaxTotal>");
        }
        for (value, scheme) in [
            (det.mto_isc, &ISC),
            (det.mto_otros_tributos, &OTROS),
            (det.mto_icbper, &ICBPER),
        ] {
            if truthy_num(value) {
                shared::tax_total_20(&mut out, currency, &n_format(value, 2), scheme);
            }
        }
        out.raw("</sac:SummaryDocumentsLine>");
    }
    out.raw("</SummaryDocuments>");
    Ok(out.finish())
}

fn billing_payment(
    out: &mut Out<'_>,
    currency: Option<&str>,
    value: Option<cpeforge_core::Decimal>,
    code: &str,
) {
    out.raw("<sac:BillingPayment><cbc:PaidAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(value)
        .raw("</cbc:PaidAmount><cbc:InstructionID>")
        .raw(code)
        .raw("</cbc:InstructionID></sac:BillingPayment>");
}

/// Datos de una comunicación de baja o reversión (misma plantilla).
pub(crate) struct VoidedView<'a> {
    pub(crate) xml_id: std::result::Result<String, cpeforge_core::CoreError>,
    pub(crate) fec_generacion: Option<&'a Zoned>,
    pub(crate) fec_comunicacion: Option<&'a Zoned>,
    pub(crate) company: Option<&'a Company>,
    pub(crate) details: Option<&'a Vec<VoidedDetail>>,
}

pub(crate) fn render_voided(doc: VoidedView<'_>, options: &RenderOptions) -> Result<String> {
    let mut out = Out::new(options, VOIDED_TEMPLATE);
    let c = out.ctx();
    out.raw(XML_DECLARATION)
        .raw(r#"<VoidedDocuments xmlns="urn:sunat:names:specification:ubl:peru:schema:xsd:VoidedDocuments-1" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1" xmlns:ds="http://www.w3.org/2000/09/xmldsig#">"#)
        .raw(EMPTY_EXTENSIONS)
        .raw("<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>1.0</cbc:CustomizationID>");
    let xml_id = doc.xml_id.map_err(|err| c.document(err))?;
    header(
        &mut out,
        &xml_id,
        doc.fec_generacion,
        doc.fec_comunicacion,
        doc.company,
    )?;
    for (index, det) in doc.details.into_iter().flatten().enumerate() {
        out.raw("<sac:VoidedDocumentsLine><cbc:LineID>")
            .index(index)
            .raw("</cbc:LineID><cbc:DocumentTypeCode>")
            .text(det.tipo_doc.as_deref())
            .raw("</cbc:DocumentTypeCode><sac:DocumentSerialID>")
            .text(det.serie.as_deref())
            .raw("</sac:DocumentSerialID><sac:DocumentNumberID>")
            .text(det.correlativo.as_deref())
            .raw("</sac:DocumentNumberID><sac:VoidReasonDescription><![CDATA[")
            .cdata(det.des_motivo_baja.as_deref())
            .raw("]]></sac:VoidReasonDescription></sac:VoidedDocumentsLine>");
    }
    out.raw("</VoidedDocuments>");
    Ok(out.finish())
}
