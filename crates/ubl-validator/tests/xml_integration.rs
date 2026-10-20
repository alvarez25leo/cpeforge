//! Generación cpeforge-xml → XSD del validador nuevo.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "arnés de prueba"
)]

#[path = "../../core/tests/support/hydrate.rs"]
mod hydrate;

use cpeforge_core::model::sale::Invoice;
use cpeforge_ubl_validator::UblValidator;
use cpeforge_xml::{InvoiceBuilder, RenderOptions};
use hydrate::{deserialize, to_serde};
use jiff::Timestamp;
use serde_json::Value;
use std::{fs, path::PathBuf};

#[test]
fn rust_invoice_builder_produces_valid_xsd_document() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases: Value = serde_json::from_slice(&fs::read(
        workspace.join("tests/reference/xml/cases/invoice.json"),
    )?)?;
    let case = cases["cases"]
        .as_array()
        .ok_or("cases ausentes")?
        .iter()
        .find(|c| c["id"] == "inv21-minimal-default")
        .ok_or("factura ausente")?;
    let value = to_serde(&case["input"])?.map_err(|e| format!("{e}"))?;
    let invoice: Invoice = deserialize(value).map_err(|e| format!("{e}"))?;
    let mut options = RenderOptions::default();
    options.now = Some(Timestamp::from_second(1_710_000_000)?);
    let generated = InvoiceBuilder::new(options).render(&invoice)?;
    // El generador deja ExtensionContent vacío para que lo rellene la firma;
    // el XSD 2.1 exige al menos un nodo externo. Igual adaptación que el test
    // XSD independiente existente de cpeforge-xml.
    let signed_slot = generated.replace(
        "<ext:ExtensionContent/>",
        "<ext:ExtensionContent><cbc:AccountID>1</cbc:AccountID></ext:ExtensionContent>",
    );
    let mut validator =
        UblValidator::new(workspace.join("target/reference/ubl-validator-v2.3.0/src/xsd"))?;
    let result = validator.validate(signed_slot.as_bytes())?;
    assert!(result.valid, "{}", result.error);
    Ok(())
}
