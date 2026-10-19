//! Fragmentos repetidos entre plantillas.
//!
//! Cada función reproduce un bloque idéntico de las plantillas, con las
//! mismas lecturas de atributos y en el mismo orden (importa para el error de
//! `strict_variables`).

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{Charge, DetailAttribute, RelatedDocument, SaleDetail};

use crate::format::tributo_by_afectacion;
use crate::render::{Out, Result, truthy_list, truthy_num, truthy_str};

pub(crate) const XML_DECLARATION: &str = r#"<?xml version="1.0" encoding="utf-8"?>"#;

pub(crate) const EMPTY_EXTENSIONS: &str = "<ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions>";

/// Bloque `cac:Signature` (todas las plantillas).
pub(crate) fn signature(out: &mut Out<'_>, emp: Option<&Company>) -> Result<()> {
    let c = out.ctx();
    out.raw("<cac:Signature><cbc:ID>SIGN")
        .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
        .raw("</cbc:ID><cac:SignatoryParty><cac:PartyIdentification><cbc:ID>")
        .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
        .raw("</cbc:ID></cac:PartyIdentification><cac:PartyName><cbc:Name><![CDATA[")
        .cdata(c.get(emp, "razonSocial", |e| e.razon_social.as_deref())?)
        .raw("]]></cbc:Name></cac:PartyName></cac:SignatoryParty><cac:DigitalSignatureAttachment><cac:ExternalReference><cbc:URI>#CPEFORGE-SIGN</cbc:URI></cac:ExternalReference></cac:DigitalSignatureAttachment></cac:Signature>");
    Ok(())
}

/// `{% for x in lista %}<cac:{tag}><cbc:ID>{{ x.nroDoc }}</cbc:ID><cbc:DocumentTypeCode>{{ x.tipoDoc }}…`
pub(crate) fn document_references(
    out: &mut Out<'_>,
    tag: &str,
    docs: Option<&Vec<RelatedDocument>>,
) {
    if truthy_list(docs) {
        for doc in docs.into_iter().flatten() {
            out.raw("<cac:")
                .raw(tag)
                .raw("><cbc:ID>")
                .text(doc.nro_doc.as_deref())
                .raw("</cbc:ID><cbc:DocumentTypeCode>")
                .text(doc.tipo_doc.as_deref())
                .raw("</cbc:DocumentTypeCode></cac:")
                .raw(tag)
                .raw(">");
        }
    }
}

/// Bloque `cac:Contact` de emisor, cliente o vendedor.
pub(crate) fn contact_company(out: &mut Out<'_>, emp: Option<&Company>) -> Result<()> {
    let c = out.ctx();
    let email = c.get(emp, "email", |e| e.email.as_deref())?;
    let has_contact =
        truthy_str(email) || truthy_str(c.get(emp, "telephone", |e| e.telephone.as_deref())?);
    if has_contact {
        out.raw("<cac:Contact>");
        let telephone = c.get(emp, "telephone", |e| e.telephone.as_deref())?;
        if truthy_str(telephone) {
            out.raw("<cbc:Telephone>")
                .text(telephone)
                .raw("</cbc:Telephone>");
        }
        let email = c.get(emp, "email", |e| e.email.as_deref())?;
        if truthy_str(email) {
            out.raw("<cbc:ElectronicMail>")
                .text(email)
                .raw("</cbc:ElectronicMail>");
        }
        out.raw("</cac:Contact>");
    }
    Ok(())
}

/// Bloque `cac:Contact` de un cliente.
pub(crate) fn contact_client(out: &mut Out<'_>, client: Option<&Client>) -> Result<()> {
    let c = out.ctx();
    let email = c.get(client, "email", |e| e.email.as_deref())?;
    let has_contact =
        truthy_str(email) || truthy_str(c.get(client, "telephone", |e| e.telephone.as_deref())?);
    if has_contact {
        out.raw("<cac:Contact>");
        let telephone = c.get(client, "telephone", |e| e.telephone.as_deref())?;
        if truthy_str(telephone) {
            out.raw("<cbc:Telephone>")
                .text(telephone)
                .raw("</cbc:Telephone>");
        }
        let email = c.get(client, "email", |e| e.email.as_deref())?;
        if truthy_str(email) {
            out.raw("<cbc:ElectronicMail>")
                .text(email)
                .raw("</cbc:ElectronicMail>");
        }
        out.raw("</cac:Contact>");
    }
    Ok(())
}

