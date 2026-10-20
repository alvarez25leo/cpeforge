//! Contraste de la transformación XPath con una implementación independiente:
//! libxml2 (XPath 1.0 y `xmlC14NDocDumpMemory` con conjunto de nodos, el
//! motor que usa xmlsec), invocada mediante `ctypes` desde
//! `tests/support/libxml2_xpath_c14n.py` con la libxml2 del sistema.
//!
//! Para cada documento del corpus, expresión y método C14N, la salida de
//! [`canonicalize_xpath`] debe coincidir byte a byte con la de libxml2, o ambos
//! deben rechazar la expresión. Además, una firma con transformación XPath
//! generada por el crate se verifica fuera de él: el digest de la salida de
//! libxml2 calculado con `openssl` debe ser el `DigestValue` firmado.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::io::Write as _;
use std::process::{Command, Stdio};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cpeforge_xmldsig::c14n::{C14N, Method, canonicalize_xpath};
use cpeforge_xmldsig::dom::{Document, NodeId};
use cpeforge_xmldsig::dsig::{ReferenceOptions, SHA256, Transform, XmlSecurityDsig};
use cpeforge_xmldsig::key::{HMAC_SHA1, XmlSecurityKey};
use serde_json::{Value, json};

const DS: &str = "http://www.w3.org/2000/09/xmldsig#";
const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const EXT: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2";

/// Documentos con namespaces por defecto y con prefijo, redeclaraciones,
/// `xmlns=""`, atributos `xml:*`, comentarios, instrucciones de proceso,
/// CDATA, referencias de caracteres y una firma anidada.
const DOCUMENTS: &[&str] = &[
    r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><ds:Signature Id="S"><ds:SignedInfo><ds:Reference URI=""><ds:DigestValue>AAA=</ds:DigestValue></ds:Reference></ds:SignedInfo><ds:SignatureValue>BBB=</ds:SignatureValue></ds:Signature></ext:ExtensionContent></ext:UBLExtension></ext:UBLExtensions><cbc:ID>F001-1</cbc:ID><cbc:Note><![CDATA[a & b < c]]></cbc:Note><!-- comentario --><cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount></Invoice>"#,
    r#"<?xml version="1.0"?><?pi antes?><r xmlns:a="urn:a" xmlns:b="urn:b" xml:lang="es"><!--c1--><a:x Id="id1" a:k="1" b:k="2" z="&#x9;tab&#10;">texto&amp;<b:y xmlns:a="urn:a2"/><?p dentro?></a:x><s xmlns="urn:d"><t xmlns="">sin ns</t><u xml:space="preserve">  esp  </u></s><a:x Id="id2" xml:lang="en-US"><c/><c>2</c><c>3</c></a:x></r><!--fin-->"#,
    r#"<doc xmlns="urn:root" xmlns:p="urn:p"><p:e p:a="x" b="y"><p:e><p:e>profundo</p:e></p:e></p:e><e xmlns:q="urn:q" q:attr="v">mixto<q:f/>cola</e><g xmlns:p="urn:p">mismo uri</g><h xmlns:p="urn:p-otro">otro uri</h></doc>"#,
    r#"<a><b>1</b><b>2</b><b>-3.5</b><b>NaN</b><b>  10  </b><c n="4"/><c n="0.5"/><d>abc</d><d>ÁÉ ñ €</d></a>"#,
];

