//! Ejemplo local sin red ni datos de clientes.

use cpeforge_core::model::sale::Invoice;
use cpeforge_validator::Validator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let invoice = Invoice::default();
    let mut validator = Validator::new();
    validator.set_version(Some("2.1"));
    for violation in validator.validate_document(&invoice)? {
        println!("{}: {}", violation.path, violation.message);
    }
    Ok(())
}
