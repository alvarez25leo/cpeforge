//! Liquidación de compra electrónica: UBL 2.1 `SelfBilledInvoice` (anexo
//! N.º 9-A, literal e), de la R. S. 123-2022/SUNAT).
//!
//! Plantilla propia de CPEForge. El emisor (comprador) va en
//! `AccountingCustomerParty` y el vendedor en `AccountingSupplierParty`; el
//! lugar de la operación, en `DeliveryTerms`. Cada línea lleva el IGV y la
//! retención del impuesto a la renta (tributo `3000`), que también se
//! totaliza en la cabecera. El orden de los elementos es el del XSD.

use cpeforge_core::Decimal;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::settlement::{
    PurchaseSettlement, SETTLEMENT_TYPE, SettlementDetail, SettlementPlace, SettlementSeller,
};
use cpeforge_core::{CoreError, Zoned};

use super::shared::{
    self, EMPTY_EXTENSIONS, EXO, IGV, INA, Scheme, TaxAmount, XML_DECLARATION, tax_scheme,
};
use crate::format::tributo_by_afectacion;
use crate::options::RenderOptions;
use crate::render::{Out, Result, truthy_num, truthy_str};

pub(crate) const TEMPLATE: &str = "settlement.xml";

/// Retención del impuesto a la renta (catálogo 05).
const IR: Scheme = Scheme {
    id: "3000",
    name: "IR",
    code: "TOX",
};

const ROOT_OPEN: &str = r#"<SelfBilledInvoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:SelfBilledInvoice-2" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2">"#;

/// Partes obligatorias del documento; sin ellas el XML no sería válido.
struct Parties<'a> {
    company: &'a Company,
    seller: &'a SettlementSeller,
    place: &'a SettlementPlace,
}

fn required<'a, T>(out: &Out<'_>, value: Option<&'a T>, field: &'static str) -> Result<&'a T> {
    value.ok_or_else(|| {
        out.ctx().document(CoreError::MissingField {
            model: "PurchaseSettlement",
            field,
        })
    })
}

pub(crate) fn render(doc: &PurchaseSettlement, options: &RenderOptions) -> Result<String> {
    let mut out = Out::new(options, TEMPLATE);
    let parties = Parties {
        company: required(&out, doc.company.as_ref(), "company")?,
        seller: required(&out, doc.vendedor.as_ref(), "vendedor")?,
        place: required(&out, doc.lugar_operacion.as_ref(), "lugarOperacion")?,
    };
    let currency = doc.tipo_moneda.as_deref();
    out.raw(XML_DECLARATION)
        .raw(ROOT_OPEN)
        .raw(EMPTY_EXTENSIONS);
    header(&mut out, doc)?;
    for legend in doc.legends.iter().flatten() {
        out.raw("<cbc:Note languageLocaleID=\"")
            .text(legend.code.as_deref())
            .raw("\"><![CDATA[")
            .cdata(legend.value.as_deref())
            .raw("]]></cbc:Note>");
    }
    out.raw("<cbc:DocumentCurrencyCode>")
        .text(currency)
        .raw("</cbc:DocumentCurrencyCode>");
    shared::document_references(&mut out, "DespatchDocumentReference", doc.guias.as_ref());
    shared::document_references(
        &mut out,
        "AdditionalDocumentReference",
        doc.rel_docs.as_ref(),
    );
    shared::signature(&mut out, Some(parties.company))?;
    buyer(&mut out, parties.company);
    seller(&mut out, parties.seller);
    out.raw("<cac:DeliveryTerms><cac:DeliveryLocation><cbc:LocationTypeCode>")
        .text(parties.place.condicion.as_deref())
        .raw("</cbc:LocationTypeCode>");
    address(&mut out, "Address", parties.place.address.as_ref(), None);
    out.raw("</cac:DeliveryLocation></cac:DeliveryTerms>");
    tax_total(&mut out, doc, currency);
    monetary_total(&mut out, doc, currency);
    for (index, detail) in doc.details.iter().flatten().enumerate() {
        line(&mut out, index, detail, currency)?;
    }
    out.raw("</SelfBilledInvoice>");
    Ok(out.finish())
}

fn header(out: &mut Out<'_>, doc: &PurchaseSettlement) -> Result<()> {
    out.raw("<cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:CustomizationID>2.0</cbc:CustomizationID><cbc:ID>")
        .text(doc.serie.as_deref())
        .raw("-")
        .text(doc.correlativo.as_deref())
        .raw("</cbc:ID>");
    issue(out, doc.fecha_emision.as_ref())?;
    out.raw("<cbc:InvoiceTypeCode listID=\"")
        .text(doc.tipo_operacion.as_deref())
        .raw("\">")
        .raw(SETTLEMENT_TYPE)
        .raw("</cbc:InvoiceTypeCode>");
    Ok(())
}

