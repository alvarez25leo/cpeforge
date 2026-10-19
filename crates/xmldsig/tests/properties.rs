//! Pruebas de propiedades y fuzzing determinista (sin dependencias extra):
//! analizador, serialización, C14N y firma SUNAT sobre documentos aleatorios,
//! y conservación de los datos de los comprobantes UBL al firmar.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

#[path = "support/keys.rs"]
mod keys;

use std::fs;
use std::path::Path;

use cpeforge_xmldsig::c14n::{self, C14N, EXC_C14N, Method};
use cpeforge_xmldsig::dom::Document;
use cpeforge_xmldsig::dsig::XMLDSIGNS;
use cpeforge_xmldsig::sunat::SignedXml;

/// xorshift64*: reproducible a partir de la semilla.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n as u64)).unwrap()
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const TEXTS: [&str; 10] = [
    "a",
    "&amp;",
    "&lt;x&gt;",
    "\"q\"",
    "'s'",
    "ñandú",
    "😀",
    " \t ",
    "\r\n",
    "]]&gt;",
];
const ATTR_TEXTS: [&str; 9] = [
    "a",
    "&amp;",
    "&lt;x&gt;",
    "&quot;q&quot;",
    "'s'",
    "ñandú",
    "😀",
    " \t ",
    "&#10;&#13;&#9;",
];
const NAMES: [&str; 5] = ["a", "b", "x:c", "y:d", "e"];

fn element(rng: &mut Rng, depth: usize, out: &mut String) {
    let name = rng.pick(&NAMES);
    out.push('<');
    out.push_str(name);
    if depth == 0 {
        out.push_str(" xmlns=\"urn:d\" xmlns:x=\"urn:x\" xmlns:y=\"urn:y\"");
    }
    if depth > 0 && rng.below(3) == 0 {
        out.push_str(" xmlns:x=\"urn:x2\"");
    }
    for (i, attr) in ["k", "x:k", "z"].iter().enumerate() {
        if rng.below(2) == 0 {
            out.push_str(&format!(" {attr}=\"{}{i}\"", rng.pick(&ATTR_TEXTS)));
        }
    }
    out.push('>');
    for _ in 0..rng.below(4) {
        match rng.below(6) {
            0 if depth < 6 => element(rng, depth + 1, out),
            1 => out.push_str("<!-- c -->"),
            2 => out.push_str("<![CDATA[ <c> & ]]>"),
            3 => out.push_str("<?pi data?>"),
            _ => out.push_str(rng.pick(&TEXTS)),
        }
    }
    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

fn random_document(rng: &mut Rng) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>");
    if rng.below(2) == 0 {
        out.push_str("<!-- antes -->");
    }
    element(rng, 0, &mut out);
    out
}

#[test]
fn serialization_and_c14n_are_stable() {
    let mut rng = Rng(0x5EED_0001);
    for _ in 0..2000 {
        let xml = random_document(&mut rng);
        let doc = Document::parse(&xml).unwrap_or_else(|e| panic!("{e}: {xml}"));
        let once = doc.to_xml();
        let twice = Document::parse(&once).unwrap().to_xml();
        assert_eq!(once, twice, "serialización no estable: {xml}");
        for method in [C14N, EXC_C14N] {
            let m = Method::from_uri(method).unwrap();
            let canonical = c14n::canonicalize(&doc, doc.document_node(), &m);
            let again = Document::parse(&canonical).unwrap();
            assert_eq!(
                canonical,
                c14n::canonicalize(&again, again.document_node(), &m),
                "C14N no idempotente: {xml}"
            );
        }
    }
}

