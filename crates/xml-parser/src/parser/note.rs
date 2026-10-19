//! Nota de crédito y de débito en UBL 2.0 (`NoteParser`).

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::sale::{
    BaseSale, Legend, Note, RelatedDocument, SaleDetail, SalePerception,
};

use super::full_address;
use crate::convert::{self, DateContext, explode_part, loose_eq, to_float};
use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::{Lookup, node_attribute, node_value};

super::parser!(NoteParser, Note, "Note");

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<Note, XmlParserError> {
    let root = xml.root();
    let at_root = Some(root);
    // `nodeName` incluye el prefijo: `<cn:CreditNote>` se lee como débito.
    let is_ncr = loose_eq(root.name(), "CreditNote");

    let id = xml.value("cbc:ID", at_root)?;
    // loadDocAfectado()
    let response = xml.node("cac:DiscrepancyResponse", at_root)?;
    let cod_motivo = xml.value("cbc:ResponseCode", response)?;
    let des_motivo = xml.value("cbc:Description", response)?;
    let num_doc_afectado = xml.value("cbc:ReferenceID", response)?;
    let tip_doc_afectado = xml.value(
        "cac:BillingReference/cac:InvoiceDocumentReference/cbc:DocumentTypeCode",
        None,
    )?;

    let tipo_moneda = xml.value("cbc:DocumentCurrencyCode", at_root)?;
    let fecha_emision = dates.parse(&xml.value("cbc:IssueDate", at_root)?)?;
    let company = company(xml, root)?;
    let client = client(xml, root)?;
    let mut note = Note {
        base: BaseSale {
            serie: explode_part(&id, 0),
            correlativo: explode_part(&id, 1),
            tipo_doc: Some(if is_ncr { "07" } else { "08" }.to_owned()),
            tipo_moneda: Some(tipo_moneda),
            fecha_emision: Some(fecha_emision),
            company: Some(company),
            client: Some(client),
            ..BaseSale::default()
        },
        cod_motivo: Some(cod_motivo),
        des_motivo: Some(des_motivo),
        num_doc_afectado: Some(num_doc_afectado),
        tip_doc_afectado: Some(tip_doc_afectado),
        ..Note::default()
    };

    let extensions = xml.node("ext:UBLExtensions", at_root)?;
    let additional = xml.node("//sac:AdditionalInformation", extensions)?;
    load_totals(xml, &mut note, additional)?;
    for tax in xml.nodes("cac:TaxTotal", at_root)? {
        let at = Some(tax);
        let name = xml.value("cac:TaxSubtotal/cac:TaxCategory/cac:TaxScheme/cbc:Name", at)?;
        let name = convert::trim(&name);
        let val = to_float(&xml.value_def("cbc:TaxAmount", at, "0")?)?;
        if loose_eq(name, "IGV") {
            note.base.mto_igv = Some(val);
        } else if loose_eq(name, "ISC") {
            note.base.mto_isc = Some(val);
        } else if loose_eq(name, "OTROS") {
            note.base.sum_otros_cargos = Some(val);
        }
    }
    let total_query = if is_ncr {
        "cac:LegalMonetaryTotal"
    } else {
        "cac:RequestedMonetaryTotal"
    };
    let monetary_total = xml.node(total_query, at_root)?;
    note.base.mto_otros_tributos = Some(to_float(&xml.value_def(
        "cbc:ChargeTotalAmount",
        monetary_total,
        "0",
    )?)?);
    note.base.mto_imp_venta = Some(to_float(&xml.value_def(
        "cbc:PayableAmount",
        monetary_total,
        "0",
    )?)?);
    note.base.details = Some(details(xml, root, is_ncr)?);
    note.base.legends = Some(legends(xml, additional)?);
    let mut guias = Vec::new();
    for guia in xml.nodes("cac:DespatchDocumentReference", at_root)? {
        let at = Some(guia);
        guias.push(RelatedDocument {
            tipo_doc: Some(xml.value("cbc:DocumentTypeCode", at)?),
            nro_doc: Some(xml.value("cbc:ID", at)?),
        });
    }
    note.base.guias = Some(guias);
    Ok(note)
}

