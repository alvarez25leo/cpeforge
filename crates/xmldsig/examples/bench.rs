//! Medición reproducible de firma y verificación (`cargo run --release --example bench`).
//! Cada cifra es el promedio de N iteraciones tras una de calentamiento.
//!
//! Genera una identidad efímera y facturas UBL sintéticas de distinto tamaño;
//! no usa claves reales ni red.

use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use cpeforge_xmldsig::c14n::{self, Method};
use cpeforge_xmldsig::dom::Document;
use cpeforge_xmldsig::dsig::SHA256;
use cpeforge_xmldsig::key::RSA_SHA256;
use cpeforge_xmldsig::sunat::SignedXml;
use openssl::asn1::Asn1Time;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::x509::{X509, X509NameBuilder};

fn identity() -> Result<String, Box<dyn Error>> {
    let pkey = PKey::from_rsa(Rsa::generate(2048)?)?;
    let mut name = X509NameBuilder::new()?;
    name.append_entry_by_text("CN", "20000000001")?;
    let name = name.build();
    let mut b = X509::builder()?;
    b.set_version(2)?;
    b.set_subject_name(&name)?;
    b.set_issuer_name(&name)?;
    b.set_pubkey(&pkey)?;
    let (nb, na) = (Asn1Time::days_from_now(0)?, Asn1Time::days_from_now(30)?);
    b.set_not_before(&nb)?;
    b.set_not_after(&na)?;
    b.sign(&pkey, MessageDigest::sha256())?;
    let cert = String::from_utf8(b.build().to_pem()?)?;
    let key = String::from_utf8(pkey.private_key_to_pem_pkcs8()?)?;
    Ok(format!("{key}{cert}"))
}

fn invoice(lines: usize) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\" xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\" xmlns:ext=\"urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2\"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions><cbc:ID>F001-1</cbc:ID>",
    );
    for i in 0..lines {
        s.push_str(&format!(
            "<cac:InvoiceLine><cbc:ID>{i}</cbc:ID><cbc:InvoicedQuantity unitCode=\"NIU\">2</cbc:InvoicedQuantity><cbc:LineExtensionAmount currencyID=\"PEN\">100.00</cbc:LineExtensionAmount><cac:Item><cbc:Description>Producto &amp; servicio {i}</cbc:Description></cac:Item><cac:Price><cbc:PriceAmount currencyID=\"PEN\">50.00</cbc:PriceAmount></cac:Price></cac:InvoiceLine>"
        ));
    }
    s.push_str("</Invoice>");
    s
}

fn time<T>(label: &str, iters: u32, mut f: impl FnMut() -> T) {
    black_box(f());
    let t = Instant::now();
    for _ in 0..iters {
        black_box(f());
    }
    let per = t.elapsed() / iters;
    println!("{label:<44} {:>10} µs/op", per.as_micros());
}

fn main() -> Result<(), Box<dyn Error>> {
    let pem = identity()?;
    // `bench memoria`: solo firma y verificación de la factura grande, para
    // medir el pico de memoria (`/usr/bin/time -l`).
    if std::env::args().nth(1).as_deref() == Some("memoria") {
        let xml = invoice(2000);
        let mut signer = SignedXml::new();
        signer.set_certificate(&pem);
        let signed = signer.sign_xml(&xml)?;
        for _ in 0..5 {
            black_box(SignedXml::new().verify_xml(&signed)?);
        }
        return Ok(());
    }
    for lines in [5usize, 200, 2000] {
        let xml = invoice(lines);
        println!("== factura de {lines} líneas ({} KiB) ==", xml.len() / 1024);
        let iters = if lines >= 2000 { 10 } else { 50 };
        for (name, sha256) in [("RSA-SHA1", false), ("RSA-SHA256", true)] {
            let mut signer = SignedXml::new();
            if sha256 {
                signer.set_signature_algorithm(RSA_SHA256)?;
                signer.set_digest_algorithm(SHA256)?;
            }
            signer.set_certificate(&pem);
            let signed = signer.sign_xml(&xml)?;
            time(
                &format!("{name} sign_xml (parse+firma+serializar)"),
                iters,
                || signer.sign_xml(&xml).map(|s| s.len()),
            );
            time(&format!("{name} verify_xml"), iters, || {
                SignedXml::new().verify_xml(&signed)
            });
        }
        let doc = Document::parse(&xml)?;
        time("Document::parse", iters, || {
            Document::parse(&xml).map(|d| d.document_node())
        });
        time("Document::to_xml", iters, || doc.to_xml().len());
        let m = Method::from_uri(c14n::C14N).ok_or("c14n")?;
        time("c14n inclusivo documento", iters, || {
            c14n::canonicalize(&doc, doc.document_node(), &m).len()
        });
        let m = Method::from_uri(c14n::EXC_C14N).ok_or("exc")?;
        time("c14n exclusivo documento", iters, || {
            c14n::canonicalize(&doc, doc.document_node(), &m).len()
        });
    }
    Ok(())
}
