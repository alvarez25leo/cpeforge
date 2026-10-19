//! Rechazo de entradas y recursos hostiles; todas las pruebas son locales.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "arnés de prueba"
)]

use std::fs;
use std::path::{Path, PathBuf};

use cpeforge_ubl_validator::{InfrastructureError, SchemaValidator, UblValidator};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn schema_dir() -> PathBuf {
    workspace().join("target/reference/ubl-validator-v2.3.0/src/xsd")
}
fn valid_invoice() -> Vec<u8> {
    fs::read(workspace().join("tests/reference/ubl-validator/xml/2.1-Invoice.xml")).unwrap()
}
fn temporary(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("cpeforge-ubl-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn rejects_dtd_entities_and_limits() {
    let mut validator = UblValidator::new(schema_dir()).unwrap();
    assert_eq!(validator.get_error(), None);
    for xml in [
        br#"<!DOCTYPE Invoice [<!ENTITY secret SYSTEM "file:///tmp/nonexistent-ubl-secret">]><Invoice/>"#.as_slice(),
        br#"<!DOCTYPE Invoice [<!ENTITY a "xxxxxxxx">]><Invoice>&a;</Invoice>"#.as_slice(),
    ] {
        assert_eq!(validator.validate(xml).unwrap_err(), InfrastructureError::DtdForbidden);
    }
    let utf16 = "<?xml version='1.0' encoding='UTF-16'?><!DOCTYPE Invoice [<!ENTITY a 'x'>]><Invoice>&a;</Invoice>"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<u8>>();
    let utf16 = [0xff, 0xfe].into_iter().chain(utf16).collect::<Vec<u8>>();
    assert_eq!(
        validator.validate(&utf16).unwrap_err(),
        InfrastructureError::DtdForbidden
    );
    let fixture =
        fs::read(workspace().join("tests/reference/ubl-validator/xml/security-dtd-internal.xml"))
            .unwrap();
    assert_eq!(
        validator.validate(&fixture).unwrap_err(),
        InfrastructureError::DtdForbidden
    );
    let huge = vec![b'x'; 10 * 1024 * 1024 + 1];
    assert_eq!(
        validator.validate(&huge).unwrap_err(),
        InfrastructureError::XmlLimit
    );
    let deep = format!("{}{}", "<x>".repeat(130), "</x>".repeat(130));
    assert_eq!(
        validator.validate(deep.as_bytes()).unwrap_err(),
        InfrastructureError::XmlLimit
    );
    assert_eq!(
        validator.get_error(),
        None,
        "un rechazo de seguridad no altera el último resultado"
    );
}

/// `ubl/deep-nesting`: la referencia (libxml2 2.15.1) valida la factura con 204
/// niveles; Rust la rechaza en la prevalidación (más de 128 bajo la raíz).
#[test]
fn rejects_deep_nesting_that_reference_accepts() {
    let mut validator = UblValidator::new(schema_dir()).unwrap();
    let fixture =
        fs::read(workspace().join("tests/reference/ubl-validator/xml/security-deep-nesting.xml"))
            .unwrap();
    assert_eq!(
        validator.validate(&fixture).unwrap_err(),
        InfrastructureError::XmlLimit
    );
    assert!(validator.validate(&valid_invoice()).unwrap().valid);
}

#[test]
fn rejects_unsafe_version_and_ignores_instance_schema_hint() {
    let mut validator = UblValidator::new(schema_dir()).unwrap();
    let xml = fs::read(
        workspace().join("tests/reference/ubl-validator/xml/security-version-traversal.xml"),
    )
    .unwrap();
    assert_eq!(
        validator.validate(&xml).unwrap_err(),
        InfrastructureError::UnsafePath
    );
    let xml = String::from_utf8(valid_invoice()).unwrap().replace(
        "<Invoice xmlns=",
        "<Invoice xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2 https://example.invalid/remote.xsd\" xmlns=",
    );
    assert!(validator.validate(xml.as_bytes()).unwrap().valid);
}

#[test]
fn rejects_changed_missing_and_symlinked_schema() {
    let temp = temporary("resource");
    let root = temp.join("2.1/maindoc");
    fs::create_dir_all(&root).unwrap();
    let name = "UBL-Invoice-2.1.xsd";
    let original = schema_dir().join("2.1/maindoc").join(name);
    let target = root.join(name);
    fs::copy(&original, &target).unwrap();
    let mut validator = UblValidator::new(&temp).unwrap();
    assert!(matches!(
        validator.validate(&valid_invoice()),
        Err(InfrastructureError::MissingImport(_))
    ));
    fs::write(
        &target,
        b"<schema xmlns='http://www.w3.org/2001/XMLSchema'/>",
    )
    .unwrap();
    assert!(matches!(
        validator.validate(&valid_invoice()),
        Err(InfrastructureError::SchemaHash(_))
    ));
    let mut remote_import = fs::read(&original).unwrap();
    let old = b"../common/UBL-CommonAggregateComponents-2.1.xsd";
    let replacement = b"https://example.invalid/evil.xsd";
    let offset = remote_import
        .windows(old.len())
        .position(|x| x == old)
        .unwrap();
    remote_import.splice(offset..offset + old.len(), replacement.iter().copied());
    fs::write(&target, remote_import).unwrap();
    assert!(matches!(
        validator.validate(&valid_invoice()),
        Err(InfrastructureError::SchemaHash(_))
    ));
    fs::remove_file(&target).unwrap();
    symlink_file(&original, &target);
    assert!(matches!(
        validator.validate(&valid_invoice()),
        Err(InfrastructureError::SchemaEscape(_))
    ));
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn bounded_mutation_corpus_never_panics() {
    let original = valid_invoice();
    let mut validator = UblValidator::new(schema_dir()).unwrap();
    let mut state = 0x6d2b79f5u32;
    for _ in 0..64 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut xml = original.clone();
        let index = (state as usize) % xml.len();
        xml[index] ^= 0x5a;
        let _ = validator.validate(&xml);
    }
}

#[cfg(unix)]
fn symlink_file(original: &Path, target: &Path) {
    std::os::unix::fs::symlink(original, target).unwrap();
}
#[cfg(windows)]
fn symlink_file(original: &Path, target: &Path) {
    std::os::windows::fs::symlink_file(original, target).unwrap();
}

#[test]
fn state_and_missing_schema() {
    let mut validator = UblValidator::new(schema_dir()).unwrap();
    let first = validator.validate(&valid_invoice()).unwrap();
    assert!(first.valid);
    assert_eq!(validator.get_error(), Some(""));
    let unknown = br#"<Unknown xmlns="urn:example" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><cbc:UBLVersionID>2.1</cbc:UBLVersionID></Unknown>"#;
    let second = validator.validate(unknown).unwrap();
    assert!(!second.valid);
    assert!(second.error.contains("UBL-Unknown-2.1.xsd"));
    assert_eq!(validator.get_error(), Some(second.error.as_str()));
    validator.set_version(Some("2.0")).unwrap();
    let forced = validator.validate(&valid_invoice()).unwrap();
    assert_eq!(forced.version, "2.0");
    assert_eq!(
        forced.schema.as_deref(),
        Some("2.0/maindoc/UBL-Invoice-2.0.xsd")
    );
}

#[test]
fn direct_schema_validation_and_missing_xades_import() {
    let mut validator = SchemaValidator::new(schema_dir()).unwrap();
    let invoice = valid_invoice();
    assert!(
        validator
            .validate(&invoice, "2.1/maindoc/UBL-Invoice-2.1.xsd")
            .unwrap()
    );
    assert!(validator.get_errors().is_empty());
    let invalid = fs::read(
        workspace().join("tests/reference/ubl-validator/xml/2.1-Invoice-missing-date.xml"),
    )
    .unwrap();
    assert!(
        !validator
            .validate(&invalid, "2.1/maindoc/UBL-Invoice-2.1.xsd")
            .unwrap()
    );
    assert_eq!(validator.get_errors()[0].code, 1871);
    let xades = validator.validate(
        br#"<Signature xmlns="http://www.w3.org/2000/09/xmldsig#"/>"#,
        "2.0/common/xmldsig-core-schema.xsd",
    );
    assert_eq!(
        xades.unwrap_err(),
        InfrastructureError::MissingImport("2.0/common/XAdES.xsd".into())
    );
    let arbitrary = validator.validate(
        b"<probe>synthetic</probe>",
        "../../tests/reference/ubl-validator/probe.xsd",
    );
    assert_eq!(arbitrary.unwrap_err(), InfrastructureError::UnsafePath);
}
