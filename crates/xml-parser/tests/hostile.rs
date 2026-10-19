//! Entradas hostiles y mutaciones: ningún parser debe entrar en pánico,
//! agotar la pila ni aceptar DTD; los límites se aplican antes de analizar.

use std::fs;
use std::path::Path;

use cpeforge_core::ports::DocumentParser;
use cpeforge_xml_parser::{
    DespatchParser, InvoiceParser, Limits, NoteParser, ParseOptions, PerceptionParser,
    ReceiptParser, RetentionParser, SummaryParser, VoidedParser, XmlDocument, XmlParserError,
    XmlReader,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// xorshift64*: generador determinista sin dependencias.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
    }
}

fn run_all(bytes: &[u8]) {
    let options = ParseOptions::new().with_fixed_now(jiff::Timestamp::UNIX_EPOCH);
    let _ = InvoiceParser::with_options(options.clone()).parse(bytes);
    let _ = NoteParser::with_options(options.clone()).parse(bytes);
    let _ = DespatchParser::with_options(options.clone()).parse(bytes);
    let _ = ReceiptParser::with_options(options.clone()).parse(bytes);
    let _ = SummaryParser::with_options(options.clone()).parse(bytes);
    let _ = VoidedParser::with_options(options.clone()).parse(bytes);
    let _ = RetentionParser::with_options(options.clone()).parse(bytes);
    let _ = PerceptionParser::with_options(options).parse(bytes);
    if let Ok(reader) = XmlReader::load_xml(bytes) {
        for query in [
            "cbc:ID",
            "//cbc:ID",
            "*/*",
            "@x",
            "cac:Nada/zz:x",
            "a[",
            "//",
            "/*/@*",
        ] {
            let _ = reader.nodes(query, None);
            let _ = reader.value(query, None);
        }
    }
}

fn seeds() -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference/xml-parser/cases.json");
    let cases: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(cases["cases"]
        .as_array()
        .ok_or("sin casos")?
        .iter()
        .filter_map(|c| c["input"]["xml"].as_str())
        .filter(|xml| xml.len() < 64 * 1024)
        .map(|xml| xml.as_bytes().to_vec())
        .collect())
}

#[test]
fn mutated_inputs_never_panic() -> TestResult {
    let seeds = seeds()?;
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let tokens: [&[u8]; 10] = [
        b"<",
        b">",
        b"</",
        b"/>",
        b"&",
        b"&amp;",
        b"<![CDATA[",
        b"<!DOCTYPE r [<!ENTITY e 'x'>]>",
        b"\xFF",
        b"cbc:",
    ];
    for round in 0..3000 {
        let mut bytes = seeds[round % seeds.len()].clone();
        for _ in 0..=rng.below(4) {
            let at = rng.below(bytes.len() + 1);
            match rng.below(5) {
                0 => bytes.truncate(at),
                1 => {
                    let token = tokens[rng.below(tokens.len())];
                    bytes.splice(at..at, token.iter().copied());
                }
                2 if at < bytes.len() => bytes[at] ^= 1 << rng.below(8),
                3 if at < bytes.len() => {
                    let end = (at + rng.below(64)).min(bytes.len());
                    bytes.drain(at..end);
                }
                _ => {
                    let end = (at + rng.below(128)).min(bytes.len());
                    let chunk = bytes[at..end].to_vec();
                    bytes.splice(at..at, chunk);
                }
            }
        }
        run_all(&bytes);
    }
    Ok(())
}

