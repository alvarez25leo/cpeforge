//! Preescaneo previo a libxml2 (revisión de seguridad): atributos por
//! elemento acotados en tiempo lineal y DOCTYPE rechazado en cualquier
//! posición, también en UTF-16 big-endian sin BOM.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use cpeforge_ubl_validator::{InfrastructureError, UblValidator};
use std::fmt::Write as _;

fn validator() -> UblValidator {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/reference/ubl-validator-v2.3.0/src/xsd");
    UblValidator::new(dir).unwrap()
}

#[test]
fn elements_with_huge_attribute_lists_are_rejected_quickly() {
    let attributes: String = (0..50_000).fold(String::new(), |mut out, i| {
        let _ = write!(out, " a{i}=\"x\"");
        out
    });
    let xml = format!("<Invoice{attributes}/>");
    let started = Instant::now();
    assert_eq!(
        validator().validate(xml.as_bytes()).unwrap_err(),
        InfrastructureError::XmlLimit
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn doctype_is_rejected_after_comments_and_in_utf16_be() {
    let utf16be: Vec<u8> = "<!DOCTYPE a [<!ENTITY x 'y'>]><a>&x;</a>"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    for xml in [
        utf16be,
        b"<?xml version='1.0'?><!-- c --><!ELEMENT a ANY><a/>".to_vec(),
    ] {
        assert_eq!(
            validator().validate(&xml).unwrap_err(),
            InfrastructureError::DtdForbidden
        );
    }
}
