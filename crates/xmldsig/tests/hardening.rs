//! Regresiones de seguridad y estabilidad ante firmas y XML hostiles: la
//! verificación procesa datos no confiables y no debe agotar la pila, la CPU
//! ni filtrar material sensible. Sin equivalente en la referencia.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "support/keys.rs"]
mod keys;

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use cpeforge_xmldsig::c14n::{self, Method};
use cpeforge_xmldsig::dom::Document;
use cpeforge_xmldsig::dsig::{EXC_C14N, SHA256, XmlSecurityDsig};
use cpeforge_xmldsig::key::{RSA_SHA256, XmlSecurityKey};
use cpeforge_xmldsig::sunat::SignedXml;
use cpeforge_xmldsig::{Error, XmlError};

/// Pila de 1 MiB (la mitad que la de un hilo de tokio): un desbordamiento aborta el proceso, así que cada caso se
/// ejecuta en un hilo con menos pila que la habitual.
fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

fn xpath_error(expr: String) -> Error {
    on_small_stack(move || {
        let doc = Document::parse("<r><a/></r>").unwrap();
        let method = Method::from_uri(c14n::C14N).unwrap();
        c14n::canonicalize_xpath(&doc, doc.document_node(), &method, &expr, &[]).unwrap_err()
    })
}

#[test]
fn hostile_xpath_expressions_fail_without_exhausting_the_stack() {
    // Todas caben en el límite de longitud: lo que las frena es la
    // profundidad y el número de operadores.
    for expr in [
        format!("{}true(){}", "(".repeat(5000), ")".repeat(5000)),
        format!("{}true(){}", "not(".repeat(3000), ")".repeat(3000)),
        format!("{}1", "-".repeat(8000)),
        format!("1{}", "+1".repeat(7000)),
        format!("true(){}", " or true()".repeat(1500)),
        format!("{}self::node(){}", "a[".repeat(5000), "]".repeat(5000)),
        format!("a{}", "|a".repeat(7000)),
    ] {
        assert!(matches!(xpath_error(expr), Error::XPath(_)));
    }
    let oversized = format!("1{}", "+1".repeat(40_000));
    assert!(matches!(xpath_error(oversized), Error::XPath(_)));
}

#[test]
fn reasonable_xpath_expressions_still_work() {
    let doc = Document::parse("<r><a/><b/></r>").unwrap();
    let method = Method::from_uri(c14n::C14N).unwrap();
    let expr = format!("not(self::b) and {}", "true() and ".repeat(100) + "true()");
    let out = c14n::canonicalize_xpath(&doc, doc.document_node(), &method, &expr, &[]).unwrap();
    assert_eq!(out, "<r><a></a></r>");
}

/// La expresión más compleja admitida se evalúa con una pila de 1 MiB.
#[test]
fn largest_allowed_xpath_expression_fits_a_small_stack() {
    let expr = format!("true(){}", " and true()".repeat(124));
    let out = on_small_stack(move || {
        let doc = Document::parse("<r/>").unwrap();
        let method = Method::from_uri(c14n::C14N).unwrap();
        c14n::canonicalize_xpath(&doc, doc.document_node(), &method, &expr, &[]).unwrap()
    });
    assert_eq!(out, "<r></r>");
}

/// Un `EncryptedKey` cuyo `KeyInfo` se referencia a sí mismo no debe provocar
/// recursión infinita al verificar una firma recibida.
#[test]
fn self_referencing_encrypted_key_is_rejected() {
    let xml = r##"<r xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:xenc="http://www.w3.org/2001/04/xmlenc#"><ds:Signature><ds:SignedInfo><ds:SignatureMethod Algorithm="http://www.w3.org/2000/09/xmldsig#rsa-sha1"/></ds:SignedInfo><ds:SignatureValue>AAAA</ds:SignatureValue><ds:KeyInfo><ds:RetrievalMethod URI="#k" Type="http://www.w3.org/2001/04/xmlenc#EncryptedKey"/></ds:KeyInfo></ds:Signature><xenc:EncryptedKey Id="k"><xenc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#rsa-1_5"/><ds:KeyInfo><ds:RetrievalMethod URI="#k" Type="http://www.w3.org/2001/04/xmlenc#EncryptedKey"/></ds:KeyInfo></xenc:EncryptedKey></r>"##;
    let error = on_small_stack(|| SignedXml::new().verify_xml(xml).unwrap_err());
    assert_eq!(
        error,
        Error::Xml(XmlError::LimitExceeded("anidamiento de EncryptedKey"))
    );
}

