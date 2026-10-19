//! `invoice2.0.xml.twig` y su fragmento `embededDespatch.xml.twig`.

use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::sale::{EmbededDespatch, Invoice, Legend, SaleDetail, SalePerception};

use super::shared::{self, IGV, ISC, OTROS, XML_DECLARATION};
use crate::format::{n_format, tributo_by_afectacion};
use crate::options::RenderOptions;
use crate::render::{Ctx, Out, Result, truthy_list, truthy_num, truthy_str};

pub(crate) const TEMPLATE: &str = "invoice2.0.xml.twig";
const EMBEDDED_TEMPLATE: &str = "embededDespatch.xml.twig";

pub(crate) fn render(doc: &Invoice, options: &RenderOptions) -> Result<String> {
    let mut out = Out::new(options, TEMPLATE);
    let c = out.ctx();
    let base = &doc.base;
    let currency = base.tipo_moneda.as_deref();
    out.raw(XML_DECLARATION)
        .raw(r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1">"#)
        .raw("<ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><sac:AdditionalInformation>");
    if truthy_num(doc.mto_descuentos) {
        additional_total(&mut out, "2005", currency, doc.mto_descuentos);
    }
    if truthy_num(base.mto_oper_gravadas) {
        additional_total(&mut out, "1001", currency, base.mto_oper_gravadas);
    }
    additional_total(&mut out, "1002", currency, base.mto_oper_inafectas);
    additional_total(&mut out, "1003", currency, base.mto_oper_exoneradas);
    if truthy_num(base.mto_oper_gratuitas) {
        additional_total(&mut out, "1004", currency, base.mto_oper_gratuitas);
    }
    if let Some(detr) = doc.detraccion.as_ref() {
        out.raw("<sac:AdditionalMonetaryTotal><cbc:ID>2003</cbc:ID>");
        if truthy_num(detr.value_ref) {
            out.raw("<cbc:ReferenceAmount currencyID=\"PEN\">")
                .amount(detr.value_ref)
                .raw("</cbc:ReferenceAmount>");
        }
        out.raw("<cbc:PayableAmount currencyID=\"PEN\">")
            .amount(detr.mount)
            .raw("</cbc:PayableAmount><cbc:Percent>")
            .amount(detr.percent)
            .raw("</cbc:Percent></sac:AdditionalMonetaryTotal>");
    }
    if let Some(perc) = doc.perception.as_ref() {
        perception_20(&mut out, perc);
    }
    legends_20(&mut out, base.legends.as_ref());
    if let Some(desp) = doc.guia_embebida.as_ref() {
        embedded_despatch(&mut out, desp, options)?;
    }
    if truthy_str(doc.tipo_operacion.as_deref()) {
        out.raw("<sac:SUNATTransaction><cbc:ID>")
            .text(doc.tipo_operacion.as_deref())
            .raw("</cbc:ID></sac:SUNATTransaction>");
    }
    out.raw("</sac:AdditionalInformation></ext:ExtensionContent></ext:UBLExtension><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions>")
        .raw("<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>1.0</cbc:CustomizationID><cbc:ID>")
        .text(base.serie.as_deref())
        .raw("-")
        .text(base.correlativo.as_deref())
        .raw("</cbc:ID><cbc:IssueDate>")
        .date(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueDate><cbc:IssueTime>")
        .time(base.fecha_emision.as_ref())?
        .raw("</cbc:IssueTime><cbc:InvoiceTypeCode>")
        .text(base.tipo_doc.as_deref())
        .raw("</cbc:InvoiceTypeCode><cbc:DocumentCurrencyCode>")
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
    let emp = base.company.as_ref();
    shared::signature(&mut out, emp)?;
    supplier_20(&mut out, emp, true)?;
    customer_20(
        &mut out,
        "AccountingCustomerParty",
        base.client.as_ref(),
        true,
    )?;
    if doc.seller.is_some() {
        customer_20(&mut out, "SellerSupplierParty", doc.seller.as_ref(), true)?;
    }
    if doc.fec_vencimiento.is_some() {
        out.raw(
            "<cac:PaymentMeans><cbc:PaymentMeansCode>-</cbc:PaymentMeansCode><cbc:PaymentDueDate>",
        )
        .date(doc.fec_vencimiento.as_ref())?
        .raw("</cbc:PaymentDueDate></cac:PaymentMeans>");
    }
    if truthy_list(doc.anticipos.as_ref()) {
        for ant in doc.anticipos.iter().flatten() {
            out.raw("<cac:PrepaidPayment><cbc:ID schemeID=\"")
                .text(ant.tipo_doc_rel.as_deref())
                .raw("\">")
                .text(ant.nro_doc_rel.as_deref())
                .raw("</cbc:ID><cbc:PaidAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(ant.total)
                .raw("</cbc:PaidAmount><cbc:InstructionID schemeID=\"6\">")
                .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
                .raw("</cbc:InstructionID></cac:PrepaidPayment>");
        }
    }
    if truthy_num(base.mto_isc) {
        shared::tax_total_20(&mut out, currency, &n_format(base.mto_isc, 2), &ISC);
    }
    shared::tax_total_20(
        &mut out,
        currency,
        &n_format(Some(base.mto_igv.unwrap_or(Decimal::ZERO)), 2),
        &IGV,
    );
    if truthy_num(base.sum_otros_cargos) {
        shared::tax_total_20(
            &mut out,
            currency,
            &n_format(base.sum_otros_cargos, 2),
            &OTROS,
        );
    }
    out.raw("<cac:LegalMonetaryTotal><cbc:AllowanceTotalAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(Some(doc.sum_dscto_global.unwrap_or(Decimal::ZERO)))
        .raw("</cbc:AllowanceTotalAmount><cbc:ChargeTotalAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(Some(base.mto_otros_tributos.unwrap_or(Decimal::ZERO)))
        .raw("</cbc:ChargeTotalAmount>");
    if doc.total_anticipos.is_some() {
        out.raw("<cbc:PrepaidAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(doc.total_anticipos)
            .raw("</cbc:PrepaidAmount>");
    }
    out.raw("<cbc:PayableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(base.mto_imp_venta)
        .raw("</cbc:PayableAmount></cac:LegalMonetaryTotal>");
    let kind = Line20 {
        line_tag: "InvoiceLine",
        quantity_tag: "InvoicedQuantity",
        invoice: true,
    };
    lines_20(&mut out, base.details.as_ref(), currency, &kind);
    out.raw("</Invoice>");
    Ok(out.finish())
}

/// `<sac:AdditionalMonetaryTotal>` con código e importe.
pub(crate) fn additional_total(
    out: &mut Out<'_>,
    id: &str,
    currency: Option<&str>,
    value: Option<Decimal>,
) {
    out.raw("<sac:AdditionalMonetaryTotal><cbc:ID>")
        .raw(id)
        .raw("</cbc:ID><cbc:PayableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(value)
        .raw("</cbc:PayableAmount></sac:AdditionalMonetaryTotal>");
}

/// Percepción en `sac:AdditionalInformation` (UBL 2.0).
pub(crate) fn perception_20(out: &mut Out<'_>, perc: &SalePerception) {
    out.raw("<sac:AdditionalMonetaryTotal><cbc:ID schemeID=\"")
        .text(perc.cod_reg.as_deref())
        .raw("\">2001</cbc:ID><sac:ReferenceAmount currencyID=\"PEN\">")
        .amount(perc.mto_base)
        .raw("</sac:ReferenceAmount><cbc:PayableAmount currencyID=\"PEN\">")
        .amount(perc.mto)
        .raw("</cbc:PayableAmount><sac:TotalAmount currencyID=\"PEN\">")
        .amount(perc.mto_total)
        .raw("</sac:TotalAmount></sac:AdditionalMonetaryTotal><sac:AdditionalProperty><cbc:ID>2000</cbc:ID><cbc:Value>COMPROBANTE DE PERCEPCION</cbc:Value></sac:AdditionalProperty>");
}

/// Leyendas como `sac:AdditionalProperty` (UBL 2.0).
pub(crate) fn legends_20(out: &mut Out<'_>, legends: Option<&Vec<Legend>>) {
    for leg in legends.into_iter().flatten() {
        out.raw("<sac:AdditionalProperty><cbc:ID>")
            .text(leg.code.as_deref())
            .raw("</cbc:ID><cbc:Value>")
            .text(leg.value.as_deref())
            .raw("</cbc:Value></sac:AdditionalProperty>");
    }
}

/// `{% include 'embededDespatch.xml.twig' with {'desp': …} only %}`.
fn embedded_despatch(
    out: &mut Out<'_>,
    desp: &EmbededDespatch,
    options: &RenderOptions,
) -> Result<()> {
    // El fragmento es otra plantilla: los errores de `strict_variables` la nombran.
    let c = Out::new(options, EMBEDDED_TEMPLATE).ctx();
    let llegada = desp.llegada.as_ref();
    let partida = desp.partida.as_ref();
    let transportista = desp.transportista.as_ref();
    out.raw("<sac:SUNATEmbededDespatchAdvice><cac:DeliveryAddress><cbc:ID>")
        .text(c.get(llegada, "ubigueo", |d| d.ubigueo.as_deref())?)
        .raw("</cbc:ID><cbc:StreetName>")
        .text(c.get(llegada, "direccion", |d| d.direccion.as_deref())?)
        .raw("</cbc:StreetName></cac:DeliveryAddress><cac:OriginAddress><cbc:ID>")
        .text(c.get(partida, "ubigueo", |d| d.ubigueo.as_deref())?)
        .raw("</cbc:ID><cbc:StreetName>")
        .text(c.get(partida, "direccion", |d| d.direccion.as_deref())?)
        .raw("</cbc:StreetName></cac:OriginAddress><sac:SUNATCarrierParty><cbc:CustomerAssignedAccountID>")
        .text(c.get(transportista, "numDoc", |t| t.num_doc.as_deref())?)
        .raw("</cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID>")
        .text(c.get(transportista, "tipoDoc", |t| t.tipo_doc.as_deref())?)
        .raw("</cbc:AdditionalAccountID><cac:Party><cac:PartyLegalEntity><cbc:RegistrationName>")
        .text(c.get(transportista, "rznSocial", |t| t.rzn_social.as_deref())?)
        .raw("</cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></sac:SUNATCarrierParty><sac:DriverParty><cac:Party><cac:PartyIdentification><cbc:ID>")
        .text(desp.nro_licencia.as_deref())
        .raw("</cbc:ID></cac:PartyIdentification></cac:Party></sac:DriverParty><sac:SUNATRoadTransport><cbc:LicensePlateID>")
        .text(desp.transp_placa.as_deref())
        .raw("</cbc:LicensePlateID><cbc:TransportAuthorizationCode>")
        .text(desp.transp_code_auth.as_deref())
        .raw("</cbc:TransportAuthorizationCode><cbc:BrandName>")
        .text(desp.transp_marca.as_deref())
        .raw("</cbc:BrandName></sac:SUNATRoadTransport><cbc:TransportModeCode>")
        .text(desp.mod_traslado.as_deref())
        .raw("</cbc:TransportModeCode><cbc:GrossWeightMeasure unitCode=\"")
        .text(desp.und_peso_bruto.as_deref())
        .raw("\">")
        .amount(desp.peso_bruto)
        .raw("</cbc:GrossWeightMeasure></sac:SUNATEmbededDespatchAdvice>");
    Ok(())
}

/// `cac:AccountingSupplierParty` de UBL 2.0. `contact`: solo la factura
/// emite el bloque de contacto.
pub(crate) fn supplier_20(out: &mut Out<'_>, emp: Option<&Company>, contact: bool) -> Result<()> {
    let c = out.ctx();
    out.raw("<cac:AccountingSupplierParty><cbc:CustomerAssignedAccountID>")
        .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
        .raw("</cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID>6</cbc:AdditionalAccountID><cac:Party><cac:PartyName><cbc:Name><![CDATA[")
        .cdata(c.get(emp, "nombreComercial", |e| e.nombre_comercial.as_deref())?)
        .raw("]]></cbc:Name></cac:PartyName>");
    let addr = c.obj(emp, "address", |e| e.address.as_ref())?;
    postal_address_20(out, c, addr)?;
    out.raw("<cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(emp, "razonSocial", |e| e.razon_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity>");
    if contact {
        shared::contact_company(out, emp)?;
    }
    out.raw("</cac:Party></cac:AccountingSupplierParty>");
    Ok(())
}

/// `cac:PostalAddress` completa del emisor (UBL 2.0, retención y percepción).
pub(crate) fn postal_address_20(
    out: &mut Out<'_>,
    c: Ctx,
    addr: Option<&cpeforge_core::model::company::Address>,
) -> Result<()> {
    out.raw("<cac:PostalAddress><cbc:ID>")
        .text(c.get(addr, "ubigueo", |a| a.ubigueo.as_deref())?)
        .raw("</cbc:ID><cbc:StreetName><![CDATA[")
        .cdata(c.get(addr, "direccion", |a| a.direccion.as_deref())?)
        .raw("]]></cbc:StreetName><cbc:CityName>")
        .text(c.get(addr, "departamento", |a| a.departamento.as_deref())?)
        .raw("</cbc:CityName><cbc:CountrySubentity>")
        .text(c.get(addr, "provincia", |a| a.provincia.as_deref())?)
        .raw("</cbc:CountrySubentity><cbc:District>")
        .text(c.get(addr, "distrito", |a| a.distrito.as_deref())?)
        .raw("</cbc:District><cac:Country><cbc:IdentificationCode>")
        .text(c.get(addr, "codigoPais", |a| a.codigo_pais.as_deref())?)
        .raw("</cbc:IdentificationCode></cac:Country></cac:PostalAddress>");
    Ok(())
}

/// Cliente o vendedor en UBL 2.0. `invoice`: la factura condiciona el ubigeo
/// y emite contacto; las notas no.
pub(crate) fn customer_20(
    out: &mut Out<'_>,
    tag: &str,
    party: Option<&Client>,
    invoice: bool,
) -> Result<()> {
    let c = out.ctx();
    out.raw("<cac:")
        .raw(tag)
        .raw("><cbc:CustomerAssignedAccountID>")
        .text(c.get(party, "numDoc", |p| p.num_doc.as_deref())?)
        .raw("</cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID>")
        .text(c.get(party, "tipoDoc", |p| p.tipo_doc.as_deref())?)
        .raw("</cbc:AdditionalAccountID><cac:Party>");
    let addr = c.obj(party, "address", |p| p.address.as_ref())?;
    if let Some(addr) = addr {
        out.raw("<cac:PostalAddress>");
        if !invoice || truthy_str(addr.ubigueo.as_deref()) {
            out.raw("<cbc:ID>")
                .text(addr.ubigueo.as_deref())
                .raw("</cbc:ID>");
        }
        out.raw("<cbc:StreetName><![CDATA[")
            .cdata(addr.direccion.as_deref())
            .raw("]]></cbc:StreetName><cac:Country><cbc:IdentificationCode>")
            .text(addr.codigo_pais.as_deref())
            .raw("</cbc:IdentificationCode></cac:Country></cac:PostalAddress>");
    }
    out.raw("<cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(party, "rznSocial", |p| p.rzn_social.as_deref())?)
        .raw("]]></cbc:RegistrationName></cac:PartyLegalEntity>");
    if invoice {
        shared::contact_client(out, party)?;
    }
    out.raw("</cac:Party></cac:").raw(tag).raw(">");
    Ok(())
}

/// Variación de las líneas UBL 2.0.
pub(crate) struct Line20 {
    pub(crate) line_tag: &'static str,
    pub(crate) quantity_tag: &'static str,
    /// La factura emite descuento por línea y el tributo según la afectación;
    /// las notas usan siempre IGV.
    pub(crate) invoice: bool,
}

/// Líneas de factura y notas UBL 2.0.
pub(crate) fn lines_20(
    out: &mut Out<'_>,
    details: Option<&Vec<SaleDetail>>,
    currency: Option<&str>,
    kind: &Line20,
) {
    for (index, detail) in details.into_iter().flatten().enumerate() {
        out.raw("<cac:")
            .raw(kind.line_tag)
            .raw("><cbc:ID>")
            .index(index)
            .raw("</cbc:ID><cbc:")
            .raw(kind.quantity_tag)
            .raw(" unitCode=\"")
            .text(detail.unidad.as_deref())
            .raw("\">")
            .num(detail.cantidad)
            .raw("</cbc:")
            .raw(kind.quantity_tag)
            .raw("><cbc:LineExtensionAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(detail.mto_valor_venta)
            .raw("</cbc:LineExtensionAmount><cac:PricingReference>");
        let (price, code) = if truthy_num(detail.mto_valor_gratuito) {
            (detail.mto_valor_gratuito, "02")
        } else {
            (detail.mto_precio_unitario, "01")
        };
        out.raw("<cac:AlternativeConditionPrice><cbc:PriceAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(price)
            .raw("</cbc:PriceAmount><cbc:PriceTypeCode>")
            .raw(code)
            .raw("</cbc:PriceTypeCode></cac:AlternativeConditionPrice></cac:PricingReference>");
        if kind.invoice && truthy_num(detail.descuento) {
            out.raw("<cac:AllowanceCharge><cbc:ChargeIndicator>false</cbc:ChargeIndicator><cbc:AllowanceChargeReasonCode>00</cbc:AllowanceChargeReasonCode><cbc:Amount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(detail.descuento)
                .raw("</cbc:Amount></cac:AllowanceCharge>");
        }
        if truthy_num(detail.isc) {
            let isc = n_format(detail.isc, 2);
            out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .text(Some(&isc))
                .raw("</cbc:TaxAmount><cac:TaxSubtotal><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .text(Some(&isc))
                .raw("</cbc:TaxAmount><cac:TaxCategory><cbc:TierRange>")
                .text(detail.tip_sis_isc.as_deref())
                .raw("</cbc:TierRange>");
            shared::tax_scheme(out, &ISC);
            out.raw("</cac:TaxCategory></cac:TaxSubtotal></cac:TaxTotal>");
        }
        if truthy_num(detail.igv) {
            let igv = n_format(detail.igv, 2);
            out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .text(Some(&igv))
                .raw("</cbc:TaxAmount><cac:TaxSubtotal><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .text(Some(&igv))
                .raw("</cbc:TaxAmount><cac:TaxCategory><cbc:TaxExemptionReasonCode>")
                .text(detail.tip_afe_igv.as_deref())
                .raw("</cbc:TaxExemptionReasonCode>");
            if kind.invoice {
                let afect = tributo_by_afectacion(detail.tip_afe_igv.as_deref());
                out.raw("<cac:TaxScheme><cbc:ID>")
                    .text(Some(afect.id))
                    .raw("</cbc:ID><cbc:Name>")
                    .text(Some(afect.name))
                    .raw("</cbc:Name><cbc:TaxTypeCode>")
                    .text(Some(afect.code))
                    .raw("</cbc:TaxTypeCode></cac:TaxScheme>");
            } else {
                shared::tax_scheme(out, &IGV);
            }
            out.raw("</cac:TaxCategory></cac:TaxSubtotal></cac:TaxTotal>");
        }
        out.raw("<cac:Item><cbc:Description><![CDATA[")
            .cdata(detail.descripcion.as_deref())
            .raw("]]></cbc:Description><cac:SellersItemIdentification><cbc:ID>")
            .text(detail.cod_producto.as_deref())
            .raw("</cbc:ID></cac:SellersItemIdentification>");
        if truthy_str(detail.cod_prod_sunat.as_deref()) {
            out.raw(r#"<cac:CommodityClassification><cbc:ItemClassificationCode listID="UNSPSC" listAgencyName="GS1 US" listName="Item Classification">"#)
                .text(detail.cod_prod_sunat.as_deref())
                .raw("</cbc:ItemClassificationCode></cac:CommodityClassification>");
        }
        out.raw("</cac:Item><cac:Price><cbc:PriceAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(detail.mto_valor_unitario)
            .raw("</cbc:PriceAmount></cac:Price></cac:")
            .raw(kind.line_tag)
            .raw(">");
    }
}