/// `cac:RegistrationAddress` del emisor en UBL 2.1.
pub(crate) fn registration_address_21(out: &mut Out<'_>, addr: Option<&Address>) -> Result<()> {
    let c = out.ctx();
    out.raw("<cac:RegistrationAddress><cbc:ID>")
        .text(c.get(addr, "ubigueo", |a| a.ubigueo.as_deref())?)
        .raw("</cbc:ID><cbc:AddressTypeCode>")
        .text(c.get(addr, "codLocal", |a| a.cod_local.as_deref())?)
        .raw("</cbc:AddressTypeCode>");
    let urbanizacion = c.get(addr, "urbanizacion", |a| a.urbanizacion.as_deref())?;
    if truthy_str(urbanizacion) {
        out.raw("<cbc:CitySubdivisionName>")
            .text(urbanizacion)
            .raw("</cbc:CitySubdivisionName>");
    }
    out.raw("<cbc:CityName>")
        .text(c.get(addr, "provincia", |a| a.provincia.as_deref())?)
        .raw("</cbc:CityName><cbc:CountrySubentity>")
        .text(c.get(addr, "departamento", |a| a.departamento.as_deref())?)
        .raw("</cbc:CountrySubentity><cbc:District>")
        .text(c.get(addr, "distrito", |a| a.distrito.as_deref())?)
        .raw("</cbc:District><cac:AddressLine><cbc:Line><![CDATA[")
        .cdata(c.get(addr, "direccion", |a| a.direccion.as_deref())?)
        .raw("]]></cbc:Line></cac:AddressLine><cac:Country><cbc:IdentificationCode>")
        .text(c.get(addr, "codigoPais", |a| a.codigo_pais.as_deref())?)
        .raw("</cbc:IdentificationCode></cac:Country></cac:RegistrationAddress>");
    Ok(())
}

/// `cac:AccountingSupplierParty` en UBL 2.1 (factura y notas).
///
/// `optional_trade_name`: la factura solo emite `cac:PartyName` si
/// `nombreComercial` es verdadero; las notas lo emiten siempre.
pub(crate) fn supplier_21(
    out: &mut Out<'_>,
    emp: Option<&Company>,
    optional_trade_name: bool,
) -> Result<()> {
    let c = out.ctx();
    out.raw(
        "<cac:AccountingSupplierParty><cac:Party><cac:PartyIdentification><cbc:ID schemeID=\"6\">",
    )
    .text(c.get(emp, "ruc", |e| e.ruc.as_deref())?)
    .raw("</cbc:ID></cac:PartyIdentification>");
    let trade_name = c.get(emp, "nombreComercial", |e| e.nombre_comercial.as_deref())?;
    if !optional_trade_name || truthy_str(trade_name) {
        out.raw("<cac:PartyName><cbc:Name><![CDATA[")
            .cdata(c.get(emp, "nombreComercial", |e| e.nombre_comercial.as_deref())?)
            .raw("]]></cbc:Name></cac:PartyName>");
    }
    out.raw("<cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(emp, "razonSocial", |e| e.razon_social.as_deref())?)
        .raw("]]></cbc:RegistrationName>");
    let addr = c.obj(emp, "address", |e| e.address.as_ref())?;
    registration_address_21(out, addr)?;
    out.raw("</cac:PartyLegalEntity>");
    contact_company(out, emp)?;
    out.raw("</cac:Party></cac:AccountingSupplierParty>");
    Ok(())
}

/// Parte con identificación, razón social, dirección opcional y contacto en
/// UBL 2.1 (`AccountingCustomerParty` o `SellerSupplierParty`).
pub(crate) fn party_21(out: &mut Out<'_>, tag: &str, party: Option<&Client>) -> Result<()> {
    let c = out.ctx();
    out.raw("<cac:")
        .raw(tag)
        .raw("><cac:Party><cac:PartyIdentification><cbc:ID schemeID=\"")
        .text(c.get(party, "tipoDoc", |p| p.tipo_doc.as_deref())?)
        .raw("\">")
        .text(c.get(party, "numDoc", |p| p.num_doc.as_deref())?)
        .raw("</cbc:ID></cac:PartyIdentification><cac:PartyLegalEntity><cbc:RegistrationName><![CDATA[")
        .cdata(c.get(party, "rznSocial", |p| p.rzn_social.as_deref())?)
        .raw("]]></cbc:RegistrationName>");
    let addr = c.obj(party, "address", |p| p.address.as_ref())?;
    if let Some(addr) = addr {
        out.raw("<cac:RegistrationAddress>");
        if truthy_str(addr.ubigueo.as_deref()) {
            out.raw("<cbc:ID>")
                .text(addr.ubigueo.as_deref())
                .raw("</cbc:ID>");
        }
        out.raw("<cac:AddressLine><cbc:Line><![CDATA[")
            .cdata(addr.direccion.as_deref())
            .raw("]]></cbc:Line></cac:AddressLine><cac:Country><cbc:IdentificationCode>")
            .text(addr.codigo_pais.as_deref())
            .raw("</cbc:IdentificationCode></cac:Country></cac:RegistrationAddress>");
    }
    out.raw("</cac:PartyLegalEntity>");
    contact_client(out, party)?;
    out.raw("</cac:Party></cac:").raw(tag).raw(">");
    Ok(())
}

/// Un `cac:TaxSubtotal` de cabecera con base, impuesto y esquema fijo.
pub(crate) struct Scheme {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) code: &'static str,
}