#[test]
fn depth_is_bounded_before_parsing() {
    let limit = Limits::default().max_depth;
    let nested = |depth: usize, open: &str, close: &str| {
        format!("{}{}", open.repeat(depth), close.repeat(depth))
    };
    assert!(XmlDocument::parse(nested(limit, "<a>", "</a>").as_bytes()).is_ok());
    for (open, close) in [
        ("<a>", "</a>"),
        ("<a x='>'>", "</a>"),
        ("<a><!--c-->", "</a>"),
        ("<a><![CDATA[x]]>", "</a>"),
    ] {
        let err = XmlDocument::parse(nested(limit + 1, open, close).as_bytes()).err();
        assert_eq!(err, Some(XmlParserError::TooDeep { limit }), "{open}");
    }
    // Un millón de aperturas sin cierre: se rechaza sin recursión.
    let open = "<a>".repeat(1_000_000);
    assert!(matches!(
        XmlDocument::parse(open.as_bytes()),
        Err(XmlParserError::TooDeep { .. })
    ));
    // Cierres desparejados no rebajan la cota lo bastante para desbordar:
    // el analizador falla en el primer cierre que no corresponde.
    let tricky = format!("{}</b>{}", "<a>".repeat(limit), "<a>".repeat(limit));
    assert!(XmlDocument::parse(tricky.as_bytes()).is_err());
    // El elemento vacío del nivel `limit + 1` también cuenta.
    let leaf = format!("{}<b/>{}", "<a>".repeat(limit), "</a>".repeat(limit));
    assert_eq!(
        XmlDocument::parse(leaf.as_bytes()).err(),
        Some(XmlParserError::TooDeep { limit })
    );
    // Una etiqueta sin cerrar en el nivel `limit + 1` detiene el pre-escaneo:
    // el parser la rechaza sin recurrir más.
    let open_tag = format!("{}<b", "<a>".repeat(limit));
    assert!(matches!(
        XmlDocument::parse(open_tag.as_bytes()),
        Err(XmlParserError::Malformed { .. })
    ));
}

#[test]
fn entities_and_dtd_are_rejected() {
    let mut laughs = String::from("<!DOCTYPE r [<!ENTITY a 'lol'>");
    for i in 0..9 {
        let name = char::from(b'b' + i);
        let prev = char::from(b'a' + i);
        laughs.push_str(&format!(
            "<!ENTITY {name} '{}'>",
            format!("&{prev};").repeat(10)
        ));
    }
    laughs.push_str("]><r>&j;</r>");
    for xml in [
        laughs.as_str(),
        "<!DOCTYPE r [<!ENTITY e SYSTEM 'file:///etc/passwd'>]><r>&e;</r>",
        "<!DOCTYPE r [<!ENTITY % p SYSTEM 'http://127.0.0.1/x'> %p;]><r/>",
    ] {
        assert_eq!(
            XmlDocument::parse(xml.as_bytes()).err(),
            Some(XmlParserError::Dtd),
            "{xml}"
        );
    }
    // Referencia a entidad no declarada: mal formado.
    assert!(matches!(
        XmlDocument::parse(b"<r>&e;</r>"),
        Err(XmlParserError::Malformed { .. })
    ));
}

#[test]
fn size_and_node_limits() {
    let limits = Limits {
        max_bytes: 1024,
        max_depth: 8,
        max_nodes: 50,
    };
    let big = format!("<r>{}</r>", "x".repeat(2000));
    assert!(matches!(
        XmlDocument::parse_with_limits(big.as_bytes(), &limits),
        Err(XmlParserError::TooLarge { .. })
    ));
    let many = format!("<r>{}</r>", "<a/>".repeat(100));
    assert!(matches!(
        XmlDocument::parse_with_limits(many.as_bytes(), &limits),
        Err(XmlParserError::TooManyNodes { limit: 50 })
    ));
    let options = ParseOptions::new().with_limits(limits);
    let err = VoidedParser::with_options(options)
        .parse(big.as_bytes())
        .err();
    assert!(matches!(
        err,
        Some(XmlParserError::TooLarge {
            size: 2007,
            limit: 1024
        })
    ));
}

#[test]
fn context_from_other_document_is_rejected() -> TestResult {
    let a = XmlReader::load_xml(b"<r><x>1</x></r>")?;
    let b = XmlReader::load_xml(b"<r><x>2</x></r>")?;
    let node = b.node("x", None)?;
    assert!(matches!(
        a.value("x", node),
        Err(XmlParserError::Query { .. })
    ));
    Ok(())
}
