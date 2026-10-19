//! Firma y verifica un comprobante UBL mínimo con el perfil SUNAT.
//!
//! La clave y el certificado se generan en memoria en cada ejecución y no se
//! imprimen. Muestra la diferencia entre la validez matemática de la firma
//! (`SignedXml::verify_xml`) y la confianza en el certificado
//! (`trust::TrustPolicy`).

use std::error::Error;

use cpeforge_xmldsig::dsig::SHA256;
use cpeforge_xmldsig::key::RSA_SHA256;
use cpeforge_xmldsig::sunat::SignedXml;
use cpeforge_xmldsig::trust::TrustPolicy;
use openssl::asn1::Asn1Time;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::x509::{X509, X509NameBuilder};

const INVOICE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions><cbc:ID>F001-1</cbc:ID><cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount></Invoice>"#;

/// Certificado autofirmado efímero (clave + certificado en PEM).
fn ephemeral_identity() -> Result<(String, String), Box<dyn Error>> {
    let pkey = PKey::from_rsa(Rsa::generate(2048)?)?;
    let mut name = X509NameBuilder::new()?;
    name.append_entry_by_text("C", "PE")?;
    name.append_entry_by_text("CN", "20000000001")?;
    let name = name.build();
    let mut builder = X509::builder()?;
    builder.set_version(2)?;
    builder.set_subject_name(&name)?;
    builder.set_issuer_name(&name)?;
    builder.set_pubkey(&pkey)?;
    let not_before = Asn1Time::days_from_now(0)?;
    let not_after = Asn1Time::days_from_now(30)?;
    builder.set_not_before(&not_before)?;
    builder.set_not_after(&not_after)?;
    builder.sign(&pkey, MessageDigest::sha256())?;
    let cert = String::from_utf8(builder.build().to_pem()?)?;
    let key = String::from_utf8(pkey.private_key_to_pem_pkcs8()?)?;
    Ok((key, cert))
}

fn main() -> Result<(), Box<dyn Error>> {
    let (key, cert) = ephemeral_identity()?;

    // Perfil SUNAT por defecto: RSA-SHA1 + SHA-1 (compatibilidad con la
    // referencia de referencia). Aquí se elige RSA-SHA256 + SHA-256.
    let mut signer = SignedXml::new();
    signer.set_certificate(&format!("{key}{cert}"));
    signer.set_signature_algorithm(RSA_SHA256)?;
    signer.set_digest_algorithm(SHA256)?;
    let signed = signer.sign_xml(INVOICE)?;
    println!("firmado: {} bytes", signed.len());

    // Validez matemática: firma y digest, con el certificado incluido.
    let valid = SignedXml::new().verify_xml(&signed)?;
    println!("firma y referencias válidas: {valid}");
    let tampered = signed.replace("118.00", "1.00");
    println!(
        "documento alterado válido: {}",
        SignedXml::new().verify_xml(&tampered)?
    );

    // Confianza: el certificado autofirmado no es confiable sin raíz explícita.
    println!(
        "confianza sin raíces: {:?}",
        TrustPolicy::new().evaluate_pem(cert.as_bytes())?
    );
    let mut policy = TrustPolicy::new();
    policy.add_root_pem(cert.as_bytes())?;
    println!(
        "confianza con la raíz configurada: {:?}",
        policy.evaluate_pem(cert.as_bytes())?
    );
    Ok(())
}