pub(crate) const ISC: Scheme = Scheme {
    id: "2000",
    name: "ISC",
    code: "EXC",
};
pub(crate) const IGV: Scheme = Scheme {
    id: "1000",
    name: "IGV",
    code: "VAT",
};
pub(crate) const INA: Scheme = Scheme {
    id: "9998",
    name: "INA",
    code: "FRE",
};
pub(crate) const EXO: Scheme = Scheme {
    id: "9997",
    name: "EXO",
    code: "VAT",
};
pub(crate) const GRA: Scheme = Scheme {
    id: "9996",
    name: "GRA",
    code: "FRE",
};
pub(crate) const EXP: Scheme = Scheme {
    id: "9995",
    name: "EXP",
    code: "FRE",
};
pub(crate) const IVAP: Scheme = Scheme {
    id: "1016",
    name: "IVAP",
    code: "VAT",
};
pub(crate) const OTROS: Scheme = Scheme {
    id: "9999",
    name: "OTROS",
    code: "OTH",
};
pub(crate) const ICBPER: Scheme = Scheme {
    id: "7152",
    name: "ICBPER",
    code: "OTH",
};

pub(crate) fn tax_scheme(out: &mut Out<'_>, scheme: &Scheme) {
    out.raw("<cac:TaxScheme><cbc:ID>")
        .raw(scheme.id)
        .raw("</cbc:ID><cbc:Name>")
        .raw(scheme.name)
        .raw("</cbc:Name><cbc:TaxTypeCode>")
        .raw(scheme.code)
        .raw("</cbc:TaxTypeCode></cac:TaxScheme>");
}

/// Importe de impuesto en un subtotal: `n_format` o el literal `0`.
pub(crate) enum TaxAmount<'a> {
    Amount(Option<cpeforge_core::Decimal>),
    Literal(&'a str),
}

