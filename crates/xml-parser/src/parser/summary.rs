//! Resumen diario de boletas (`SummaryParser`).

use cpeforge_core::model::company::Company;
use cpeforge_core::model::sale::RelatedDocument;
use cpeforge_core::model::summary::{Summary, SummaryDetail, SummaryPerception};

use crate::convert::{self, DateContext, explode_part, loose_eq, to_float};
use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::Lookup;

super::parser!(SummaryParser, Summary, "Summary");

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<Summary, XmlParserError> {
    let root = Some(xml.root());
    let id = xml.value("cbc:ID", root)?;
    let fec_generacion = dates.parse(&xml.value("cbc:ReferenceDate", root)?)?;
    let fec_resumen = dates.parse(&xml.value("cbc:IssueDate", root)?)?;
    let node = xml.node("cac:AccountingSupplierParty", root)?;
    let company = Company {
        ruc: Some(xml.value("cbc:CustomerAssignedAccountID", node)?),
        nombre_comercial: Some(xml.value("cac:Party/cac:PartyName/cbc:Name", node)?),
        razon_social: Some(xml.value("cac:Party/cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        ..Company::default()
    };
    let mut details = Vec::new();
    for line in xml.nodes("sac:SummaryDocumentsLine", root)? {
        details.push(detail(xml, line)?);
    }
    Ok(Summary {
        correlativo: explode_part(&id, 2),
        fec_generacion: Some(fec_generacion),
        fec_resumen: Some(fec_resumen),
        company: Some(company),
        details: Some(details),
        ..Summary::default()
    })
}

fn float<'a>(
    xml: &Lookup<'a>,
    query: &str,
    context: Option<Node<'a>>,
) -> Result<rust_decimal::Decimal, XmlParserError> {
    to_float(&xml.value_def(query, context, "0")?)
}

fn trimmed<'a>(
    xml: &Lookup<'a>,
    query: &str,
    context: Option<Node<'a>>,
) -> Result<String, XmlParserError> {
    Ok(convert::trim(&xml.value(query, context)?).to_owned())
}

fn detail<'a>(xml: &Lookup<'a>, node: Node<'a>) -> Result<SummaryDetail, XmlParserError> {
    let at = Some(node);
    let mut det = SummaryDetail {
        tipo_doc: Some(xml.value("cbc:DocumentTypeCode", at)?),
        serie_nro: Some(xml.value("cbc:ID", at)?),
        estado: Some(trimmed(xml, "cac:Status/cbc:ConditionCode", at)?),
        cliente_tipo: Some(trimmed(
            xml,
            "cac:AccountingCustomerParty/cbc:AdditionalAccountID",
            at,
        )?),
        cliente_nro: Some(trimmed(
            xml,
            "cac:AccountingCustomerParty/cbc:CustomerAssignedAccountID",
            at,
        )?),
        total: Some(float(xml, "sac:TotalAmount", at)?),
        mto_otros_cargos: Some(float(xml, "cac:AllowanceCharge/cbc:Amount", at)?),
        ..SummaryDetail::default()
    };
    if let Some(reference) = xml.node("cac:BillingReference", at)? {
        let r = Some(reference);
        det.doc_referencia = Some(RelatedDocument {
            tipo_doc: Some(trimmed(
                xml,
                "cac:InvoiceDocumentReference/cbc:DocumentTypeCode",
                r,
            )?),
            nro_doc: Some(trimmed(xml, "cac:InvoiceDocumentReference/cbc:ID", r)?),
        });
    }
    // loadPerception()
    if let Some(reference) = xml.node("sac:SUNATPerceptionSummaryDocumentReference", at)? {
        let r = Some(reference);
        det.percepcion = Some(SummaryPerception {
            cod_reg: Some(trimmed(xml, "sac:SUNATPerceptionSystemCode", r)?),
            tasa: Some(float(xml, "sac:SUNATPerceptionPercent", r)?),
            mto: Some(float(xml, "sac:TotalInvoiceAmount", r)?),
            mto_total: Some(float(xml, "sac:SUNATTotalCashed", r)?),
            mto_base: Some(float(xml, "sac:TaxableAmount", r)?),
        });
    }
    // loadTotals()
    for total in xml.nodes("sac:BillingPayment", at)? {
        let t = Some(total);
        let id = trimmed(xml, "cbc:InstructionID", t)?;
        let val = float(xml, "cbc:PaidAmount", t)?;
        if loose_eq(&id, "01") {
            det.mto_oper_gravadas = Some(val);
        } else if loose_eq(&id, "02") {
            det.mto_oper_exoneradas = Some(val);
        } else if loose_eq(&id, "03") {
            det.mto_oper_inafectas = Some(val);
        } else if loose_eq(&id, "05") {
            det.mto_oper_gratuitas = Some(val);
        }
    }
    // loadTaxs()
    for tax in xml.nodes("cac:TaxTotal", at)? {
        let t = Some(tax);
        let name = trimmed(
            xml,
            "cac:TaxSubtotal/cac:TaxCategory/cac:TaxScheme/cbc:Name",
            t,
        )?;
        let val = float(xml, "cbc:TaxAmount", t)?;
        if loose_eq(&name, "IGV") {
            det.mto_igv = Some(val);
        } else if loose_eq(&name, "ISC") {
            det.mto_isc = Some(val);
        } else if loose_eq(&name, "OTROS") {
            det.mto_otros_tributos = Some(val);
        }
    }
    Ok(det)
}
