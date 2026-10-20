//! XML Encryption endurecido (revisión de seguridad): AES-GCM con RSA-OAEP,
//! algoritmos heredados rechazados por defecto y un solo error de descifrado.
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
use cpeforge_xmldsig::enc::{Decrypted, ELEMENT, XmlSecEnc};
use cpeforge_xmldsig::key::{
    AES128_CBC, AES256_GCM, RSA_1_5, RSA_OAEP_MGF1P, XmlSecurityKey, is_legacy_encryption,
};

const XML: &str = "<r><secret>datos</secret></r>";

fn secret(doc: &Document) -> cpeforge_xmldsig::dom::NodeId {
    doc.descendants(doc.document_node())
        .into_iter()
        .find(|n| doc.local_name(*n) == Some("secret"))
        .unwrap()
}

/// Cifra `<secret>` con `session` y transporta la clave con `transport`.
fn encrypt(
    session_alg: &str,
    transport: &mut XmlSecurityKey,
    legacy: bool,
) -> Result<String, Error> {
    let mut doc = Document::parse(XML).unwrap();
    let mut session = XmlSecurityKey::new(session_alg, None)?;
    session.generate_session_key()?;
    let mut enc = XmlSecEnc::new();
    enc.allow_legacy_algorithms = legacy;
    enc.set_node(secret(&doc));
    enc.kind = Some(ELEMENT.to_owned());
    enc.encrypt_key(transport, &session, true)?;
    enc.encrypt_node(&mut doc, &mut session, true)?;
    Ok(doc.to_xml())
}

fn decrypt(xml: &str, private_pem: &[u8]) -> Result<String, Error> {
    let mut doc = Document::parse(xml).unwrap();
    let data = XmlSecEnc::locate_encrypted_data(&doc).unwrap();
    let mut enc = XmlSecEnc::new();
    enc.set_node(data);
    enc.kind = Some(ELEMENT.to_owned());
    let key = enc.locate_key(&doc, None).unwrap();
    let algorithm = key.algorithm();
    let mut info = enc.locate_key_info(&doc, Some(key), None)?.unwrap();
    info.load_key(private_pem, false)?;
    let session = info.decrypt_encrypted_key(&mut doc)?;
    let mut key = XmlSecurityKey::new(algorithm, None)?;
    key.load_key(&session, false)?;
    match enc.decrypt_node(&mut doc, &mut key, true)? {
        Decrypted::Node(_) | Decrypted::Parent(_) => Ok(doc.to_xml()),
        other => panic!("{other:?}"),
    }
}

fn transport(keys: &keys::EphemeralKeys, algorithm: &str) -> XmlSecurityKey {
    let mut key = XmlSecurityKey::new(algorithm, Some("public")).unwrap();
    key.load_key(&fs::read(keys.dir().join("a.crt")).unwrap(), true)
        .unwrap();
    key
}

#[test]
fn aes_gcm_with_rsa_oaep_round_trips() {
    let keys = keys::generate().unwrap();
    let xml = encrypt(AES256_GCM, &mut transport(&keys, RSA_OAEP_MGF1P), false).unwrap();
    assert!(!xml.contains("datos") && xml.contains("xmlenc11#aes256-gcm"));
    let private = fs::read(keys.dir().join("a.key")).unwrap();
    assert!(decrypt(&xml, &private).unwrap().contains(XML));
}

#[test]
fn legacy_algorithms_are_rejected_unless_allowed() {
    assert!(is_legacy_encryption(AES128_CBC) && is_legacy_encryption(RSA_1_5));
    assert!(!is_legacy_encryption(AES256_GCM) && !is_legacy_encryption(RSA_OAEP_MGF1P));
    let keys = keys::generate().unwrap();
    assert_eq!(
        encrypt(AES128_CBC, &mut transport(&keys, RSA_OAEP_MGF1P), false).unwrap_err(),
        Error::LegacyEncryptionAlgorithm(AES128_CBC.to_owned())
    );
    assert_eq!(
        encrypt(AES256_GCM, &mut transport(&keys, RSA_1_5), false).unwrap_err(),
        Error::LegacyEncryptionAlgorithm(RSA_1_5.to_owned())
    );
    // Un documento heredado (compatible con la referencia) no se descifra
    // sin activarlo explícitamente.
    let legacy = encrypt(AES128_CBC, &mut transport(&keys, RSA_1_5), true).unwrap();
    let private = fs::read(keys.dir().join("a.key")).unwrap();
    assert_eq!(
        decrypt(&legacy, &private).unwrap_err(),
        Error::LegacyEncryptionAlgorithm(RSA_1_5.to_owned())
    );
}

#[test]
fn tampering_and_wrong_keys_give_one_opaque_error() {
    let keys = keys::generate().unwrap();
    let xml = encrypt(AES256_GCM, &mut transport(&keys, RSA_OAEP_MGF1P), false).unwrap();
    let private = fs::read(keys.dir().join("a.key")).unwrap();

    // Otra clave privada: falla el transporte (RSA-OAEP).
    let other = keys::generate().unwrap();
    let wrong = fs::read(other.dir().join("a.key")).unwrap();
    assert_eq!(decrypt(&xml, &wrong).unwrap_err(), Error::DecryptionFailed);

    // Cada CipherValue alterado en un byte: falla la etiqueta GCM o el OAEP.
    let values: Vec<&str> = xml
        .split("CipherValue>")
        .skip(1)
        .step_by(2)
        .map(|v| v.trim_end_matches("</xenc:"))
        .collect();
    assert_eq!(values.len(), 2);
    for value in values {
        let flipped = if value.starts_with('A') { "B" } else { "A" };
        let tampered = xml.replacen(value, &format!("{flipped}{}", &value[1..]), 1);
        assert_eq!(
            decrypt(&tampered, &private).unwrap_err(),
            Error::DecryptionFailed
        );
    }

    // Datos demasiado cortos para IV y etiqueta.
    let mut key = XmlSecurityKey::new(AES256_GCM, None).unwrap();
    key.generate_session_key().unwrap();
    assert_eq!(
        key.decrypt_data(&[0; 27]).unwrap_err(),
        Error::DecryptionFailed
    );
}