/// Expresiones del catálogo: ejes, pruebas de nodo, predicados, operadores y
/// las funciones de XPath 1.0. Las que usan `p:`/`ds:`/`cbc:`/`ext:` se
/// evalúan con esos prefijos registrados.
const EXPRESSIONS: &[&str] = &[
    "true()",
    "false()",
    "self::node()",
    "self::*",
    "self::text()",
    "self::comment()",
    "not(self::comment())",
    "self::processing-instruction()",
    "self::processing-instruction('p')",
    "not(ancestor-or-self::ds:Signature)",
    "not(ancestor-or-self::*[local-name()='Signature'])",
    "ancestor-or-self::ds:SignedInfo",
    "count(ancestor-or-self::ds:Signature) = 0",
    "not(self::ds:SignatureValue or ancestor::ds:SignatureValue)",
    "ancestor-or-self::cbc:ID or ancestor-or-self::cbc:PayableAmount",
    "not(ancestor-or-self::ext:UBLExtensions)",
    "parent::*",
    "../self::*",
    "count(ancestor::*) < 2",
    "count(ancestor::node()) mod 2 = 0",
    "child::*",
    "count(child::node()) > 1",
    "descendant::text()",
    "following-sibling::*",
    "preceding-sibling::*",
    "following::*",
    "preceding::comment()",
    "ancestor::*[1]",
    "ancestor::*[last()]",
    "self::*[@Id]",
    "@Id",
    "@*",
    "self::*[@Id='id1']//node()",
    "ancestor-or-self::*[@Id='id1']",
    "ancestor-or-self::*[@Id='id2']",
    "attribute::*[name()='z']",
    "self::*[@currencyID='PEN'] or ancestor::*[@currencyID]",
    "namespace::*",
    "count(namespace::*) > 2",
    "name()='xml'",
    "local-name()='a'",
    "namespace-uri()='urn:a'",
    "namespace-uri(..)='urn:d'",
    "string(.)='2'",
    "string-length(string(.)) > 3",
    "contains(., 'b')",
    "starts-with(name(), 'a:')",
    "substring(name(), 2, 1) = 'b'",
    "substring-before(name(), ':') = 'p'",
    "substring-after(name(), ':') = 'e'",
    "normalize-space(.) = 'esp'",
    "translate(string(.), 'abc', 'ABC') = 'ABC'",
    "concat(local-name(), '-', count(*)) = 'a-0'",
    "number(.) > 1",
    "number(.) = number(.)",
    "sum(c/@n) > 4",
    "floor(number(.)) = 2",
    "ceiling(number(.)) = -3",
    "round(number(.)) = 10",
    "string(number(.)) = 'NaN'",
    "boolean(@*)",
    "lang('es')",
    "lang('en')",
    "id('id1')",
    "count(id('id1 id2')) = 2",
    "position() = 1",
    "last() > 1",
    "(1 + 2) * 3 div 4 - -1 = 3.25",
    "self::b | self::c",
    "self::* and not(self::b)",
    "@* | text()",
    "text() = '2'",
    "* = 'abc'",
    "self::d != 'abc'",
    "2 > 1 and '2' >= 1 and not(1 < 'x')",
    "count(//node()) > 10",
    "/descendant::*[1] = .",
    "1",
    "0",
    "'texto'",
    "''",
    // Inválidas o no admitidas: ambos deben rechazarlas.
    "here()",
    "unknown-function()",
    "noregistrado:x",
    "self::*[",
    "1 +",
];

/// Prefijos registrados para el catálogo (los de un elemento `ds:XPath`).
fn namespaces() -> Vec<(String, String)> {
    [
        ("ds", DS),
        ("cbc", CBC),
        ("ext", EXT),
        ("p", "urn:p"),
        ("a", "urn:a"),
    ]
    .iter()
    .map(|(p, u)| ((*p).to_owned(), (*u).to_owned()))
    .collect()
}

fn methods() -> Vec<(&'static str, Method)> {
    let method = |exclusive, with_comments, inclusive: &[&str]| {
        Method::new(exclusive, with_comments)
            .with_inclusive_prefixes(inclusive.iter().map(|p| (*p).to_owned()).collect())
    };
    vec![
        ("c14n", method(false, false, &[])),
        ("c14n", method(false, true, &[])),
        ("exc-c14n", method(true, false, &[])),
        ("exc-c14n", method(true, true, &[])),
        ("exc-c14n", method(true, false, &["a", "#default"])),
    ]
}

/// xorshift64*: generador determinista sin dependencias.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        let n = u64::try_from(items.len()).unwrap();
        items[usize::try_from(self.next() % n).unwrap()]
    }
}

