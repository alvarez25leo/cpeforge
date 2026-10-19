//! `notacr2.0`, `notadb2.0`, `notacr2.1` y `notadb2.1.xml.twig`.

use cpeforge_core::Decimal;
use cpeforge_core::model::sale::Note;

use super::invoice20::{self, Line20};
use super::invoice21::{monetary, payment_terms_21};
use super::shared::{
    self, EMPTY_EXTENSIONS, EXO, EXP, GRA, ICBPER, IGV, INA, ISC, IVAP, Line21, OTROS, TaxAmount,
    XML_DECLARATION,
};
use crate::format::n_format;
use crate::options::RenderOptions;
use crate::render::{Out, Result, truthy_num, truthy_str};

/// Tipo de nota según la plantilla elegida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Credit,
    Debit,
}

impl Kind {
    fn root(self) -> &'static str {
        match self {
            Self::Credit => "CreditNote",
            Self::Debit => "DebitNote",
        }
    }

    fn line(self) -> (&'static str, &'static str) {
        match self {
            Self::Credit => ("CreditNoteLine", "CreditedQuantity"),
            Self::Debit => ("DebitNoteLine", "DebitedQuantity"),
        }
    }

    fn monetary_total(self) -> &'static str {
        match self {
            Self::Credit => "LegalMonetaryTotal",
            Self::Debit => "RequestedMonetaryTotal",
        }
    }

    pub(crate) fn template(self, version: &str) -> Option<&'static str> {
        match (self, version) {
            (Self::Credit, "2.0") => Some("notacr2.0.xml.twig"),
            (Self::Credit, "2.1") => Some("notacr2.1.xml.twig"),
            (Self::Debit, "2.0") => Some("notadb2.0.xml.twig"),
            (Self::Debit, "2.1") => Some("notadb2.1.xml.twig"),
            _ => None,
        }
    }
}

