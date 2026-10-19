//! Consulta local del catálogo fijo, sin red.

use cpeforge_xcodes::XmlErrorCodeProvider;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = XmlErrorCodeProvider::new()?;
    println!("0100: {}", catalog.get("0100"));
    println!("registros: {}", catalog.iter().len());
    Ok(())
}