/// Expresión booleana aleatoria a partir de piezas válidas.
fn random_expression(rng: &mut Rng, depth: usize) -> String {
    const STEPS: &[&str] = &[
        "self::*",
        "self::text()",
        "self::comment()",
        "parent::*",
        "ancestor::*",
        "ancestor-or-self::*",
        "child::*",
        "child::text()",
        "descendant::*",
        "following-sibling::node()",
        "preceding-sibling::*",
        "@*",
        "@Id",
        "namespace::*",
        "self::processing-instruction()",
    ];
    const PREDICATES: &[&str] = &[
        "",
        "[1]",
        "[last()]",
        "[@*]",
        "[position() mod 2 = 1]",
        "[local-name()='x']",
        "[namespace-uri()='urn:a']",
        "[string-length(.) > 2]",
    ];
    const TESTS: &[&str] = &[
        "count({p}) > 0",
        "not({p})",
        "boolean({p})",
        "count({p}) = count(child::node())",
        "string({p}) = string(.)",
        "{p}",
    ];
    if depth > 0 && rng.next() % 3 == 0 {
        let op = rng.pick(&[" and ", " or "]);
        return format!(
            "({}{op}{})",
            random_expression(rng, depth - 1),
            random_expression(rng, depth - 1)
        );
    }
    let mut path = format!("{}{}", rng.pick(STEPS), rng.pick(PREDICATES));
    if rng.next() % 2 == 0 {
        path = format!("{path}/{}{}", rng.pick(STEPS), rng.pick(PREDICATES));
    }
    if rng.next() % 4 == 0 {
        path = format!("{path} | {}", rng.pick(STEPS));
    }
    rng.pick(TESTS).replace("{p}", &path)
}

struct Case {
    document: usize,
    context: Option<&'static str>,
    expr: String,
    mode: &'static str,
    method: Method,
}

/// Elemento con atributo `Id` igual a `id`, buscado en el DOM del crate.
fn by_id(doc: &Document, id: &str) -> NodeId {
    let root = doc.document_node();
    doc.descendants(root)
        .into_iter()
        .find(|n| doc.attribute(*n, "Id") == Some(id))
        .unwrap()
}

