//! Lee una factura UBL 2.0 sintética y muestra algunos campos.
//!
//! `cargo run -p cpeforge-xml-parser --example parse_invoice`

use cpeforge_core::ports::DocumentParser;
use cpeforge_xml_parser::{InvoiceParser, ParseOptions, XmlParserError};

const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
  xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"
  xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"
  xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2"
  xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1">
  <cbc:ID>F001-123</cbc:ID>
  <cbc:IssueDate>2024-01-15</cbc:IssueDate>
  <cbc:InvoiceTypeCode>01</cbc:InvoiceTypeCode>
  <cbc:DocumentCurrencyCode>PEN</cbc:DocumentCurrencyCode>
  <cac:AccountingSupplierParty>
    <cbc:CustomerAssignedAccountID>20000000001</cbc:CustomerAssignedAccountID>
    <cac:Party><cac:PartyLegalEntity><cbc:RegistrationName>EMPRESA SINTETICA</cbc:RegistrationName></cac:PartyLegalEntity></cac:Party>
  </cac:AccountingSupplierParty>
  <cac:LegalMonetaryTotal><cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount></cac:LegalMonetaryTotal>
</Invoice>"#;

fn main() -> Result<(), XmlParserError> {
    let parser = InvoiceParser::with_options(ParseOptions::new());
    let Some(invoice) = parser.parse(XML.as_bytes())? else {
        return Ok(());
    };
    let base = &invoice.base;
    println!("serie: {:?}", base.serie);
    println!("correlativo: {:?}", base.correlativo);
    println!(
        "emisión: {:?}",
        base.fecha_emision.as_ref().map(ToString::to_string)
    );
    println!(
        "RUC: {:?}",
        base.company.as_ref().and_then(|c| c.ruc.as_deref())
    );
    println!("total: {:?}", base.mto_imp_venta);
    Ok(())
}
