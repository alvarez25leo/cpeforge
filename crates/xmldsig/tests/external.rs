//! Contraste con implementaciones independientes: las firmas que genera el
//! crate se verifican fuera de él, con `xmllint` (canonicalización C14N de
//! libxml2) y el binario `openssl` (digests, RSA y HMAC). Sin red ni
//! dependencias nuevas: ambos binarios ya los usan otras pruebas del
//! repositorio.
//!
//! Cubre:
//! - referencia `URI=""` con transformación enveloped y C14N inclusiva (perfil
//!   SUNAT) con RSA-SHA1/256/384/512 y digests SHA-1/256/384/512 y RIPEMD-160;
//! - referencia por `Id` con C14N exclusiva, firmada con RSA-SHA512 y con
//!   HMAC-SHA1.
//!
//! La transformación XPath se contrasta con libxml2 en `external_xpath.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cpeforge_xmldsig::c14n::{C14N, EXC_C14N};
use cpeforge_xmldsig::dom::Document;
use cpeforge_xmldsig::dsig::{
    ENVELOPED, RIPEMD160, ReferenceOptions, SHA1, SHA256, SHA384, SHA512, Transform,
    XmlSecurityDsig,
};
use cpeforge_xmldsig::key::{
    HMAC_SHA1, RSA_SHA1, RSA_SHA256, RSA_SHA384, RSA_SHA512, XmlSecurityKey,
};
use cpeforge_xmldsig::sunat::SignedXml;
use std::fmt::Write as _;

const INVOICE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions><cbc:ID>F001-1</cbc:ID><cbc:Note><![CDATA[a & b]]></cbc:Note><cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount></Invoice>"#;

/// Directorio temporal propio de cada prueba.
fn workdir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "xmldsig-external-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(program: &str, args: &[&str]) -> Vec<u8> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("no se pudo ejecutar {program}: {e}"));
    assert!(
        output.status.success(),
        "{program} {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Clave RSA y certificado autofirmado generados por `openssl`.
fn identity(dir: &Path) -> (String, PathBuf) {
    let (key, cert, public) = (dir.join("k.pem"), dir.join("c.pem"), dir.join("pub.pem"));
    run(
        "openssl",
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=20000000001",
            "-keyout",
            path(&key),
            "-out",
            path(&cert),
        ],
    );
    let public_pem = run(
        "openssl",
        &["x509", "-in", path(&cert), "-pubkey", "-noout"],
    );
    fs::write(&public, public_pem).unwrap();
    let pem = fs::read_to_string(&key).unwrap() + &fs::read_to_string(&cert).unwrap();
    (pem, public)
}

/// Texto entre `<{tag}` (inclusive) y `</{tag}>` (inclusive).
fn element<'a>(xml: &'a str, tag: &str) -> &'a str {
    let start = xml
        .find(&format!("<{tag}"))
        .unwrap_or_else(|| panic!("sin <{tag}"));
    let close = format!("</{tag}>");
    let end = xml[start..].find(&close).unwrap() + start + close.len();
    &xml[start..end]
}

fn text_of<'a>(xml: &'a str, tag: &str) -> &'a str {
    let open = format!("<{tag}>");
    let start = xml.find(&open).unwrap() + open.len();
    let end = xml[start..].find(&format!("</{tag}>")).unwrap() + start;
    &xml[start..end]
}

/// Copia independiente de un subárbol: declara en su raíz los namespaces en
/// ámbito (`namespaces`), como hace C14N con el apex de un subconjunto.
fn with_namespaces(fragment: &str, namespaces: &[&str]) -> String {
    let name_end = fragment.find([' ', '>', '/']).unwrap();
    let declarations: String = namespaces.iter().fold(String::new(), |mut out, ns| {
        let _ = write!(out, " {ns}");
        out
    });
    format!(
        "{}{declarations}{}",
        &fragment[..name_end],
        &fragment[name_end..]
    )
}

/// C14N de libxml2 (`--c14n` inclusiva o `--exc-c14n`).
fn xmllint_c14n(dir: &Path, name: &str, xml: &str, exclusive: bool) -> Vec<u8> {
    let file = dir.join(name);
    fs::write(&file, xml).unwrap();
    let mode = if exclusive { "--exc-c14n" } else { "--c14n" };
    run("xmllint", &[mode, path(&file)])
}

