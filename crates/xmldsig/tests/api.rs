//! Pruebas de API sin oráculo: métodos públicos que los casos de referencia no
//! ejercitan directamente y la evaluación de confianza (sin equivalente en la referencia).

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
use cpeforge_xmldsig::dsig::{XMLDSIGNS, XmlSecurityDsig};
use cpeforge_xmldsig::key::{RSA_SHA1, RSA_SHA256, XmlSecurityKey};
use cpeforge_xmldsig::sunat::SignedXml;
use cpeforge_xmldsig::trust::{TrustPolicy, TrustResult};
use cpeforge_xmldsig::x509::X509Certificate;

const INVOICE: &str = r#"<?xml version="1.0" encoding="utf-8"?><Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions><ID>F001-1</ID></Invoice>"#;

fn read(keys: &keys::EphemeralKeys, name: &str) -> String {
    fs::read_to_string(keys.dir().join(name)).unwrap()
}

/// `SignedXml` guarda la clave ya cargada entre firmas (rendimiento): el
/// resultado debe ser el de un firmador nuevo, y la clave guardada se descarta
/// al cambiar certificado o algoritmo. Una clave inválida no se guarda.
#[test]
fn reused_signer_matches_fresh_signer() {
    let keys = keys::generate().unwrap();
    let (a, b) = (read(&keys, "a.pem"), read(&keys, "b.pem"));
    let fresh = |pem: &str, algorithm: &str| {
        let mut signer = SignedXml::new();
        signer.set_certificate(pem);
        signer.set_signature_algorithm(algorithm).unwrap();
        signer.sign_xml(INVOICE).unwrap()
    };

    let mut signer = SignedXml::new();
    signer.set_certificate(&a);
    let first = signer.sign_xml(INVOICE).unwrap();
    assert_eq!(signer.sign_xml(INVOICE).unwrap(), first);
    assert_eq!(first, fresh(&a, RSA_SHA1));
    assert_eq!(signer.clone().sign_xml(INVOICE).unwrap(), first);

    signer.set_certificate(&b);
    let with_b = signer.sign_xml(INVOICE).unwrap();
    assert_ne!(with_b, first);
    assert_eq!(with_b, fresh(&b, RSA_SHA1));

    signer.set_signature_algorithm(RSA_SHA256).unwrap();
    assert_eq!(signer.sign_xml(INVOICE).unwrap(), fresh(&b, RSA_SHA256));

    let mut invalid = SignedXml::new();
    invalid.set_certificate("no es un PEM");
    let error = invalid.sign_xml(INVOICE).unwrap_err().to_string();
    assert_eq!(invalid.sign_xml(INVOICE).unwrap_err().to_string(), error);
    invalid.set_certificate(&a);
    assert_eq!(invalid.sign_xml(INVOICE).unwrap(), first);
}

#[test]
fn dsig_public_helpers() {
    let keys = keys::generate().unwrap();
    let mut signer = SignedXml::new();
    signer.set_certificate(&read(&keys, "a.pem"));
    let signed = signer.sign_xml(INVOICE).unwrap();

    let mut doc = Document::parse(&signed).unwrap();
    let mut dsig = XmlSecurityDsig::default();
    let root = doc.document_node();
    let sig = dsig.locate_signature(&doc, root, 0).unwrap();
    let reference = doc
        .descendants_ns(sig, Some(XMLDSIGNS), "Reference")
        .first()
        .copied()
        .unwrap();
    // Sin retirar la firma, el digest envuelto no coincide (la referencia igual: solo
    // validateReference retira el nodo Signature).
    assert!(!dsig.process_reference(&doc, reference).unwrap());
    dsig.append_cert();

    let name = dsig
        .create_new_sign_node(&mut doc, "KeyName", Some("a &amp; b"))
        .unwrap();
    dsig.append_to_key_info(&mut doc, name).unwrap();
    assert!(
        doc.to_xml()
            .contains("<ds:KeyName>a &amp; b</ds:KeyName></ds:KeyInfo>")
    );
    assert!(dsig.validate_reference(&mut doc).unwrap());
    assert_eq!(dsig.validated_nodes().unwrap().len(), 1);
}

#[test]
fn trust_is_separate_from_signature_validity() {
    let keys = keys::generate().unwrap();
    let expired = read(&keys, "expired.crt");
    let a = read(&keys, "a.crt");

    // La firma con un certificado vencido es matemáticamente válida…
    let mut signer = SignedXml::new();
    signer.set_certificate(&read(&keys, "expired.pem"));
    let signed = signer.sign_xml(INVOICE).unwrap();
    assert!(SignedXml::new().verify_xml(&signed).unwrap());

    // …pero no confiable: vencido aunque sea raíz, y sin raíz no hay confianza.
    let mut policy = TrustPolicy::new();
    policy.add_root_pem(expired.as_bytes()).unwrap();
    assert!(
        matches!(policy.evaluate_pem(expired.as_bytes()).unwrap(), TrustResult::Untrusted(r) if r.contains("expired"))
    );
    assert!(matches!(
        TrustPolicy::new().evaluate_pem(a.as_bytes()).unwrap(),
        TrustResult::Untrusted(_)
    ));

    // Raíz correcta e instante dentro de la vigencia (2025-01-01).
    let mut policy = TrustPolicy::new();
    policy.add_root_pem(a.as_bytes()).unwrap();
    policy.at = Some(1_735_689_600);
    assert_eq!(
        policy.evaluate_pem(a.as_bytes()).unwrap(),
        TrustResult::Trusted
    );
    // Un certificado con el mismo sujeto pero otra clave no encadena.
    assert!(matches!(
        policy
            .evaluate_pem(read(&keys, "b.crt").as_bytes())
            .unwrap(),
        TrustResult::Untrusted(_)
    ));
}

#[test]
fn file_based_loading() {
    let keys = keys::generate().unwrap();
    let mut key = XmlSecurityKey::new(RSA_SHA1, Some("private")).unwrap();
    key.load_key_file(&keys.dir().join("a.key"), false).unwrap();
    assert!(key.has_usable_material());
    assert!(matches!(
        key.load_key_file(&keys.dir().join("no-existe"), false),
        Err(Error::FileNotFound(_))
    ));

    let pass = read(&keys, "pfx.pass");
    let cert = X509Certificate::from_file(&keys.dir().join("a.pfx"), &pass).unwrap();
    assert_eq!(
        cert.name(),
        Some("/C=PE/O=EMPRESA SINTETICA S.A.C./CN=20000000001")
    );
    assert!(matches!(
        X509Certificate::from_file(&keys.dir().join("no-existe"), &pass),
        Err(Error::FileNotFound(_))
    ));

    let xml_path = keys.dir().join("factura.xml");
    fs::write(&xml_path, INVOICE).unwrap();
    let mut signer = SignedXml::new();
    signer
        .set_certificate_from_file(&keys.dir().join("a.pem"))
        .unwrap();
    let signed = signer.sign_from_file(&xml_path).unwrap();
    assert!(SignedXml::new().verify_xml_bytes(&signed).unwrap());
    assert!(matches!(
        signer.sign_from_file(&keys.dir().join("no.xml")),
        Err(Error::FileNotFound(_))
    ));
}
