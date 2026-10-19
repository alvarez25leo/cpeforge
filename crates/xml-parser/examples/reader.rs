//! Consultas con `XmlReader` y rechazo de entradas hostiles.
//!
//! `cargo run -p cpeforge-xml-parser --example reader`

use cpeforge_xml_parser::{XmlParserError, XmlReader};

fn main() -> Result<(), XmlParserError> {
    let xml = r#"<r xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
        <cbc:ID schemeID="6">20000000001</cbc:ID><cbc:ID>2</cbc:ID></r>"#;
    let reader = XmlReader::load_xml(xml.as_bytes())?;
    println!("primer ID: {}", reader.value("cbc:ID", None)?);
    println!("schemeID: {}", reader.value("cbc:ID/@schemeID", None)?);
    let count = reader.nodes("cbc:ID", None)?.map_or(0, |nodes| nodes.len());
    println!("IDs: {count}");
    // Prefijo no declarado: la referencia devuelve `false`; aquí `None`.
    println!("prefijo indefinido: {:?}", reader.nodes("zz:ID", None)?);

    let hostile = br#"<!DOCTYPE r [<!ENTITY e SYSTEM "file:///etc/passwd">]><r>&e;</r>"#;
    match XmlReader::load_xml(hostile) {
        Err(err) => println!("rechazado: {err}"),
        Ok(_) => println!("aceptado"),
    }
    Ok(())
}