fn openssl_digest(dir: &Path, name: &str, data: &[u8], digest: &str) -> String {
    let file = dir.join(name);
    fs::write(&file, data).unwrap();
    STANDARD.encode(run("openssl", &["dgst", digest, "-binary", path(&file)]))
}

fn openssl_verify(dir: &Path, data: &[u8], signature_b64: &str, digest: &str, public: &Path) {
    let (data_file, sig_file) = (dir.join("signedinfo.c14n"), dir.join("signature.bin"));
    fs::write(&data_file, data).unwrap();
    fs::write(&sig_file, STANDARD.decode(signature_b64.trim()).unwrap()).unwrap();
    let out = run(
        "openssl",
        &[
            "dgst",
            digest,
            "-verify",
            path(public),
            "-signature",
            path(&sig_file),
            path(&data_file),
        ],
    );
    assert_eq!(String::from_utf8_lossy(&out).trim(), "Verified OK");
}

const UBL_NAMESPACES: [&str; 3] = [
    r#"xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2""#,
    r#"xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2""#,
    r#"xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2""#,
];
const DS: &str = r#"xmlns:ds="http://www.w3.org/2000/09/xmldsig#""#;

/// Firma enveloped con C14N inclusiva. El perfil SUNAT (`SignedXml`) admite
/// RSA-SHA1/256 con SHA-1/256, como la referencia; el resto de algoritmos se
/// firma con la API genérica y la misma referencia `URI=""`.
fn sign_enveloped(pem: &str, signature: &str, digest: &str) -> String {
    if [RSA_SHA1, RSA_SHA256].contains(&signature) && [SHA1, SHA256].contains(&digest) {
        let mut signer = SignedXml::new();
        signer.set_certificate(pem);
        signer.set_signature_algorithm(signature).unwrap();
        signer.set_digest_algorithm(digest).unwrap();
        return signer.sign_xml(INVOICE).unwrap();
    }
    let mut doc = Document::parse(INVOICE).unwrap();
    let mut key = XmlSecurityKey::new(signature, Some("private")).unwrap();
    key.load_key(pem.as_bytes(), false).unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.set_canonical_method(&mut doc, C14N).unwrap();
    let mut options = ReferenceOptions::default();
    options.force_uri = true;
    let root = doc.document_node();
    dsig.add_reference(
        &mut doc,
        root,
        digest,
        Some(&[Transform::Algorithm(ENVELOPED.to_owned())]),
        Some(&options),
    )
    .unwrap();
    let target = doc.document_element();
    dsig.sign(&mut doc, &key, target).unwrap();
    doc.to_xml()
}

/// Enveloped con C14N inclusiva y cada algoritmo de firma y de digest:
/// digest y firma recalculados por libxml2 y OpenSSL.
#[test]
fn enveloped_signatures_verify_with_libxml2_and_openssl() {
    let dir = workdir("enveloped");
    let (pem, public) = identity(&dir);
    for (signature, digest, sig_dgst, ref_dgst) in [
        (RSA_SHA1, SHA1, "-sha1", "-sha1"),
        (RSA_SHA256, SHA256, "-sha256", "-sha256"),
        (RSA_SHA384, SHA384, "-sha384", "-sha384"),
        (RSA_SHA512, SHA512, "-sha512", "-sha512"),
        (RSA_SHA256, RIPEMD160, "-sha256", "-ripemd160"),
    ] {
        let signed = sign_enveloped(&pem, signature, digest);
        assert!(
            signed.contains(&format!("Algorithm=\"{digest}\"")),
            "{digest}"
        );

        // Referencia URI="": el documento sin la firma, canonizado por libxml2.
        let without_signature = signed.replacen(element(&signed, "ds:Signature"), "", 1);
        let canonical = xmllint_c14n(&dir, "document.xml", &without_signature, false);
        assert_eq!(
            openssl_digest(&dir, "document.c14n", &canonical, ref_dgst),
            text_of(&signed, "ds:DigestValue"),
            "{digest}"
        );

        // SignedInfo con los namespaces en ámbito, canonizado por libxml2 y
        // verificado por OpenSSL con la clave pública del certificado.
        let mut namespaces = UBL_NAMESPACES.to_vec();
        namespaces.push(DS);
        let signed_info = with_namespaces(element(&signed, "ds:SignedInfo"), &namespaces);
        let canonical = xmllint_c14n(&dir, "signedinfo.xml", &signed_info, false);
        openssl_verify(
            &dir,
            &canonical,
            text_of(&signed, "ds:SignatureValue"),
            sig_dgst,
            &public,
        );

        // Control negativo: un importe alterado ya no coincide.
        let tampered = without_signature.replace("118.00", "1.00");
        let canonical = xmllint_c14n(&dir, "tampered.xml", &tampered, false);
        assert_ne!(
            openssl_digest(&dir, "tampered.c14n", &canonical, ref_dgst),
            text_of(&signed, "ds:DigestValue")
        );
    }
}