/// `(.//. | .//@* | .//namespace::*)[true()]` es el mismo conjunto que usa
/// la C14N sin XPath: las dos rutas (evaluador y visibilidad directa) deben
/// coincidir. Además, expresiones aleatorias nunca entran en pánico.
#[test]
fn xpath_true_matches_plain_c14n_and_fuzzed_expressions_do_not_panic() {
    let mut rng = Rng(0x5EED_0004);
    let ns = [
        ("x".to_owned(), "urn:x".to_owned()),
        ("d".to_owned(), "urn:d".to_owned()),
    ];
    let pieces = [
        "self::*",
        "self::text()",
        "not(",
        ")",
        " or ",
        " and ",
        "ancestor::x:c",
        "@k",
        "../@*",
        "namespace::*",
        "position()",
        "last()",
        " = ",
        " != ",
        " < ",
        "1",
        "'a'",
        "count(.)",
        "string(.)",
        "[1]",
        "following::*",
        "preceding-sibling::node()",
        "local-name()",
        "name()",
        "lang('es')",
        " | ",
        "//d:a",
        "sum(@*)",
        " div ",
        "-",
    ];
    for _ in 0..300 {
        let xml = random_document(&mut rng);
        let doc = Document::parse(&xml).unwrap();
        let root = doc.document_element().unwrap();
        for method in [C14N, EXC_C14N] {
            let m = Method::from_uri(method).unwrap();
            for node in [doc.document_node(), root] {
                let plain = c14n::canonicalize(&doc, node, &m);
                let via_xpath = c14n::canonicalize_xpath(&doc, node, &m, "true()", &ns).unwrap();
                assert_eq!(plain, via_xpath, "{method}: {xml}");
            }
        }
        let mut expr = String::new();
        for _ in 0..1 + rng.below(6) {
            expr.push_str(rng.pick(&pieces));
        }
        let m = Method::from_uri(C14N).unwrap();
        let _ = c14n::canonicalize_xpath(&doc, doc.document_node(), &m, &expr, &ns);
    }
}

#[test]
fn parser_never_panics_on_mutations() {
    let mut rng = Rng(0x5EED_0002);
    let alphabet = [
        "<", ">", "&", ";", "\"", "'", "=", "/", "!", "?", "[", "]", "-", ":", "x", " ", "#",
        "\u{0}", "\u{FFFE}", "é",
    ];
    for _ in 0..5000 {
        let mut xml = random_document(&mut rng);
        for _ in 0..1 + rng.below(4) {
            let mut pos = rng.below(xml.len() + 1);
            while !xml.is_char_boundary(pos) {
                pos -= 1;
            }
            match rng.below(3) {
                0 => xml.insert_str(pos, rng.pick(&alphabet)),
                1 => {
                    let end = (pos + 1 + rng.below(4)).min(xml.len());
                    if xml.is_char_boundary(end) {
                        xml.replace_range(pos..end, "");
                    }
                }
                _ => xml.truncate(pos),
            }
        }
        // Basta con que no entre en pánico; si analiza, debe reserializar.
        if let Ok(doc) = Document::parse(&xml) {
            Document::parse(&doc.to_xml())
                .unwrap_or_else(|e| panic!("reserialización inválida ({e}): {xml}"));
        }
    }
}

#[test]
fn random_documents_sign_and_detect_tampering() {
    let keys = keys::generate().unwrap();
    let mut signer = SignedXml::new();
    signer.set_certificate(&fs::read_to_string(keys.dir().join("a.pem")).unwrap());
    let mut rng = Rng(0x5EED_0003);
    for _ in 0..150 {
        let xml = random_document(&mut rng);
        let signed = signer.sign_xml(&xml).unwrap();
        assert!(
            SignedXml::new().verify_xml(&signed).unwrap(),
            "firma propia no verifica: {xml}"
        );
        // Alterar un atributo del contenido firmado.
        if let Some(pos) = signed.find(" k=\"") {
            let mut tampered = signed.clone();
            tampered.insert(pos + 4, 'Z');
            assert!(
                !SignedXml::new().verify_xml(&tampered).unwrap(),
                "alteración no detectada: {xml}"
            );
        }
    }
}

/// Firmar no cambia el comprobante: al retirar `Signature`, el documento es
/// idéntico al original serializado.
#[test]
fn signing_preserves_ubl_documents() {
    let keys = keys::generate().unwrap();
    let mut signer = SignedXml::new();
    signer.set_certificate(&fs::read_to_string(keys.dir().join("a.pem")).unwrap());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/xml/expected");
    let mut checked = 0;
    for family in fs::read_dir(&dir).unwrap() {
        let family = family.unwrap().path();
        if !family.is_dir() {
            continue;
        }
        for file in fs::read_dir(&family).unwrap() {
            let path = file.unwrap().path();
            let Ok(original) = Document::parse(&fs::read_to_string(&path).unwrap()) else {
                continue;
            };
            let Ok(signed) = signer.sign_xml(&original.to_xml()) else {
                continue;
            };
            let mut doc = Document::parse(&signed).unwrap();
            let sig = doc.descendants_ns(doc.document_node(), Some(XMLDSIGNS), "Signature")[0];
            doc.remove_child(sig);
            assert_eq!(
                doc.to_xml(),
                original.to_xml(),
                "la firma alteró {}",
                path.display()
            );
            assert!(
                SignedXml::new().verify_xml(&signed).unwrap(),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "solo {checked} documentos");
    println!("{checked} comprobantes UBL firmados sin cambios en sus datos");
}
