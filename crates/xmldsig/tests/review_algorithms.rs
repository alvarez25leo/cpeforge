//! Piso de algoritmos y usos de clave (revisión de seguridad): SHA-1
//! rechazable fuera del perfil SUNAT y `TrustPolicy` con `keyUsage` y
//! `extendedKeyUsage`.
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
use cpeforge_xmldsig::dsig::{EXC_C14N, SHA1, SHA256, XmlSecurityDsig};
use cpeforge_xmldsig::key::{RSA_SHA1, RSA_SHA256, SignatureCheck, XmlSecurityKey};
use cpeforge_xmldsig::trust::{TrustPolicy, TrustResult};
use openssl::asn1::Asn1Time;
use openssl::bn::BigNum;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::x509::extension::{BasicConstraints, ExtendedKeyUsage, KeyUsage};
use openssl::x509::{X509, X509NameBuilder};

fn sign(keys: &keys::EphemeralKeys, signature: &str, digest: &str) -> String {
    let mut doc = Document::parse("<r><item>1</item></r>").unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.set_canonical_method(&mut doc, EXC_C14N).unwrap();
    let root = doc.document_element().unwrap();
    dsig.add_reference(&mut doc, root, digest, None, None)
        .unwrap();
    let mut key = XmlSecurityKey::new(signature, Some("private")).unwrap();
    key.load_key(&fs::read(keys.dir().join("a.pem")).unwrap(), false)
        .unwrap();
    dsig.sign(&mut doc, &key, Some(root)).unwrap();
    doc.to_xml()
}

/// Valida referencias y firma con o sin `reject_sha1`.
fn check(
    keys: &keys::EphemeralKeys,
    xml: &str,
    reject_sha1: bool,
) -> Result<SignatureCheck, Error> {
    let mut doc = Document::parse(xml).unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.reject_sha1 = reject_sha1;
    let root = doc.document_node();
    dsig.locate_signature(&doc, root, 0).unwrap();
    dsig.canonicalize_signed_info(&doc).unwrap();
    let method = if xml.contains("rsa-sha1") {
        RSA_SHA1
    } else {
        RSA_SHA256
    };
    let mut key = XmlSecurityKey::new(method, Some("public")).unwrap();
    key.load_key(&fs::read(keys.dir().join("a.crt")).unwrap(), true)
        .unwrap();
    let verdict = dsig.verify(&doc, &key)?;
    dsig.validate_reference(&mut doc)?;
    Ok(verdict)
}

#[test]
fn sha1_is_accepted_by_default_and_rejected_on_request() {
    let keys = keys::generate().unwrap();
    let modern = sign(&keys, RSA_SHA256, SHA256);
    assert_eq!(check(&keys, &modern, true).unwrap(), SignatureCheck::Valid);

    // Perfil SUNAT: RSA-SHA1 y SHA-1 siguen siendo válidos por defecto.
    let sunat = sign(&keys, RSA_SHA1, SHA1);
    assert_eq!(check(&keys, &sunat, false).unwrap(), SignatureCheck::Valid);
    assert_eq!(
        check(&keys, &sunat, true).unwrap_err(),
        Error::WeakAlgorithm(RSA_SHA1.to_owned())
    );
    // Firma moderna con un digest SHA-1 en la referencia.
    let mixed = sign(&keys, RSA_SHA256, SHA1);
    assert_eq!(
        check(&keys, &mixed, true).unwrap_err(),
        Error::WeakAlgorithm(SHA1.to_owned())
    );
}

/// Certificado autofirmado con los usos indicados (vigente en 2025).
fn certificate(key_usage: KeyUsage, extended: Option<ExtendedKeyUsage>) -> String {
    let pkey = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", "20000000001").unwrap();
    let name = name.build();
    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder
        .set_serial_number(&BigNum::from_u32(7).unwrap().to_asn1_integer().unwrap())
        .unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_issuer_name(&name).unwrap();
    builder
        .set_not_before(&Asn1Time::from_str("20240101000000Z").unwrap())
        .unwrap();
    builder
        .set_not_after(&Asn1Time::from_str("20300101000000Z").unwrap())
        .unwrap();
    builder.set_pubkey(&pkey).unwrap();
    builder
        .append_extension(BasicConstraints::new().critical().build().unwrap())
        .unwrap();
    let mut key_usage = key_usage;
    builder
        .append_extension(key_usage.critical().build().unwrap())
        .unwrap();
    if let Some(extended) = extended {
        builder.append_extension(extended.build().unwrap()).unwrap();
    }
    builder.sign(&pkey, MessageDigest::sha256()).unwrap();
    String::from_utf8(builder.build().to_pem().unwrap()).unwrap()
}

fn evaluate(pem: &str, accepted: &[&str]) -> TrustResult {
    let mut policy = TrustPolicy::new();
    policy.add_root_pem(pem.as_bytes()).unwrap();
    policy.at = Some(1_735_689_600);
    policy.accepted_extended_key_usages = accepted.iter().map(|s| (*s).to_owned()).collect();
    policy.evaluate_pem(pem.as_bytes()).unwrap()
}

#[test]
fn trust_policy_checks_key_usage_and_extended_key_usage() {
    const CLIENT_AUTH: &str = "1.3.6.1.5.5.7.3.2";
    const CODE_SIGNING: &str = "1.3.6.1.5.5.7.3.3";

    let mut signing = KeyUsage::new();
    signing.digital_signature().non_repudiation();
    let mut client = ExtendedKeyUsage::new();
    client.client_auth();
    let signer = certificate(signing, Some(client));
    assert_eq!(evaluate(&signer, &[]), TrustResult::Trusted);
    assert_eq!(evaluate(&signer, &[CLIENT_AUTH]), TrustResult::Trusted);
    assert!(matches!(
        evaluate(&signer, &[CODE_SIGNING]),
        TrustResult::Untrusted(r) if r.contains("extendedKeyUsage")
    ));

    // Solo cifrado: no sirve para firmar, salvo que se desactive la regla.
    let mut encipher = KeyUsage::new();
    encipher.key_encipherment();
    let encryption_only = certificate(encipher, None);
    assert!(matches!(
        evaluate(&encryption_only, &[]),
        TrustResult::Untrusted(r) if r.contains("keyUsage")
    ));
    let mut policy = TrustPolicy::new();
    policy.require_signing_key_usage = false;
    policy.add_root_pem(encryption_only.as_bytes()).unwrap();
    policy.at = Some(1_735_689_600);
    assert_eq!(
        policy.evaluate_pem(encryption_only.as_bytes()).unwrap(),
        TrustResult::Trusted
    );

    // Sin extendedKeyUsage, cualquier uso extendido es aceptable (RFC 5280).
    let mut plain = KeyUsage::new();
    plain.digital_signature();
    assert_eq!(
        evaluate(&certificate(plain, None), &[CODE_SIGNING]),
        TrustResult::Trusted
    );
}
