//! Regresiones de la revisión de seguridad de 2026-10-19 (complementa
//! `hardening.rs`): confusión de
//! algoritmo RSA/HMAC, referencias sin destino, tope de referencias, perfil
//! SUNAT de [`SignedXml`], estado del firmante y codificación de las
//! variantes `&str`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "support/keys.rs"]
mod keys;

use std::fs;

use cpeforge_xmldsig::Error;
use cpeforge_xmldsig::dom::Document;
use cpeforge_xmldsig::dsig::{MAX_REFERENCES, XmlSecurityDsig};
use cpeforge_xmldsig::enc::XmlSecEnc;
use cpeforge_xmldsig::key::{HMAC_SHA1, RSA_SHA1, RSA_SHA256, XmlSecurityKey};
use cpeforge_xmldsig::sunat::SignedXml;

const INVOICE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions><cbc:ID>F001-1</cbc:ID><cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount></Invoice>"#;

fn signer(keys: &keys::EphemeralKeys, name: &str) -> SignedXml {
    let mut signer = SignedXml::new();
    signer.set_certificate(&fs::read_to_string(keys.dir().join(format!("{name}.pem"))).unwrap());
    signer
}

#[test]
fn certificate_from_key_info_is_never_an_hmac_secret() {
    let keys = keys::generate().unwrap();
    let signed = signer(&keys, "a").sign_xml(INVOICE).unwrap();
    // El atacante declara HMAC: el certificado público (conocido) no puede
    // convertirse en el secreto del HMAC.
    let forged = signed.replace(RSA_SHA1, HMAC_SHA1);
    let doc = Document::parse(&forged).unwrap();
    let mut dsig = XmlSecurityDsig::default();
    let sig = dsig.locate_signature(&doc, doc.document_node(), 0).unwrap();
    let key = dsig.locate_key(&doc).unwrap();
    assert!(key.is_hmac());
    assert_eq!(
        XmlSecEnc::static_locate_key_info(&doc, Some(key), sig).err(),
        Some(Error::PublicKeyAsHmacSecret)
    );
    // Y la verificación SUNAT lo rechaza (error tipado, nunca `Ok(true)`).
    assert_eq!(
        SignedXml::new().verify_xml(&forged),
        Err(Error::PublicKeyAsHmacSecret)
    );
}

