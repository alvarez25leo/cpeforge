//! Liquidación de compra (`SelfBilledInvoice`, tipo `04`): tipo de documento
//! y nombre de archivo SUNAT, con el RUC del comprador que la emite.

#![allow(clippy::unwrap_used, reason = "pruebas")]

use cpeforge_ws::cdr::{extract_filename, resolve_type};

const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?><SelfBilledInvoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:SelfBilledInvoice-2" xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:ID>L001-7</cbc:ID><cbc:InvoiceTypeCode listID="0501">04</cbc:InvoiceTypeCode><cac:AccountingCustomerParty><cac:Party><cac:PartyIdentification><cbc:ID schemeID="6">20000000001</cbc:ID></cac:PartyIdentification></cac:Party></cac:AccountingCustomerParty><cac:AccountingSupplierParty><cac:Party><cac:PartyIdentification><cbc:ID schemeID="1">40123456</cbc:ID></cac:PartyIdentification></cac:Party></cac:AccountingSupplierParty></SelfBilledInvoice>"#;

#[test]
fn settlement_type_is_recognized() {
    assert_eq!(
        resolve_type(XML).unwrap(),
        "CPEForge\\Model\\Settlement\\PurchaseSettlement"
    );
}

#[test]
fn settlement_filename_uses_the_buyer_ruc_not_the_seller_dni() {
    assert_eq!(extract_filename(XML).unwrap(), "20000000001-04-L001-7");
}
