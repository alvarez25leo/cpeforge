//! Ejemplo local: valida un XML sin contactar servicios externos.

use cpeforge_ubl_validator::UblValidator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(schema_dir) = std::env::args().nth(1) else {
        eprintln!("uso: validate_ubl <directorio-xsd> <archivo-xml>");
        return Ok(());
    };
    let Some(xml_path) = std::env::args().nth(2) else {
        eprintln!("uso: validate_ubl <directorio-xsd> <archivo-xml>");
        return Ok(());
    };
    let mut validator = UblValidator::new(schema_dir)?;
    let xml = std::fs::read(xml_path)?;
    let result = validator.validate(&xml)?;
    println!(
        "valid={} version={} schema={:?}",
        result.valid, result.version, result.schema
    );
    if !result.error.is_empty() {
        eprintln!("{}", result.error);
    }
    Ok(())
}
