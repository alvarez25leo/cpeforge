//! Recibo por honorarios (`ReceiptParser`).

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::Receipt;

use crate::convert::{DateContext, explode_part, to_float};
use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::Lookup;

super::parser!(ReceiptParser, Receipt, "Receipt");

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<Receipt, XmlParserError> {
    let root = Some(xml.root());
    let id = xml.value("cbc:ID", root)?;
    let fecha_emision = dates.parse(&xml.value("cbc:IssueDate", root)?)?;
    let monto_letras = xml.value("cbc:Note", root)?;
    let person_node = xml.node("cac:AccountingSupplierParty", root)?;
    let person = Company {
        ruc: Some(xml.value("cbc:CustomerAssignedAccountID", person_node)?),
        razon_social: Some(xml.value("cac:Party/cac:PartyName/cbc:Name", person_node)?),
        address: Some(address(xml, person_node)?),
        ..Company::default()
    };
    let client_node = xml.node("cac:AccountingCustomerParty", root)?;
    let receptor = Client {
        num_doc: Some(xml.value("cbc:CustomerAssignedAccountID", client_node)?),
        tipo_doc: Some(xml.value("cbc:AdditionalAccountID", client_node)?),
        rzn_social: Some(xml.value("cac:Party/cac:PartyName/cbc:Name", client_node)?),
        address: Some(address(xml, client_node)?),
        ..Client::default()
    };
    let monetary_total = xml.node("cac:LegalMonetaryTotal", root)?;
    let float = |query: &str, context: Option<Node<'_>>| {
        xml.value_def(query, context, "0")
            .and_then(|v| to_float(&v))
    };
    let sub_total = float("cac:TaxTotal/cac:TaxSubtotal/cbc:TaxableAmount", root)?;
    let retencion = float("cbc:TaxExclusiveAmount", monetary_total)?;
    let total = float("cbc:PayableAmount", monetary_total)?;
    // loadFromDetail()
    let line = xml.node("cac:InvoiceLine", root)?;
    let porcentaje = float("cac:TaxTotal/cac:TaxSubtotal/cbc:Percent", line)?;
    let concepto = xml.value("cac:Item/cbc:Description", line)?;
    Ok(Receipt {
        person: Some(person),
        receptor: Some(receptor),
        serie: explode_part(&id, 0),
        correlativo: explode_part(&id, 1),
        fecha_emision: Some(fecha_emision),
        concepto: Some(concepto),
        monto_letras: Some(monto_letras),
        sub_total: Some(sub_total),
        retencion: Some(retencion),
        porcentaje: Some(porcentaje),
        total: Some(total),
    })
}

/// Solo la calle (`cac:Party/cac:PostalAddress/cbc:StreetName`).
fn address<'a>(xml: &Lookup<'a>, node: Option<Node<'a>>) -> Result<Address, XmlParserError> {
    Ok(Address {
        direccion: Some(xml.value("cac:Party/cac:PostalAddress/cbc:StreetName", node)?),
        ..Address::default()
    })
}