/// Firma de un elemento por `Id` con C14N exclusiva.
fn sign_by_id(key: &XmlSecurityKey, digest: &str) -> String {
    let mut doc = Document::parse(
        r#"<r xmlns:a="urn:a" xmlns:b="urn:no-usado"><a:item>uno &amp; dos</a:item><otro>x</otro></r>"#,
    )
    .unwrap();
    let item = doc
        .descendants(doc.document_node())
        .into_iter()
        .find(|n| doc.local_name(*n) == Some("item"))
        .unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.set_canonical_method(&mut doc, EXC_C14N).unwrap();
    dsig.add_reference(&mut doc, item, digest, None, None)
        .unwrap();
    let root = doc.document_element();
    dsig.sign(&mut doc, key, root).unwrap();
    doc.to_xml()
}

/// Referencia por `Id` (C14N exclusiva): digest del elemento y firma RSA o
/// HMAC recalculados por libxml2 y OpenSSL.
#[test]
fn id_references_with_rsa_and_hmac_verify_externally() {
    let dir = workdir("id");
    let (pem, public) = identity(&dir);

    let mut rsa = XmlSecurityKey::new(RSA_SHA512, Some("private")).unwrap();
    rsa.load_key(pem.as_bytes(), false).unwrap();
    let signed = sign_by_id(&rsa, SHA512);
    let item = with_namespaces(element(&signed, "a:item"), &[r#"xmlns:a="urn:a""#]);
    assert!(item.contains(" Id=\""), "{signed}");
    let canonical = xmllint_c14n(&dir, "item.xml", &item, true);
    assert_eq!(
        openssl_digest(&dir, "item.c14n", &canonical, "-sha512"),
        text_of(&signed, "ds:DigestValue")
    );
    let signed_info = with_namespaces(element(&signed, "ds:SignedInfo"), &[DS]);
    let canonical = xmllint_c14n(&dir, "si-rsa.xml", &signed_info, true);
    openssl_verify(
        &dir,
        &canonical,
        text_of(&signed, "ds:SignatureValue"),
        "-sha512",
        &public,
    );

    let secret = "clave-hmac-de-prueba";
    let mut hmac = XmlSecurityKey::new(HMAC_SHA1, None).unwrap();
    hmac.load_key(secret.as_bytes(), false).unwrap();
    let signed = sign_by_id(&hmac, SHA256);
    assert!(signed.contains(&format!("Algorithm=\"{HMAC_SHA1}\"")));
    let item = with_namespaces(element(&signed, "a:item"), &[r#"xmlns:a="urn:a""#]);
    let canonical = xmllint_c14n(&dir, "item-hmac.xml", &item, true);
    assert_eq!(
        openssl_digest(&dir, "item-hmac.c14n", &canonical, "-sha256"),
        text_of(&signed, "ds:DigestValue")
    );
    let signed_info = with_namespaces(element(&signed, "ds:SignedInfo"), &[DS]);
    let canonical = xmllint_c14n(&dir, "si-hmac.xml", &signed_info, true);
    let data = dir.join("si-hmac.c14n");
    fs::write(&data, &canonical).unwrap();
    let mac = run(
        "openssl",
        &[
            "mac",
            "-digest",
            "SHA1",
            "-macopt",
            &format!("key:{secret}"),
            "-binary",
            "-in",
            path(&data),
            "HMAC",
        ],
    );
    assert_eq!(
        STANDARD.encode(mac),
        text_of(&signed, "ds:SignatureValue").trim()
    );
}
