//! Regresiones de la revisión de seguridad: atributos por elemento y ámbitos
//! de espacios de nombres se limitan antes de construir el árbol.

use std::time::{Duration, Instant};

use cpeforge_xml_parser::{XmlDocument, XmlParserError};
use std::fmt::Write as _;

fn many_attributes(n: usize) -> String {
    let attributes: String = (0..n).fold(String::new(), |mut out, i| {
        let _ = write!(out, " a{i}=''");
        out
    });
    format!("<Invoice{attributes}/>")
}

/// 250 prefijos en la raíz y `n` hijos que declaran cada uno un prefijo
/// distinto: sin límite, cada hijo copia los 251 espacios en ámbito (645 KB
/// de entrada llegaban a unos 685 MB de memoria).
fn namespace_amplification(n: usize) -> String {
    let declarations: String = (0..250).fold(String::new(), |mut out, i| {
        let _ = write!(out, " xmlns:p{i}='urn:{i}'");
        out
    });
    let children: String = (0..n).fold(String::new(), |mut out, i| {
        let _ = write!(out, "<x xmlns:z{i}='u'/>");
        out
    });
    format!("<Invoice{declarations}>{children}</Invoice>")
}

#[test]
fn many_attributes_are_rejected_before_parsing() {
    let started = Instant::now();
    assert_eq!(
        XmlDocument::parse(many_attributes(40_000).as_bytes()).err(),
        Some(XmlParserError::TooManyAttributes { limit: 256 })
    );
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(XmlDocument::parse(many_attributes(250).as_bytes()).is_ok());
}

#[test]
fn namespace_amplification_is_rejected() {
    assert!(matches!(
        XmlDocument::parse(namespace_amplification(20_000).as_bytes()),
        Err(XmlParserError::TooManyNamespaces { .. })
    ));
    assert!(XmlDocument::parse(namespace_amplification(1_000).as_bytes()).is_ok());
}

#[test]
fn utf16_input_is_checked_after_decoding() {
    let xml = format!(
        "<?xml version='1.0' encoding='UTF-16'?>{}",
        many_attributes(300)
    );
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(
        XmlDocument::parse(&bytes).err(),
        Some(XmlParserError::TooManyAttributes { limit: 256 })
    );
    let ok = "<?xml version='1.0' encoding='UTF-16'?><Invoice a='1'/>";
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(ok.encode_utf16().flat_map(u16::to_le_bytes));
    assert!(XmlDocument::parse(&bytes).is_ok());
}