/// `<cac:TaxSubtotal>` con `TaxableAmount`, `TaxAmount` y categoría simple.
pub(crate) fn tax_subtotal(
    out: &mut Out<'_>,
    currency: Option<&str>,
    taxable: Option<cpeforge_core::Decimal>,
    tax: &TaxAmount<'_>,
    scheme: &Scheme,
) {
    out.raw("<cac:TaxSubtotal><cbc:TaxableAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .amount(taxable)
        .raw("</cbc:TaxableAmount><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">");
    match tax {
        TaxAmount::Amount(value) => out.amount(*value),
        TaxAmount::Literal(text) => out.raw(text),
    };
    out.raw("</cbc:TaxAmount><cac:TaxCategory>");
    tax_scheme(out, scheme);
    out.raw("</cac:TaxCategory></cac:TaxSubtotal>");
}

/// `<cac:TaxTotal>` de UBL 2.0 con un único subtotal: el mismo importe ya
/// formateado en `TaxAmount` total y del subtotal.
pub(crate) fn tax_total_20(
    out: &mut Out<'_>,
    currency: Option<&str>,
    formatted: &str,
    scheme: &Scheme,
) {
    out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .text(Some(formatted))
        .raw("</cbc:TaxAmount><cac:TaxSubtotal><cbc:TaxAmount currencyID=\"")
        .text(currency)
        .raw("\">")
        .text(Some(formatted))
        .raw("</cbc:TaxAmount><cac:TaxCategory>");
    tax_scheme(out, scheme);
    out.raw("</cac:TaxCategory></cac:TaxSubtotal></cac:TaxTotal>");
}

/// Variación de las líneas UBL 2.1 entre factura y notas.
pub(crate) struct Line21 {
    /// `InvoiceLine`, `CreditNoteLine` o `DebitNoteLine`.
    pub(crate) line_tag: &'static str,
    /// `InvoicedQuantity`, `CreditedQuantity` o `DebitedQuantity`.
    pub(crate) quantity_tag: &'static str,
    /// Solo la factura emite cargos y descuentos por línea.
    pub(crate) charges: bool,
    /// La factura usa `unitCode="NIU"` fijo en ICBPER; las notas la unidad de la línea.
    pub(crate) icbper_fixed_unit: bool,
    /// La factura emite `cbc:Value` si no es nulo; las notas si es verdadero.
    pub(crate) attribute_value_not_null: bool,
}

/// Líneas de factura y notas UBL 2.1.
pub(crate) fn lines_21(
    out: &mut Out<'_>,
    details: Option<&Vec<SaleDetail>>,
    currency: Option<&str>,
    kind: &Line21,
) -> Result<()> {
    for (index, detail) in details.into_iter().flatten().enumerate() {
        out.raw("<cac:")
            .raw(kind.line_tag)
            .raw("><cbc:ID>")
            .index(index)
            .raw("</cbc:ID><cbc:");
        out.raw(kind.quantity_tag)
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
        if truthy_num(detail.mto_valor_gratuito) {
            out.raw("<cac:AlternativeConditionPrice><cbc:PriceAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .limit(detail.mto_valor_gratuito, 10)
                .raw("</cbc:PriceAmount><cbc:PriceTypeCode>02</cbc:PriceTypeCode></cac:AlternativeConditionPrice>");
        } else {
            out.raw("<cac:AlternativeConditionPrice><cbc:PriceAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .limit(detail.mto_precio_unitario, 10)
                .raw("</cbc:PriceAmount><cbc:PriceTypeCode>01</cbc:PriceTypeCode></cac:AlternativeConditionPrice>");
        }
        out.raw("</cac:PricingReference>");
        if kind.charges {
            line_charges(out, detail.cargos.as_ref(), "true", currency);
            line_charges(out, detail.descuentos.as_ref(), "false", currency);
        }
        out.raw("<cac:TaxTotal><cbc:TaxAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .amount(detail.total_impuestos)
            .raw("</cbc:TaxAmount>");
        if truthy_num(detail.isc) {
            out.raw("<cac:TaxSubtotal><cbc:TaxableAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(detail.mto_base_isc)
                .raw("</cbc:TaxableAmount><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(detail.isc)
                .raw("</cbc:TaxAmount><cac:TaxCategory><cbc:Percent>")
                .num(detail.porcentaje_isc)
                .raw("</cbc:Percent><cbc:TierRange>")
                .text(detail.tip_sis_isc.as_deref())
                .raw("</cbc:TierRange>");
            tax_scheme(out, &ISC);
            out.raw("</cac:TaxCategory></cac:TaxSubtotal>");
        }
        out.raw("<cac:TaxSubtotal><cbc:TaxableAmount currencyID=\"")
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
        let afect = tributo_by_afectacion(detail.tip_afe_igv.as_deref());
        out.raw("<cac:TaxScheme><cbc:ID>")
            .text(Some(afect.id))
            .raw("</cbc:ID><cbc:Name>")
            .text(Some(afect.name))
            .raw("</cbc:Name><cbc:TaxTypeCode>")
            .text(Some(afect.code))
            .raw("</cbc:TaxTypeCode></cac:TaxScheme></cac:TaxCategory></cac:TaxSubtotal>");
        if truthy_num(detail.otro_tributo) {
            out.raw("<cac:TaxSubtotal><cbc:TaxableAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(detail.mto_base_oth)
                .raw("</cbc:TaxableAmount><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(detail.otro_tributo)
                .raw("</cbc:TaxAmount><cac:TaxCategory><cbc:Percent>")
                .num(detail.porcentaje_oth)
                .raw("</cbc:Percent>");
            tax_scheme(out, &OTROS);
            out.raw("</cac:TaxCategory></cac:TaxSubtotal>");
        }
        if truthy_num(detail.icbper) {
            out.raw("<cac:TaxSubtotal><cbc:TaxAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(detail.icbper)
                .raw("</cbc:TaxAmount><cbc:BaseUnitMeasure unitCode=\"");
            if kind.icbper_fixed_unit {
                out.raw("NIU");
            } else {
                out.text(detail.unidad.as_deref());
            }
            out.raw("\">")
                .num(detail.cantidad)
                .raw("</cbc:BaseUnitMeasure><cac:TaxCategory><cbc:PerUnitAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .amount(detail.factor_icbper)
                .raw("</cbc:PerUnitAmount>");
            tax_scheme(out, &ICBPER);
            out.raw("</cac:TaxCategory></cac:TaxSubtotal>");
        }
        out.raw("</cac:TaxTotal><cac:Item><cbc:Description><![CDATA[")
            .cdata(detail.descripcion.as_deref())
            .raw("]]></cbc:Description>");
        if truthy_str(detail.cod_producto.as_deref()) {
            out.raw("<cac:SellersItemIdentification><cbc:ID>")
                .text(detail.cod_producto.as_deref())
                .raw("</cbc:ID></cac:SellersItemIdentification>");
        }
        if truthy_str(detail.cod_prod_gs1.as_deref()) {
            out.raw("<cac:StandardItemIdentification><cbc:ID>")
                .text(detail.cod_prod_gs1.as_deref())
                .raw("</cbc:ID></cac:StandardItemIdentification>");
        }
        if truthy_str(detail.cod_prod_sunat.as_deref()) {
            out.raw("<cac:CommodityClassification><cbc:ItemClassificationCode>")
                .text(detail.cod_prod_sunat.as_deref())
                .raw("</cbc:ItemClassificationCode></cac:CommodityClassification>");
        }
        if truthy_list(detail.atributos.as_ref()) {
            for attribute in detail.atributos.iter().flatten() {
                item_property_21(out, attribute, kind.attribute_value_not_null)?;
            }
        }
        out.raw("</cac:Item><cac:Price><cbc:PriceAmount currencyID=\"")
            .text(currency)
            .raw("\">")
            .limit(detail.mto_valor_unitario, 10)
            .raw("</cbc:PriceAmount></cac:Price></cac:")
            .raw(kind.line_tag)
            .raw(">");
    }
    Ok(())
}

/// Cargos o descuentos de una línea UBL 2.1 (importes impresos sin formato).
fn line_charges(
    out: &mut Out<'_>,
    charges: Option<&Vec<Charge>>,
    indicator: &str,
    currency: Option<&str>,
) {
    if truthy_list(charges) {
        for charge in charges.into_iter().flatten() {
            out.raw("<cac:AllowanceCharge><cbc:ChargeIndicator>")
                .raw(indicator)
                .raw("</cbc:ChargeIndicator><cbc:AllowanceChargeReasonCode>")
                .text(charge.cod_tipo.as_deref())
                .raw("</cbc:AllowanceChargeReasonCode><cbc:MultiplierFactorNumeric>")
                .limit(charge.factor, 5)
                .raw("</cbc:MultiplierFactorNumeric><cbc:Amount currencyID=\"")
                .text(currency)
                .raw("\">")
                .num(charge.monto)
                .raw("</cbc:Amount><cbc:BaseAmount currencyID=\"")
                .text(currency)
                .raw("\">")
                .num(charge.monto_base)
                .raw("</cbc:BaseAmount></cac:AllowanceCharge>");
        }
    }
}

/// `cac:AdditionalItemProperty` de UBL 2.1 (con periodo de uso).
fn item_property_21(out: &mut Out<'_>, atr: &DetailAttribute, value_not_null: bool) -> Result<()> {
    out.raw("<cac:AdditionalItemProperty ><cbc:Name>")
        .text(atr.name.as_deref())
        .raw("</cbc:Name><cbc:NameCode>")
        .text(atr.code.as_deref())
        .raw("</cbc:NameCode>");
    let emit_value = if value_not_null {
        atr.value.is_some()
    } else {
        truthy_str(atr.value.as_deref())
    };
    if emit_value {
        out.raw("<cbc:Value>")
            .text(atr.value.as_deref())
            .raw("</cbc:Value>");
    }
    let has_duration = atr.duracion.is_some_and(|d| d != 0);
    if atr.fec_inicio.is_some() || atr.fec_fin.is_some() || has_duration {
        out.raw("<cac:UsabilityPeriod>");
        if atr.fec_inicio.is_some() {
            out.raw("<cbc:StartDate>")
                .date(atr.fec_inicio.as_ref())?
                .raw("</cbc:StartDate>");
        }
        if atr.fec_fin.is_some() {
            out.raw("<cbc:EndDate>")
                .date(atr.fec_fin.as_ref())?
                .raw("</cbc:EndDate>");
        }
        if has_duration {
            out.raw("<cbc:DurationMeasure unitCode=\"DAY\">")
                .int(atr.duracion)
                .raw("</cbc:DurationMeasure>");
        }
        out.raw("</cac:UsabilityPeriod>");
    }
    out.raw("</cac:AdditionalItemProperty>");
    Ok(())
}
