//! Preescaneo previo a libxml2 (revisión de seguridad): atributos por
//! elemento acotados en tiempo lineal y DOCTYPE rechazado aunque llegue en
//! UTF-16 o fuera del inicio del documento.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::time::{Duration, Instant};

use cpeforge_cpe_validator::{CpeValidatorError, InputSource, XmlDocument};

fn utf16le(text: &str) -> Vec<u8> {
    [0xFF, 0xFE]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect()
}

#[test]
fn elements_with_huge_attribute_lists_are_rejected_quickly() {
    let attributes: String = (0..50_000).map(|i| format!(" a{i}=\"x\"")).collect();
    let xml = format!("<Invoice{attributes}/>");
    let started = Instant::now();
    let error = XmlDocument::parse(&xml).err().unwrap();
    assert!(
        matches!(error, CpeValidatorError::MalformedXml { .. }),
        "{error:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn doctype_is_rejected_in_any_position_and_in_utf16() {
    for xml in [
        utf16le("<?xml version='1.0' encoding='UTF-16'?><!DOCTYPE a [<!ENTITY x 'y'>]><a>&x;</a>"),
        b"<?xml version='1.0'?><!-- c --><!DOCTYPE a><a/>".to_vec(),
    ] {
        assert!(matches!(
            XmlDocument::parse(&xml).err().unwrap(),
            CpeValidatorError::DoctypeForbidden {
                source: InputSource::Xml
            }
        ));
    }
}

#[test]
fn ordinary_documents_still_parse() {
    let xml = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<!-- <!DOCTYPE en un comentario -->
<Invoice xmlns="urn:x" a="1 = 2 > 3"><![CDATA[<!DOCTYPE>]]><b/></Invoice>"#;
    assert_eq!(
        XmlDocument::parse(xml).unwrap().root_name().as_deref(),
        Some("Invoice")
    );
    let utf16 = utf16le("<?xml version='1.0' encoding='UTF-16'?><Invoice/>");
    assert_eq!(
        XmlDocument::parse(&utf16).unwrap().root_name().as_deref(),
        Some("Invoice")
    );
}