#[test]
fn namespace_work_is_bounded() {
    let wide = |decls: usize, kids: usize| {
        let mut xml = String::from("<r");
        for i in 0..decls {
            xml.push_str(&format!(" xmlns:p{i}=\"u{i}\""));
        }
        xml.push('>');
        xml.push_str(&"<c/>".repeat(kids));
        xml.push_str("</r>");
        xml
    };
    // Miles de declaraciones por miles de descendientes serían cuadráticas.
    assert_eq!(
        Document::parse(&wide(8000, 8000)).unwrap_err(),
        XmlError::LimitExceeded("namespaces")
    );
    // Muchas declaraciones en pocos elementos se canonicalizan en tiempo lineal.
    let doc = Document::parse(&wide(30_000, 3)).unwrap();
    let started = Instant::now();
    for uri in [c14n::C14N, c14n::EXC_C14N] {
        let method = Method::from_uri(uri).unwrap();
        let out = c14n::canonicalize(&doc, doc.document_node(), &method);
        assert!(out.starts_with("<r"));
    }
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn debug_output_hides_private_material() {
    let keys = keys::generate().unwrap();
    let pem = fs::read_to_string(keys.dir().join("a.pem")).unwrap();
    let body = pem
        .lines()
        .find(|l| l.len() > 40 && !l.starts_with("-----"))
        .unwrap()
        .to_owned();

    let mut signer = SignedXml::new();
    signer.set_certificate(&pem);
    signer.sign_xml("<a/>").unwrap();
    let shown = format!("{signer:?}");
    assert!(!shown.contains("PRIVATE KEY") && !shown.contains(&body));

    let mut key = XmlSecurityKey::new(RSA_SHA256, Some("private")).unwrap();
    key.passphrase = "clave-secreta-de-prueba".to_owned();
    key.load_key(pem.as_bytes(), false).unwrap();
    let shown = format!("{key:?}");
    assert!(!shown.contains("clave-secreta-de-prueba") && !shown.contains(&body));
}

/// Firma `#id` con un identificador único y la verifica con la política estricta.
fn sign_referenced_element(keys: &keys::EphemeralKeys) -> String {
    let mut doc = Document::parse("<r><item>1</item></r>").unwrap();
    let item = doc
        .descendants(doc.document_node())
        .into_iter()
        .find(|n| doc.local_name(*n) == Some("item"))
        .unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.set_canonical_method(&mut doc, EXC_C14N).unwrap();
    dsig.add_reference(&mut doc, item, SHA256, None, None)
        .unwrap();
    let mut key = XmlSecurityKey::new(RSA_SHA256, Some("private")).unwrap();
    key.load_key(
        fs::read_to_string(keys.dir().join("a.pem"))
            .unwrap()
            .as_bytes(),
        false,
    )
    .unwrap();
    let root = doc.document_element();
    dsig.sign(&mut doc, &key, root).unwrap();
    doc.to_xml()
}

fn validate(xml: &str, strict: bool) -> Result<bool, Error> {
    let mut doc = Document::parse(xml).unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.reject_duplicate_ids = strict;
    let root = doc.document_node();
    dsig.locate_signature(&doc, root, 0).unwrap();
    dsig.validate_reference(&mut doc)
}

#[test]
fn strict_mode_rejects_duplicate_reference_ids() {
    let keys = keys::generate().unwrap();
    let signed = sign_referenced_element(&keys);
    assert!(validate(&signed, false).unwrap());
    assert!(validate(&signed, true).unwrap());

    // Envoltura: un elemento falso con el mismo Id antes del firmado.
    let id = signed
        .split("URI=\"#")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap()
        .to_owned();
    let wrapped = signed.replacen("<item", &format!("<item Id=\"{id}\">forged</item><item"), 1);
    // Por defecto se usa el primero, como la referencia: el digest ya no coincide.
    assert_eq!(
        validate(&wrapped, false).unwrap_err(),
        Error::ReferenceValidationFailed
    );
    // En modo estricto se informa la ambigüedad.
    assert_eq!(
        validate(&wrapped, true).unwrap_err(),
        Error::AmbiguousReferenceId(id)
    );
}