#[test]
fn reference_without_target_never_validates() {
    let keys = keys::generate().unwrap();
    let signed = signer(&keys, "a").sign_xml(INVOICE).unwrap();
    // Referencia a un Id inexistente con el digest SHA-1 de la cadena vacía:
    // la referencia la aceptaba sin cubrir ningún contenido.
    for uri in ["#no-existe", "http://127.0.0.1:9/x"] {
        let forged = signed
            .replacen(r#"URI="""#, &format!(r#"URI="{uri}""#), 1)
            .replace(
                &text_between(&signed, "<ds:DigestValue>", "</ds:DigestValue>"),
                "2jmj7l5rSw0yVb/vlWAYkK/YBwk=",
            );
        let mut doc = Document::parse(&forged).unwrap();
        let mut dsig = XmlSecurityDsig::default();
        dsig.locate_signature(&doc, doc.document_node(), 0).unwrap();
        assert_eq!(
            dsig.validate_reference(&mut doc).err(),
            Some(Error::ReferenceValidationFailed),
            "{uri}"
        );
    }
}

#[test]
fn too_many_references_are_rejected_before_digesting() {
    let keys = keys::generate().unwrap();
    let signed = signer(&keys, "a").sign_xml(INVOICE).unwrap();
    let reference = format!(
        "<ds:Reference{}</ds:Reference>",
        text_between(&signed, "<ds:Reference", "</ds:Reference>")
    );
    let many = reference.repeat(MAX_REFERENCES + 1);
    let forged = signed.replacen(&reference, &many, 1);
    let mut doc = Document::parse(&forged).unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.locate_signature(&doc, doc.document_node(), 0).unwrap();
    assert_eq!(
        dsig.validate_reference(&mut doc).err(),
        Some(Error::TooManyReferences(MAX_REFERENCES + 1))
    );
}

#[test]
fn sunat_profile_requires_single_enveloped_reference() {
    let keys = keys::generate().unwrap();
    let mut verifier = signer(&keys, "a");
    let signed = verifier.sign_xml(INVOICE).unwrap();
    assert!(
        verifier.verify_xml(&signed).unwrap(),
        "la firma SUNAT válida verifica"
    );

    // Referencia por #Id (forma de trasplante): no es el perfil SUNAT.
    let by_id = signed.replacen(r#"URI="""#, r##"URI="#x""##, 1);
    assert!(!verifier.verify_xml(&by_id).unwrap());
    // Sin la transformación enveloped.
    let no_enveloped = signed.replace(
        "http://www.w3.org/2000/09/xmldsig#enveloped-signature",
        "http://www.w3.org/TR/2001/REC-xml-c14n-20010315",
    );
    assert!(!verifier.verify_xml(&no_enveloped).unwrap());
}

#[test]
fn verifying_does_not_change_the_signer() {
    let keys = keys::generate().unwrap();
    let mut signer_a = signer(&keys, "a");
    signer_a.set_signature_algorithm(RSA_SHA256).unwrap();
    let own_cert = signer_a.configured_public_key().unwrap().to_owned();
    assert!(
        !own_cert.contains("PRIVATE KEY"),
        "la clave pública no expone la privada"
    );
    assert!(own_cert.contains("BEGIN CERTIFICATE"));

    // Documento de otro firmante (otra clave, RSA-SHA1).
    let foreign = signer(&keys, "b").sign_xml(INVOICE).unwrap();
    assert!(signer_a.verify_xml(&foreign).unwrap());
    assert_eq!(signer_a.signature_algorithm(), RSA_SHA256);
    assert_eq!(signer_a.configured_public_key(), Some(own_cert.as_str()));

    // La firma siguiente sigue siendo de A, con su algoritmo.
    let next = signer_a.sign_xml(INVOICE).unwrap();
    assert!(next.contains(RSA_SHA256));
    let mut fresh = SignedXml::new();
    assert!(fresh.verify_xml(&next).unwrap());
}

#[test]
fn str_variants_require_utf8() {
    let keys = keys::generate().unwrap();
    let latin1 = INVOICE.replace(r#"encoding="utf-8""#, r#"encoding="ISO-8859-1""#);
    let err = signer(&keys, "a").sign_xml(&latin1).unwrap_err();
    assert!(err.to_string().contains("ISO-8859-1"), "{err}");
    // La variante en bytes sí admite la codificación declarada.
    assert!(signer(&keys, "a").sign_xml_bytes(latin1.as_bytes()).is_ok());
}

#[test]
fn symmetric_key_cannot_sign_silently() {
    let mut key = XmlSecurityKey::new(cpeforge_xmldsig::key::AES256_CBC, None).unwrap();
    key.load_key(&[7u8; 32], false).unwrap();
    assert_eq!(
        key.sign_data(b"datos").err(),
        Some(Error::UnsupportedSigningKey)
    );
}

fn text_between(haystack: &str, start: &str, end: &str) -> String {
    let from = haystack.find(start).unwrap() + start.len();
    let to = haystack[from..].find(end).unwrap() + from;
    haystack[from..to].to_owned()
}

// --- Denegación de servicio (DOM, C14N y XPath) -----------------------------

use std::time::{Duration, Instant};

use cpeforge_xmldsig::XmlError;
use cpeforge_xmldsig::c14n::{self, Method};

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let out = f();
    (out, started.elapsed())
}

#[test]
fn many_prefixed_attributes_parse_in_linear_time() {
    // 160 000 atributos con namespace tardaban 15 s (duplicados cuadráticos).
    let mut xml = String::from(r#"<a xmlns:p="urn:p""#);
    for i in 0..160_000 {
        xml.push_str(&format!(r#" p:a{i}="""#));
    }
    xml.push_str("/>");
    let (doc, elapsed) = timed(|| Document::parse(&xml));
    assert!(doc.is_ok());
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
    // Los duplicados siguen detectándose.
    assert!(matches!(
        Document::parse(r#"<a xmlns:p="u" xmlns:q="u" p:x="1" q:x="2"/>"#),
        Err(XmlError::Malformed(_))
    ));
}

#[test]
fn undeclared_prefixes_and_signed_char_refs_are_rejected() {
    for xml in [
        "<p:a/>",
        r#"<a p:x="1"/>"#,
        "<a>&#+65;</a>",
        "<a b='&#x+41;'/>",
    ] {
        assert!(
            matches!(Document::parse(xml), Err(XmlError::Malformed(_))),
            "{xml}"
        );
    }
    assert!(Document::parse(r#"<a xml:lang="es">&#65;&#x41;</a>"#).is_ok());
}

#[test]
fn inherited_xml_attributes_are_linear() {
    // 200 ancestros con muchos xml:* y la firma dentro: 2,9 MB tardaban 35 s
    // antes de buscar siquiera la clave.
    let attrs: String = (0..2_000).map(|i| format!(r#" xml:a{i}="""#)).collect();
    let mut xml = String::new();
    for _ in 0..200 {
        xml.push_str(&format!("<e{attrs}>"));
    }
    xml.push_str(r#"<ds:Signature xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ds:SignedInfo/></ds:Signature>"#);
    xml.push_str(&"</e>".repeat(200));
    let (result, elapsed) = timed(|| SignedXml::new().verify_xml(&xml));
    assert!(!matches!(result, Ok(true)));
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
}

fn xpath_over(doc: &str, expr: &str) -> (Result<String, Error>, Duration) {
    let doc = Document::parse(doc).unwrap();
    let method = Method::from_uri(c14n::C14N).unwrap();
    timed(|| c14n::canonicalize_xpath(&doc, doc.document_node(), &method, expr, &[]))
}

#[test]
fn hidden_xpath_costs_are_charged() {
    // Nodos de namespace: 1 000 declaraciones × 1 000 elementos (40 s antes).
    let decls: String = (0..1_000)
        .map(|i| format!(r#" xmlns:p{i}="u{i}""#))
        .collect();
    let doc = format!("<r{decls}>{}</r>", "<a/>".repeat(1_000));
    let (result, elapsed) = xpath_over(&doc, "count(namespace::*[. = 'zz']) = 0");
    assert!(matches!(result, Err(Error::XPath(_))), "{result:?}");
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");

    // Conjunto × conjunto.
    let doc = format!(
        "<r>{}{}</r>",
        "<a>1</a>".repeat(5_000),
        "<c>2</c>".repeat(5_000)
    );
    let (result, elapsed) = xpath_over(&doc, "not(self::r) or not(//a = //c)");
    assert!(matches!(result, Err(Error::XPath(_))), "{result:?}");
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");

    // `string(/)` por cada nodo sobre un documento con mucho texto.
    let doc = format!(
        "<r>{}</r>",
        format!("<a>{}</a>", "x".repeat(100)).repeat(10_000)
    );
    let (result, elapsed) = xpath_over(&doc, "string(/) != ''");
    assert!(matches!(result, Err(Error::XPath(_))), "{result:?}");
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
}
