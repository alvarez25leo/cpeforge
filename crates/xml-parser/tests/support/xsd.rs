//! Validación XSD independiente con `xmllint` (libxml2) de los XML que leen
//! las pruebas, contra los esquemas UBL fijados que prepara
//! `scripts/prepare_ubl_validator_reference.sh` en `target/reference`.
//!
//! Como en `cpeforge-xml/tests/schema.rs`, a los documentos UBL 2.1 se les
//! añade `<cbc:AccountID>1</cbc:AccountID>` en el primer
//! `ext:ExtensionContent` (vacío hasta que se firme), **en una copia**.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn xsd_dir() -> Result<PathBuf, String> {
    let dir = std::env::var_os("CPEFORGE_XSD_DIR").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/reference/ubl-validator-v2.3.0/src/xsd")
        },
        PathBuf::from,
    );
    if dir.join("2.1/maindoc/UBL-Invoice-2.1.xsd").is_file() {
        Ok(dir)
    } else {
        Err(format!(
            "faltan los XSD en {}; ejecutar scripts/prepare_ubl_validator_reference.sh",
            dir.display()
        ))
    }
}

/// Exige que `xml` valide contra el XSD de su raíz y versión UBL.
pub(crate) fn assert_valid(xml: &str, name: &str) -> Result<(), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("{name}: mal formado: {e}"))?;
    let root = doc.root_element();
    let version = root
        .descendants()
        .find(|n| n.tag_name().name() == "UBLVersionID")
        .and_then(|n| n.text())
        .unwrap_or("2.0")
        .to_owned();
    let schema = xsd_dir()?
        .join(&version)
        .join("maindoc")
        .join(format!("UBL-{}-{version}.xsd", root.tag_name().name()));
    if !schema.is_file() {
        return Err(format!("{name}: no hay XSD {}", schema.display()));
    }
    let copy = if version == "2.1" {
        xml.replacen(
            "<ext:ExtensionContent/>",
            "<ext:ExtensionContent><cbc:AccountID>1</cbc:AccountID></ext:ExtensionContent>",
            1,
        )
    } else {
        xml.to_owned()
    };
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("xml-parser-xsd");
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let path = tmp.join(format!("{name}.xml"));
    fs::write(&path, copy).map_err(|e| e.to_string())?;
    let output = Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&path)
        .output()
        .map_err(|e| format!("no se pudo ejecutar xmllint: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{name}: no valida contra el XSD\n{}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