fn company<'a>(xml: &Lookup<'a>, root: Node<'a>) -> Result<Company, XmlParserError> {
    let node = xml.node("cac:AccountingSupplierParty", Some(root))?;
    Ok(Company {
        ruc: Some(xml.value("cbc:CustomerAssignedAccountID", node)?),
        nombre_comercial: Some(xml.value("cac:Party/cac:PartyName/cbc:Name", node)?),
        razon_social: Some(xml.value("cac:Party/cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        address: full_address(xml, "cac:Party/cac:PostalAddress", node)?,
        ..Company::default()
    })
}

fn client<'a>(xml: &Lookup<'a>, root: Node<'a>) -> Result<Client, XmlParserError> {
    let node = xml.node("cac:AccountingCustomerParty", Some(root))?;
    Ok(Client {
        num_doc: Some(xml.value("cbc:CustomerAssignedAccountID", node)?),
        tipo_doc: Some(xml.value("cbc:AdditionalAccountID", node)?),
        rzn_social: Some(xml.value("cac:Party/cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        address: full_address(xml, "cac:Party/cac:PostalAddress", node)?,
        ..Client::default()
    })
}

fn load_totals<'a>(
    xml: &Lookup<'a>,
    note: &mut Note,
    node: Option<Node<'a>>,
) -> Result<(), XmlParserError> {
    if node.is_none() {
        return Ok(());
    }
    for total in xml.nodes("sac:AdditionalMonetaryTotal", node)? {
        let at = Some(total);
        let node_id = xml.node("cbc:ID", at)?;
        // trim(null) con strict_types lanza TypeError.
        let id = node_value(node_id).ok_or_else(|| {
            XmlParserError::runtime(
                "TypeError",
                "trim(): Argument #1 ($string) must be of type string, null given",
            )
        })?;
        let id = convert::trim(&id);
        let val = to_float(&xml.value_def("cbc:PayableAmount", at, "0")?)?;
        if loose_eq(id, "1001") {
            note.base.mto_oper_gravadas = Some(val);
        } else if loose_eq(id, "1002") {
            note.base.mto_oper_inafectas = Some(val);
        } else if loose_eq(id, "1003") {
            note.base.mto_oper_exoneradas = Some(val);
        } else if loose_eq(id, "1004") {
            note.base.mto_oper_gratuitas = Some(val);
        } else if loose_eq(id, "2001") {
            note.perception = Some(SalePerception {
                cod_reg: Some(node_attribute(node_id, "schemeID")?),
                mto: Some(val),
                mto_base: Some(to_float(&xml.value_def("sac:ReferenceAmount", at, "0")?)?),
                mto_total: Some(to_float(&xml.value_def("sac:TotalAmount", at, "0")?)?),
                ..SalePerception::default()
            });
        }
    }
    Ok(())
}

fn legends<'a>(xml: &Lookup<'a>, node: Option<Node<'a>>) -> Result<Vec<Legend>, XmlParserError> {
    let mut out = Vec::new();
    if node.is_none() {
        return Ok(out);
    }
    for legend in xml.nodes("sac:AdditionalProperty", node)? {
        let at = Some(legend);
        out.push(Legend {
            code: Some(xml.value("cbc:ID", at)?),
            value: Some(xml.value("cbc:Value", at)?),
        });
    }
    Ok(out)
}

fn details<'a>(
    xml: &Lookup<'a>,
    root: Node<'a>,
    is_ncr: bool,
) -> Result<Vec<SaleDetail>, XmlParserError> {
    let (line, quantity) = if is_ncr {
        ("cac:CreditNoteLine", "cbc:CreditedQuantity")
    } else {
        ("cac:DebitNoteLine", "cbc:DebitedQuantity")
    };
    let mut out = Vec::new();
    for node in xml.nodes(line, Some(root))? {
        let at = Some(node);
        let quant = xml.node(quantity, at)?;
        let mut det = SaleDetail {
            cantidad: Some(to_float(&node_value(quant).unwrap_or_default())?),
            unidad: Some(node_attribute(quant, "unitCode")?),
            mto_valor_venta: Some(to_float(&xml.value("cbc:LineExtensionAmount", at)?)?),
            mto_valor_unitario: Some(to_float(&xml.value("cac:Price/cbc:PriceAmount", at)?)?),
            descripcion: Some(xml.value("cac:Item/cbc:Description", at)?),
            cod_producto: Some(xml.value("cac:Item/cac:SellersItemIdentification/cbc:ID", at)?),
            cod_prod_sunat: Some(xml.value(
                "cac:Item/cac:CommodityClassification/cbc:ItemClassificationCode",
                at,
            )?),
            ..SaleDetail::default()
        };
        for tax in xml.nodes("cac:TaxTotal", at)? {
            let tax = Some(tax);
            let name = xml.value(
                "cac:TaxSubtotal/cac:TaxCategory/cac:TaxScheme/cbc:Name",
                tax,
            )?;
            let name = convert::trim(&name);
            let val = to_float(&xml.value_def("cbc:TaxAmount", tax, "0")?)?;
            if loose_eq(name, "IGV") {
                det.igv = Some(val);
                det.tip_afe_igv = Some(xml.value(
                    "cac:TaxSubtotal/cac:TaxCategory/cbc:TaxExemptionReasonCode",
                    tax,
                )?);
            } else if loose_eq(name, "ISC") {
                det.isc = Some(val);
                det.tip_sis_isc =
                    Some(xml.value("cac:TaxSubtotal/cac:TaxCategory/cbc:TierRange", tax)?);
            }
        }
        for desc in xml.nodes("cac:AllowanceCharge", at)? {
            let desc = Some(desc);
            let charge = xml.value("cbc:ChargeIndicator", desc)?;
            if loose_eq(convert::trim(&charge), "false") {
                det.descuento = Some(to_float(&xml.value_def("cbc:Amount", desc, "0")?)?);
            }
        }
        for price in xml.nodes("cac:PricingReference", at)? {
            let price = Some(price);
            let code = xml.value("cac:AlternativeConditionPrice/cbc:PriceTypeCode", price)?;
            let code = convert::trim(&code);
            let value = to_float(&xml.value_def(
                "cac:AlternativeConditionPrice/cbc:PriceAmount",
                price,
                "0",
            )?)?;
            if loose_eq(code, "01") {
                det.mto_precio_unitario = Some(value);
            } else if loose_eq(code, "02") {
                det.mto_valor_gratuito = Some(value);
            }
        }
        out.push(det);
    }
    Ok(out)
}
