//! `invoice2.1.xml.twig`: factura y boleta UBL 2.1.

use cpeforge_core::model::sale::{Charge, Invoice};

use super::shared::{
    self, EMPTY_EXTENSIONS, EXO, EXP, GRA, ICBPER, IGV, INA, ISC, IVAP, Line21, OTROS, TaxAmount,
    XML_DECLARATION,
};
use crate::options::RenderOptions;
use crate::render::{Out, Result, default_str, truthy_list, truthy_num, truthy_str};

pub(crate) const TEMPLATE: &str = "invoice2.1.xml.twig";

pub(crate) fn render(doc: &Invoice, options: &RenderOptions) -> Result<String> {
    let mut out = Out::new(options, TEMPLATE);
    let c = out.ctx();
    let base = &doc.base;
    let currency = base.tipo_moneda.as_deref();
    out.raw(XML_DECLARATION)
        .raw(r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2">"#)
        .raw(EMPTY_EXTENSIONS);
    let emp = base.company.as_ref();
    out.raw("<cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:CustomizationID>2.0</cbc:CustomizationID><cbc:ID>")
        .text(base.serie.as_deref())
        .raw("-")
        .text(base.correlativo.as_deref())
        .raw("</cbc:ID><cbc:IssueDate>")
        .date(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueDate><cbc:IssueTime>")
        .time(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueTime>");
    if doc.fec_vencimiento.is_some() {
        out.raw("<cbc:DueDate>")
            .date(doc.fec_vencimiento.as_ref())?
            .raw("</cbc:DueDate>");
    }
    out.raw("<cbc:InvoiceTypeCode listID=\"")
        .text(doc.tipo_operacion.as_deref())
        .raw("\">")
        .text(base.tipo_doc.as_deref())
        .raw("</cbc:InvoiceTypeCode>");
    for leg in base.legends.iter().flatten() {
        out.raw("<cbc:Note languageLocaleID=\"")
            .text(leg.code.as_deref())
            .raw("\"><![CDATA[")
            .text(leg.value.as_deref())
            .raw("]]></cbc:Note>");
    }
    if truthy_str(doc.observacion.as_deref()) {
        out.raw("<cbc:Note><![CDATA[")
            .text(doc.observacion.as_deref())
            .raw("]]></cbc:Note>");
    }
    out.raw("<cbc:DocumentCurrencyCode>")
        .text(currency)
        .raw("</cbc:DocumentCurrencyCode>");
    if truthy_str(base.compra.as_deref()) {
        out.raw("<cac:OrderReference><cbc:ID>")
            .text(base.compra.as_deref())
            .raw("</cbc:ID></cac:OrderReference>");
    }
    shared::document_references(&mut out, "DespatchDocumentReference", base.guias.as_ref());
    shared::document_references(
        &mut out,
        "AdditionalDocumentReference",
        base.rel_docs.as_ref(),
    );
    if truthy_list(doc.anticipos.as_ref()) {
        for (index, ant) in doc.anticipos.iter().flatten().enumerate() {
            out.raw("<cac:AdditionalDocumentReference><cbc:ID>")
                .text(ant.nro_doc_rel.as_deref())
                .raw("</cbc:ID><cbc:DocumentTypeCode>")
                .text(ant.tipo_doc_rel.as_deref())
                .raw("</cbc:DocumentTypeCode><cbc:DocumentStatusCode>")
                .index(index)
                .raw("</cbc:DocumentStatusCode><cac:IssuerParty><cac:PartyIdentification><cbc:ID schemeID=\"6\">")
                .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
                .raw("</cbc:ID></cac:PartyIdentification></cac:IssuerParty></cac:AdditionalDocumentReference>");
        }
    }
    shared::signature(&mut out, emp)?;
    shared::supplier_21(&mut out, emp, true)?;
    shared::party_21(&mut out, "AccountingCustomerParty", base.client.as_ref())?;
    if doc.seller.is_some() {
        shared::party_21(&mut out, "SellerSupplierParty", doc.seller.as_ref())?;
    }
    if let Some(addr) = doc.direccion_entrega.as_ref() {
        out.raw("<cac:Delivery><cac:DeliveryLocation><cac:Address><cbc:ID>")
            .text(addr.ubigueo.as_deref())
            .raw("</cbc:ID>");
        if truthy_str(addr.urbanizacion.as_deref()) {
            out.raw("<cbc:CitySubdivisionName>")
                .text(addr.urbanizacion.as_deref())
                .raw("</cbc:CitySubdivisionName>");
        }
        out.raw("<cbc:CityName>")
            .text(addr.provincia.as_deref())
            .raw("</cbc:CityName><cbc:CountrySubentity>")
            .text(addr.departamento.as_deref())
            .raw("</cbc:CountrySubentity><cbc:District>")
            .text(addr.distrito.as_deref())
            .raw("</cbc:District><cac:AddressLine><cbc:Line><![CDATA[")
            .cdata(addr.direccion.as_deref())
            .raw(r#"]]></cbc:Line></cac:AddressLine><cac:Country><cbc:IdentificationCode listID="ISO 3166-1" listAgencyName="United Nations Economic Commission for Europe" listName="Country">"#)
            .text(addr.codigo_pais.as_deref())
            .raw("</cbc:IdentificationCode></cac:Country></cac:Address></cac:DeliveryLocation></cac:Delivery>");
    }
    if let Some(detr) = doc.detraccion.as_ref() {
        out.raw("<cac:PaymentMeans><cbc:ID>Detraccion</cbc:ID><cbc:PaymentMeansCode>")
            .text(detr.cod_medio_pago.as_deref())
            .raw("</cbc:PaymentMeansCode><cac:PayeeFinancialAccount><cbc:ID>")
            .text(detr.cta_banco.as_deref())
            .raw("</cbc:ID></cac:PayeeFinancialAccount></cac:PaymentMeans><cac:PaymentTerms><cbc:ID>Detraccion</cbc:ID><cbc:PaymentMeansID>")
            .text(detr.cod_bien_detraccion.as_deref())
            .raw("</cbc:PaymentMeansID><cbc:PaymentPercent>")
            .num(detr.percent)
            .raw("</cbc:PaymentPercent><cbc:Amount currencyID=\"PEN\">")
            .amount(detr.mount)
            .raw("</cbc:Amount></cac:PaymentTerms>");
    }
    if let Some(perception) = doc.perception.as_ref() {
        out.raw("<cac:PaymentTerms><cbc:ID>Percepcion</cbc:ID><cbc:Amount currencyID=\"PEN\">")
            .amount(perception.mto_total)
            .raw("</cbc:Amount></cac:PaymentTerms>");
    }
    payment_terms_21(&mut out, &doc.base)?;
    if truthy_list(doc.anticipos.as_ref()) {
        for (index, ant) in doc.anticipos.iter().flatten().enumerate() {
            out.raw("<cac:PrepaidPayment><cbc:ID>")
                .index(index)
                .raw("</cbc:ID><cbc:PaidAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(ant.total)
                .raw("</cbc:PaidAmount></cac:PrepaidPayment>");
        }
    }
    global_charges(&mut out, doc.cargos.as_ref(), "true", currency, false);
    global_charges(&mut out, doc.descuentos.as_ref(), "false", currency, true);
    if let Some(perc) = doc.perception.as_ref() {
        out.raw("<cac:AllowanceCharge><cbc:ChargeIndicator>true</cbc:ChargeIndicator><cbc:AllowanceChargeReasonCode>")
            .text(perc.cod_reg.as_deref())
            .raw("</cbc:AllowanceChargeReasonCode><cbc:MultiplierFactorNumeric>")
            .limit(perc.porcentaje, 5)
            .raw("</cbc:MultiplierFactorNumeric><cbc:Amount currencyID=\"PEN\">")
            .amount(perc.mto)
            .raw("</cbc:Amount><cbc:BaseAmount currencyID=\"PEN\">")
            .amount(perc.mto_base)
            .raw("</cbc:BaseAmount></cac:AllowanceCharge>");
    }
    out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(base.total_impuestos)
        .raw("</cbc:TaxAmount>");
    if truthy_num(base.mto_isc) {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_base_isc,
            &TaxAmount::Amount(base.mto_isc),
            &ISC,
        );
    }
    if base.mto_oper_gravadas.is_some() {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_oper_gravadas,
            &TaxAmount::Amount(base.mto_igv),
            &IGV,
        );
    }
    if base.mto_oper_inafectas.is_some() {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_oper_inafectas,
            &TaxAmount::Literal("0"),
            &INA,
        );
    }
    if base.mto_oper_exoneradas.is_some() {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_oper_exoneradas,
            &TaxAmount::Literal("0"),
            &EXO,
        );
    }
    if base.mto_oper_gratuitas.is_some() {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_oper_gratuitas,
            &TaxAmount::Amount(base.mto_igv_gratuitas),
            &GRA,
        );
    }
    if base.mto_oper_exportacion.is_some() {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_oper_exportacion,
            &TaxAmount::Literal("0"),
            &EXP,
        );
    }
    if truthy_num(base.mto_ivap) {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_base_ivap,
            &TaxAmount::Amount(base.mto_ivap),
            &IVAP,
        );
    }
    if truthy_num(base.mto_otros_tributos) {
        shared::tax_subtotal(
            &mut out,
            currency,
            base.mto_base_oth,
            &TaxAmount::Amount(base.mto_otros_tributos),
            &OTROS,
        );
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
    out.raw("</cac:TaxTotal><cac:LegalMonetaryTotal><cbc:LineExtensionAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(doc.valor_venta)
        .raw("</cbc:LineExtensionAmount>");
    monetary(&mut out, "TaxInclusiveAmount", currency, doc.sub_total);
    monetary(
        &mut out,
        "AllowanceTotalAmount",
        currency,
        doc.sum_otros_descuentos,
    );
    monetary(
        &mut out,
        "ChargeTotalAmount",
        currency,
        base.sum_otros_cargos,
    );
    monetary(&mut out, "PrepaidAmount", currency, doc.total_anticipos);
    monetary(&mut out, "PayableRoundingAmount", currency, base.redondeo);
    out.raw("<cbc:PayableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(base.mto_imp_venta)
        .raw("</cbc:PayableAmount></cac:LegalMonetaryTotal>");
    let kind = Line21 {
        line_tag: "InvoiceLine",
        quantity_tag: "InvoicedQuantity",
        charges: true,
        icbper_fixed_unit: true,
        attribute_value_not_null: true,
    };
    shared::lines_21(&mut out, base.details.as_ref(), currency, &kind)?;
    out.raw("</Invoice>");
    Ok(out.finish())
}

/// `{% if x is not null %}<cbc:{tag} currencyID="…">{{ x|n_format }}</cbc:{tag}>{% endif %}`.
pub(crate) fn monetary(
    out: &mut Out<'_>,
    tag: &str,
    currency: Option<&str>,
    value: Option<cpeforge_core::Decimal>,
) {
    if value.is_some() {
        out.raw("<cbc:")
            .raw(tag)
            .raw(" currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(value)
            .raw("</cbc:")
            .raw(tag)
            .raw(">");
    }
}

/// Forma de pago y cuotas (factura 2.1 y nota de crédito 2.1).
pub(crate) fn payment_terms_21(
    out: &mut Out<'_>,
    base: &cpeforge_core::model::sale::BaseSale,
) -> Result<()> {
    let currency = base.tipo_moneda.as_deref();
    if let Some(forma) = base.forma_pago.as_ref() {
        out.raw("<cac:PaymentTerms><cbc:ID>FormaPago</cbc:ID><cbc:PaymentMeansID>")
            .text(forma.tipo.as_deref())
            .raw("</cbc:PaymentMeansID>");
        if truthy_num(forma.monto) {
            out.raw("<cbc:Amount currencyID=\"")
                .text(default_str(forma.moneda.as_deref(), currency))
                .raw("\">")
                .amount(forma.monto)
                .raw("</cbc:Amount>");
        }
        out.raw("</cac:PaymentTerms>");
    }
    if truthy_list(base.cuotas.as_ref()) {
        for (index, cuota) in base.cuotas.iter().flatten().enumerate() {
            out.raw("<cac:PaymentTerms><cbc:ID>FormaPago</cbc:ID><cbc:PaymentMeansID>Cuota")
                .text(Some(&format!("{:03}", index + 1)))
                .raw("</cbc:PaymentMeansID><cbc:Amount currencyID=\"")
                .text(default_str(cuota.moneda.as_deref(), currency))
                .raw("\">")
                .amount(cuota.monto)
                .raw("</cbc:Amount><cbc:PaymentDueDate>")
                .date(cuota.fecha_pago.as_ref())?
                .raw("</cbc:PaymentDueDate></cac:PaymentTerms>");
        }
    }
    Ok(())
}

/// Cargos o descuentos globales; los descuentos omiten el factor nulo.
fn global_charges(
    out: &mut Out<'_>,
    charges: Option<&Vec<Charge>>,
    indicator: &str,
    currency: Option<&str>,
    factor_optional: bool,
) {
    if truthy_list(charges) {
        for charge in charges.into_iter().flatten() {
            out.raw("<cac:AllowanceCharge><cbc:ChargeIndicator>")
                .raw(indicator)
                .raw("</cbc:ChargeIndicator><cbc:AllowanceChargeReasonCode>")
                .text(charge.cod_tipo.as_deref())
                .raw("</cbc:AllowanceChargeReasonCode>");
            if !factor_optional || charge.factor.is_some() {
                out.raw("<cbc:MultiplierFactorNumeric>")
                    .limit(charge.factor, 5)
                    .raw("</cbc:MultiplierFactorNumeric>");
            }
            out.raw("<cbc:Amount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(charge.monto)
                .raw("</cbc:Amount><cbc:BaseAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(charge.monto_base)
                .raw("</cbc:BaseAmount></cac:AllowanceCharge>");
        }
    }
}