fn issue(out: &mut Out<'_>, date: Option<&Zoned>) -> Result<()> {
    out.raw("<cbc:IssueDate>")
        .date(date)?
        .raw("</cbc:IssueDate><cbc:IssueTime>")
        .time(date)?
        .raw("</cbc:IssueTime>");
    Ok(())
}

/// Emisor electrónico (comprador), con RUC y domicilio fiscal.
fn buyer(out: &mut Out<'_>, company: &Company) {
    out.raw(
        "<cac:AccountingCustomerParty><cac:Party><cac:PartyIdentification><cbc:ID schemeID=\"6\">",
    )
    .text(company.ruc.as_deref())
    .raw("</cbc:ID></cac:PartyIdentification>");
    if truthy_str(company.nombre_comercial.as_deref()) {
        out.raw("<cac:PartyName><cbc:Name><![CDATA[")
            .cdata(company.nombre_comercial.as_deref())
            .raw("]]></cbc:Name></cac:PartyName>");
    }
    out.raw("<cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(company.razon_social.as_deref())
        .raw("]]></cbc:RegistrationName>");
    let addr = company.address.as_ref();
    address(
        out,
        "RegistrationAddress",
        addr,
        addr.and_then(|a| a.cod_local.as_deref()),
    );
    out.raw("</cac:PartyLegalEntity></cac:Party></cac:AccountingCustomerParty>");
}

/// Vendedor: documento de identidad, nombres y domicilio con su condición
/// (catálogo 60) en `AddressTypeCode`.
fn seller(out: &mut Out<'_>, seller: &SettlementSeller) {
    out.raw("<cac:AccountingSupplierParty><cac:Party><cac:PartyIdentification><cbc:ID schemeID=\"")
        .text(seller.tipo_doc.as_deref())
        .raw("\">")
        .text(seller.num_doc.as_deref())
        .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(seller.rzn_social.as_deref())
        .raw("]]></cbc:RegistrationName>");
    address(
        out,
        "RegistrationAddress",
        seller.address.as_ref(),
        seller.condicion.as_deref(),
    );
    out.raw("</cac:PartyLegalEntity></cac:Party></cac:AccountingSupplierParty>");
}

/// Dirección UBL 2.1 en el orden de `AddressType`; los elementos vacíos se
/// omiten.
fn address(out: &mut Out<'_>, tag: &str, addr: Option<&Address>, type_code: Option<&str>) {
    let Some(addr) = addr else {
        return;
    };
    out.raw("<cac:").raw(tag).raw(">");
    optional(out, "ID", addr.ubigueo.as_deref());
    optional(out, "AddressTypeCode", type_code);
    optional(out, "CitySubdivisionName", addr.urbanizacion.as_deref());
    optional(out, "CityName", addr.provincia.as_deref());
    optional(out, "CountrySubentity", addr.departamento.as_deref());
    optional(out, "District", addr.distrito.as_deref());
    out.raw("<cac:AddressLine><cbc:Line><![CDATA[")
        .cdata(addr.direccion.as_deref())
        .raw("]]></cbc:Line></cac:AddressLine><cac:Country><cbc:IdentificationCode>")
        .text(addr.codigo_pais.as_deref().or(Some("PE")))
        .raw("</cbc:IdentificationCode></cac:Country></cac:")
        .raw(tag)
        .raw(">");
}

fn optional(out: &mut Out<'_>, tag: &str, value: Option<&str>) {
    if truthy_str(value) {
        out.raw("<cbc:")
            .raw(tag)
            .raw(">")
            .text(value)
            .raw("</cbc:")
            .raw(tag)
            .raw(">");
    }
}

/// Tributos de la cabecera: el `TaxAmount` total es el IGV; la renta
/// retenida va como subtotal propio.
fn tax_total(out: &mut Out<'_>, doc: &PurchaseSettlement, currency: Option<&str>) {
    out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(doc.total_impuestos)
        .raw("</cbc:TaxAmount>");
    if doc.mto_oper_gravadas.is_some() {
        shared::tax_subtotal(
            out,
            currency,
            doc.mto_oper_gravadas,
            &TaxAmount::Amount(doc.mto_igv),
            &IGV,
        );
    }
    if truthy_num(doc.mto_oper_exoneradas) {
        shared::tax_subtotal(
            out,
            currency,
            doc.mto_oper_exoneradas,
            &TaxAmount::Literal("0"),
            &EXO,
        );
    }
    if truthy_num(doc.mto_oper_inafectas) {
        shared::tax_subtotal(
            out,
            currency,
            doc.mto_oper_inafectas,
            &TaxAmount::Literal("0"),
            &INA,
        );
    }
    shared::tax_subtotal(
        out,
        currency,
        doc.mto_base_ir,
        &TaxAmount::Amount(doc.mto_ir),
        &IR,
    );
    out.raw("</cac:TaxTotal>");
}

fn monetary_total(out: &mut Out<'_>, doc: &PurchaseSettlement, currency: Option<&str>) {
    let tag = |out: &mut Out<'_>, name: &str, value: Option<Decimal>| {
        out.raw("<cbc:")
            .raw(name)
            .raw(" currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(value)
            .raw("</cbc:")
            .raw(name)
            .raw(">");
    };
    out.raw("<cac:LegalMonetaryTotal>");
    tag(out, "LineExtensionAmount", doc.valor_venta);
    tag(out, "TaxInclusiveAmount", doc.sub_total);
    if truthy_num(doc.redondeo) {
        tag(out, "PayableRoundingAmount", doc.redondeo);
    }
    tag(out, "PayableAmount", doc.mto_imp_venta);
    out.raw("</cac:LegalMonetaryTotal>");
}

fn line(
    out: &mut Out<'_>,
    index: usize,
    detail: &SettlementDetail,
    currency: Option<&str>,
) -> Result<()> {
    out.raw("<cac:InvoiceLine><cbc:ID>")
        .index(index)
        .raw("</cbc:ID><cbc:InvoicedQuantity unitCode=\"")
        .text(detail.unidad.as_deref())
        .raw("\">")
        .num(detail.cantidad)
        .raw("</cbc:InvoicedQuantity><cbc:LineExtensionAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(detail.mto_valor_venta)
        .raw("</cbc:LineExtensionAmount><cac:PricingReference><cac:AlternativeConditionPrice><cbc:PriceAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .limit(detail.mto_precio_unitario, 10)
        .raw("</cbc:PriceAmount><cbc:PriceTypeCode>01</cbc:PriceTypeCode></cac:AlternativeConditionPrice></cac:PricingReference><cac:TaxTotal><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(detail.total_impuestos)
        .raw("</cbc:TaxAmount><cac:TaxSubtotal><cbc:TaxableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(detail.mto_base_igv)
        .raw("</cbc:TaxableAmount><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(detail.igv)
        .raw("</cbc:TaxAmount><cac:TaxCategory><cbc:Percent>")
        .num(detail.porcentaje_igv)
        .raw("</cbc:Percent><cbc:TaxExemptionReasonCode>")
        .text(detail.tip_afe_igv.as_deref())
        .raw("</cbc:TaxExemptionReasonCode>");
    let afectacion = tributo_by_afectacion(detail.tip_afe_igv.as_deref());
    out.raw("<cac:TaxScheme><cbc:ID>")
        .text(Some(afectacion.id))
        .raw("</cbc:ID><cbc:Name>")
        .text(Some(afectacion.name))
        .raw("</cbc:Name><cbc:TaxTypeCode>")
        .text(Some(afectacion.code))
        .raw("</cbc:TaxTypeCode></cac:TaxScheme></cac:TaxCategory></cac:TaxSubtotal><cac:TaxSubtotal><cbc:TaxableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(detail.mto_base_ir)
        .raw("</cbc:TaxableAmount><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(detail.ir)
        .raw("</cbc:TaxAmount><cac:TaxCategory><cbc:Percent>")
        .num(detail.porcentaje_ir)
        .raw("</cbc:Percent>");
    tax_scheme(out, &IR);
    out.raw(
        "</cac:TaxCategory></cac:TaxSubtotal></cac:TaxTotal><cac:Item><cbc:Description><![CDATA[",
    )
    .cdata(detail.descripcion.as_deref())
    .raw("]]></cbc:Description>");
    if truthy_str(detail.cod_producto.as_deref()) {
        out.raw("<cac:SellersItemIdentification><cbc:ID>")
            .text(detail.cod_producto.as_deref())
            .raw("</cbc:ID></cac:SellersItemIdentification>");
    }
    if truthy_str(detail.cod_prod_sunat.as_deref()) {
        out.raw("<cac:CommodityClassification><cbc:ItemClassificationCode>")
            .text(detail.cod_prod_sunat.as_deref())
            .raw("</cbc:ItemClassificationCode></cac:CommodityClassification>");
    }
    for attribute in detail.atributos.iter().flatten() {
        shared::item_property_21(out, attribute, false)?;
    }
    out.raw("</cac:Item><cac:Price><cbc:PriceAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .limit(detail.mto_valor_unitario, 10)
        .raw("</cbc:PriceAmount></cac:Price></cac:InvoiceLine>");
    Ok(())
}