fn oracle(cases: &[Case]) -> Vec<Value> {
    let input: Vec<Value> = cases
        .iter()
        .map(|case| {
            json!({
                "xml": DOCUMENTS[case.document],
                "context": case.context.map(|id| format!("//*[@Id='{id}']")),
                "expr": case.expr,
                "namespaces": namespaces(),
                "mode": case.mode,
                "with_comments": case.method.with_comments,
                "inclusive_prefixes": case.method.inclusive_prefixes,
            })
        })
        .collect();
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/libxml2_xpath_c14n.py"
    );
    let mut child = Command::new("python3")
        .args(["-I", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python3 con ctypes y la libxml2 del sistema");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&input).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "oráculo libxml2: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Compara cada caso con libxml2; devuelve cuántos produjeron salida.
fn compare(cases: &[Case]) -> usize {
    let expected = oracle(cases);
    assert_eq!(expected.len(), cases.len());
    let documents: Vec<Document> = DOCUMENTS
        .iter()
        .map(|x| Document::parse(x).unwrap())
        .collect();
    let mut mismatches = Vec::new();
    let mut produced = 0;
    for (case, reference) in cases.iter().zip(&expected) {
        let doc = &documents[case.document];
        let node = case
            .context
            .map_or(doc.document_node(), |id| by_id(doc, id));
        let rust = canonicalize_xpath(doc, node, &case.method, &case.expr, &namespaces());
        let libxml2 = reference["c14n"]
            .as_str()
            .map(|b| String::from_utf8(STANDARD.decode(b).unwrap()).unwrap());
        match (&rust, &libxml2) {
            (Ok(a), Some(b)) if a == b => produced += 1,
            (Err(_), None) => {}
            _ => mismatches.push(format!(
                "doc {} ctx {:?} {} comments={} incl={:?} expr {:?}\n  Rust:    {:?}\n  libxml2: {:?}",
                case.document,
                case.context,
                case.mode,
                case.method.with_comments,
                case.method.inclusive_prefixes,
                case.expr,
                rust,
                libxml2.as_deref().ok_or(&reference["error"]),
            )),
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} diferencias con libxml2:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    produced
}

fn contexts(document: usize) -> Vec<Option<&'static str>> {
    match document {
        0 => vec![None, Some("S")],
        1 => vec![None, Some("id1"), Some("id2")],
        _ => vec![None],
    }
}

#[test]
fn catalog_matches_libxml2() {
    let mut cases = Vec::new();
    for document in 0..DOCUMENTS.len() {
        for context in contexts(document) {
            for expr in EXPRESSIONS {
                for (mode, method) in methods() {
                    cases.push(Case {
                        document,
                        context,
                        expr: (*expr).to_owned(),
                        mode,
                        method,
                    });
                }
            }
        }
    }
    let total = cases.len();
    let produced = compare(&cases);
    // La mayoría de los casos produce salida: no se compara solo el rechazo.
    assert!(produced * 10 > total * 8, "{produced} de {total}");
}

#[test]
fn random_expressions_match_libxml2() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut cases = Vec::new();
    for _ in 0..3_000 {
        let document = usize::try_from(rng.next() % 4).unwrap();
        let options = contexts(document);
        let context =
            options[usize::try_from(rng.next() % u64::try_from(options.len()).unwrap()).unwrap()];
        let (mode, method) = methods().swap_remove(usize::try_from(rng.next() % 5).unwrap());
        cases.push(Case {
            document,
            context,
            expr: random_expression(&mut rng, 2),
            mode,
            method,
        });
    }
    assert_eq!(compare(&cases), cases.len());
}

#[test]
fn xpath_signature_digest_verifies_with_libxml2_and_openssl() {
    // Documento sin la firma de ejemplo; la nueva se añade bajo la raíz.
    let unsigned = DOCUMENTS[0].replace(
        &DOCUMENTS[0][DOCUMENTS[0].find("<ds:Signature").unwrap()
            ..DOCUMENTS[0].find("</ds:Signature>").unwrap() + "</ds:Signature>".len()],
        "",
    );
    let mut doc = Document::parse(&unsigned).unwrap();
    let mut dsig = XmlSecurityDsig::default();
    dsig.set_canonical_method(&mut doc, C14N).unwrap();
    let expr = "not(ancestor-or-self::ds:Signature) and not(self::comment())";
    let root = doc.document_node();
    dsig.add_reference(
        &mut doc,
        root,
        SHA256,
        Some(&[Transform::XPath {
            query: expr.to_owned(),
            namespaces: vec![("ds".to_owned(), DS.to_owned())],
        }]),
        Some(&{
            let mut options = ReferenceOptions::default();
            options.force_uri = true;
            options
        }),
    )
    .unwrap();
    let mut key = XmlSecurityKey::new(HMAC_SHA1, None).unwrap();
    key.load_key(b"clave-ficticia", false).unwrap();
    let target = doc.document_element();
    dsig.sign(&mut doc, &key, target).unwrap();
    let signed = doc.to_xml();
    assert!(signed.contains("<ds:XPath"));
    let digest = signed
        .split("<ds:DigestValue>")
        .nth(1)
        .and_then(|rest| rest.split("</ds:DigestValue>").next())
        .unwrap();

    // libxml2 aplica el filtro al documento firmado: la firma queda fuera.
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/libxml2_xpath_c14n.py"
    );
    let input = json!([{
        "xml": signed,
        "context": null,
        "expr": expr,
        "namespaces": [["ds", DS]],
        "mode": "c14n",
        "with_comments": false,
    }]);
    let mut child = Command::new("python3")
        .args(["-I", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let output: Value = serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap();
    let canonical = STANDARD
        .decode(output[0]["c14n"].as_str().unwrap())
        .unwrap();
    assert!(!String::from_utf8_lossy(&canonical).contains("Signature"));
    let mut openssl = Command::new("openssl")
        .args(["dgst", "-sha256", "-binary"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    openssl.stdin.take().unwrap().write_all(&canonical).unwrap();
    let hash = openssl.wait_with_output().unwrap();
    assert!(hash.status.success());
    assert_eq!(STANDARD.encode(&hash.stdout), digest);
}
