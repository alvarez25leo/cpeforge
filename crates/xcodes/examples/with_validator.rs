//! Traducción real a través del contrato `core` consumido por `validator`.

use cpeforge_validator::Validator;
use cpeforge_xcodes::XmlErrorCodeProvider;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = XmlErrorCodeProvider::new()?;
    let mut validator = Validator::new();
    validator.set_provider(catalog);
    println!(
        "{}",
        validator.translate("0100", &BTreeMap::new(), None, None)?
    );
    Ok(())
}