fn root_open(out: &mut Out<'_>, kind: Kind, sac: bool) {
    out.raw(XML_DECLARATION)
        .raw("<")
        .raw(kind.root())
        .raw(" xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:")
        .raw(kind.root())
        .raw(r#"-2" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2""#);
    if sac {
        out.raw(r#" xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1""#);
    }
    out.raw(">");
}

fn discrepancy(out: &mut Out<'_>, doc: &Note) {
    out.raw("<cac:DiscrepancyResponse><cbc:ReferenceID>")
        .text(doc.num_doc_afectado.as_deref())
        .raw("</cbc:ReferenceID><cbc:ResponseCode>")
        .text(doc.cod_motivo.as_deref())
        .raw("</cbc:ResponseCode><cbc:Description>")
        .text(doc.des_motivo.as_deref())
        .raw("</cbc:Description></cac:DiscrepancyResponse>");
}

fn billing_reference(out: &mut Out<'_>, doc: &Note) {
    out.raw("<cac:BillingReference><cac:InvoiceDocumentReference><cbc:ID>")
        .text(doc.num_doc_afectado.as_deref())
        .raw("</cbc:ID><cbc:DocumentTypeCode>")
        .text(doc.tip_doc_afectado.as_deref())
        .raw("</cbc:DocumentTypeCode></cac:InvoiceDocumentReference></cac:BillingReference>");
}

/// Notas UBL 2.1.
pub(crate) fn render_21(
    doc: &Note,
    kind: Kind,
    template: &'static str,
    options: &RenderOptions,
) -> Result<String> {
    let mut out = Out::new(options, template);
    let base = &doc.base;
    let currency = base.tipo_moneda.as_deref();
    root_open(&mut out, kind, false);
    out.raw(EMPTY_EXTENSIONS)
        .raw("<cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:CustomizationID>2.0</cbc:CustomizationID><cbc:ID>")
        .text(base.serie.as_deref())
        .raw("-")
        .text(base.correlativo.as_deref())
        .raw("</cbc:ID><cbc:IssueDate>")
        .date(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueDate><cbc:IssueTime>")
        .time(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueTime>");
    for leg in base.legends.iter().flatten() {
        out.raw("<cbc:Note languageLocaleID=\"")
            .text(leg.code.as_deref())
            .raw("\"><![CDATA[")
            .text(leg.value.as_deref())
            .raw("]]></cbc:Note>");
    }
    out.raw("<cbc:DocumentCurrencyCode>")
        .text(currency)
        .raw("</cbc:DocumentCurrencyCode>");
    discrepancy(&mut out, doc);
    if truthy_str(base.compra.as_deref()) {
        out.raw("<cac:OrderReference><cbc:ID>")
            .text(base.compra.as_deref())
            .raw("</cbc:ID></cac:OrderReference>");
    }
    billing_reference(&mut out, doc);
    shared::document_references(&mut out, "DespatchDocumentReference", base.guias.as_ref());
    shared::document_references(
        &mut out,
        "AdditionalDocumentReference",
        base.rel_docs.as_ref(),
    );
    let emp = base.company.as_ref();
    shared::signature(&mut out, emp)?;
    shared::supplier_21(&mut out, emp, false)?;
    shared::party_21(&mut out, "AccountingCustomerParty", base.client.as_ref())?;
    if kind == Kind::Credit {
        payment_terms_21(&mut out, base)?;
    }
    out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(base.total_impuestos)
        .raw("</cbc:TaxAmount>");
    let subtotals: [(
        Option<Decimal>,
        Option<Decimal>,
        TaxAmount<'_>,
        &shared::Scheme,
    ); 8] = [
        (
            base.mto_isc,
            base.mto_base_isc,
            TaxAmount::Amount(base.mto_isc),
            &ISC,
        ),
        (
            base.mto_oper_gravadas,
            base.mto_oper_gravadas,
            TaxAmount::Amount(base.mto_igv),
            &IGV,
        ),
        (
            base.mto_oper_inafectas,
            base.mto_oper_inafectas,
            TaxAmount::Literal("0"),
            &INA,
        ),
        (
            base.mto_oper_exoneradas,
            base.mto_oper_exoneradas,
            TaxAmount::Literal("0"),
            &EXO,
        ),
        (
            base.mto_oper_gratuitas,
            base.mto_oper_gratuitas,
            TaxAmount::Amount(base.mto_igv_gratuitas),
            &GRA,
        ),
        (
            base.mto_oper_exportacion,
            base.mto_oper_exportacion,
            TaxAmount::Literal("0"),
            &EXP,
        ),
        (
            base.mto_ivap,
            base.mto_base_ivap,
            TaxAmount::Amount(base.mto_ivap),
            &IVAP,
        ),
        (
            base.mto_otros_tributos,
            base.mto_base_oth,
            TaxAmount::Amount(base.mto_otros_tributos),
            &OTROS,
        ),
    ];
    for (condition, taxable, tax, scheme) in &subtotals {
        if truthy_num(*condition) {
            shared::tax_subtotal(&mut out, currency, *taxable, tax, scheme);
        }
    }
    if truthy_num(base.icbper) {
        out.raw("<cac:TaxSubtotal><cbc:TaxAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(base.icbper)
            .raw("</cbc:TaxAmount><cac:TaxCategory>");
        shared::tax_scheme(&mut out, &ICBPER);
        out.raw("</cac:TaxCategory></cac:TaxSubtotal>");
    }
    out.raw("</cac:TaxTotal><cac:")
        .raw(kind.monetary_total())
        .raw(">");
    if kind == Kind::Debit {
        monetary(&mut out, "LineExtensionAmount", currency, doc.valor_venta);
        monetary(&mut out, "TaxInclusiveAmount", currency, doc.sub_total);
    }
    monetary(
        &mut out,
        "ChargeTotalAmount",
        currency,
        base.sum_otros_cargos,
    );
    monetary(&mut out, "PayableRoundingAmount", currency, base.redondeo);
    out.raw("<cbc:PayableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(base.mto_imp_venta)
        .raw("</cbc:PayableAmount></cac:")
        .raw(kind.monetary_total())
        .raw(">");
    let (line_tag, quantity_tag) = kind.line();
    let lines = Line21 {
        line_tag,
        quantity_tag,
        charges: false,
        icbper_fixed_unit: false,
        attribute_value_not_null: false,
    };
    shared::lines_21(&mut out, base.details.as_ref(), currency, &lines)?;
    out.raw("</").raw(kind.root()).raw(">");
    Ok(out.finish())
}

/// Notas UBL 2.0.
pub(crate) fn render_20(
    doc: &Note,
    kind: Kind,
    template: &'static str,
    options: &RenderOptions,
) -> Result<String> {
    let mut out = Out::new(options, template);
    let base = &doc.base;
    let currency = base.tipo_moneda.as_deref();
    root_open(&mut out, kind, true);
    out.raw(
        "<ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><sac:AdditionalInformation>",
    );
    if truthy_num(base.mto_oper_gravadas) {
        invoice20::additional_total(&mut out, "1001", currency, base.mto_oper_gravadas);
    }
    if truthy_num(base.mto_oper_inafectas) {
        invoice20::additional_total(&mut out, "1002", currency, base.mto_oper_inafectas);
    }
    if truthy_num(base.mto_oper_exoneradas) {
        invoice20::additional_total(&mut out, "1003", currency, base.mto_oper_exoneradas);
    }
    if let Some(perc) = doc.perception.as_ref() {
        invoice20::perception_20(&mut out, perc);
    }
    if truthy_num(base.mto_oper_gratuitas) {
        invoice20::additional_total(&mut out, "1004", currency, base.mto_oper_gratuitas);
    }
    invoice20::legends_20(&mut out, base.legends.as_ref());
    out.raw("</sac:AdditionalInformation></ext:ExtensionContent></ext:UBLExtension><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions>")
        .raw("<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>1.0</cbc:CustomizationID><cbc:ID>")
        .text(base.serie.as_deref())
        .raw("-")
        .text(base.correlativo.as_deref())
        .raw("</cbc:ID><cbc:IssueDate>")
        .date(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueDate><cbc:IssueTime>")
        .time(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueTime><cbc:DocumentCurrencyCode>")
        .text(currency)
        .raw("</cbc:DocumentCurrencyCode>");
    discrepancy(&mut out, doc);
    billing_reference(&mut out, doc);
    if kind == Kind::Credit {
        shared::document_references(&mut out, "DespatchDocumentReference", base.guias.as_ref());
    }
    shared::document_references(
        &mut out,
        "AdditionalDocumentReference",
        base.rel_docs.as_ref(),
    );
    let emp = base.company.as_ref();
    shared::signature(&mut out, emp)?;
    invoice20::supplier_20(&mut out, emp, false)?;
    invoice20::customer_20(
        &mut out,
        "AccountingCustomerParty",
        base.client.as_ref(),
        false,
    )?;
    if truthy_num(base.mto_isc) {
        shared::tax_total_20(&mut out, currency, &n_format(base.mto_isc, 2), &ISC);
    }
    if truthy_num(base.mto_igv) {
        shared::tax_total_20(&mut out, currency, &n_format(base.mto_igv, 2), &IGV);
    }
    if truthy_num(base.sum_otros_cargos) {
        shared::tax_total_20(
            &mut out,
            currency,
            &n_format(base.sum_otros_cargos, 2),
            &OTROS,
        );
    }
    out.raw("<cac:")
        .raw(kind.monetary_total())
        .raw("><cbc:ChargeTotalAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(Some(base.mto_otros_tributos.unwrap_or(Decimal::ZERO)))
        .raw("</cbc:ChargeTotalAmount><cbc:PayableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(base.mto_imp_venta)
        .raw("</cbc:PayableAmount></cac:")
        .raw(kind.monetary_total())
        .raw(">");
    let (line_tag, quantity_tag) = kind.line();
    invoice20::lines_20(
        &mut out,
        base.details.as_ref(),
        currency,
        &Line20 {
            line_tag,
            quantity_tag,
            invoice: false,
        },
    );
    out.raw("</").raw(kind.root()).raw(">");
    Ok(out.finish())
}
