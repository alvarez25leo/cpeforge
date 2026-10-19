//! Casos extremos del árbol y de las consultas: ámbitos de espacios de
//! nombres que cambian entre hermanos, raíz con muchos hijos y consultas de
//! descendientes sobre la profundidad máxima.

use std::time::{Duration, Instant};

use cpeforge_xml_parser::{Limits, XmlReader};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn namespace_scopes_follow_each_element() -> TestResult {
    let xml = "<r xmlns:a='urn:a'><x xmlns:b='urn:b'><a:y><b:z>1</b:z></a:y></x>\
               <x><a:y><z>2</z></a:y></x>\
               <x xmlns:b='urn:otro'><a:y><b:z>3</b:z></a:y></x>\
               <x xmlns:a='urn:a2'><a:y>4</a:y></x></r>";
    let reader = XmlReader::load_xml(xml.as_bytes())?;
    let xs = reader.nodes("x", None)?.unwrap_or_default();
    assert_eq!(xs.len(), 4);
    let first = |i: usize, query: &str| -> Result<Option<String>, Box<dyn std::error::Error>> {
        Ok(reader.value_or(query, xs.get(i).copied(), None)?)
    };
    // `b` solo está en ámbito donde se declara, y con su propio URI; fuera
    // de ámbito la consulta no tiene resultado y se usa el valor por defecto.
    assert_eq!(first(0, "a:y/b:z")?, Some("1".into()));
    assert_eq!(first(1, "a:y/b:z")?, None);
    assert_eq!(first(2, "a:y/b:z")?, Some("3".into()));
    // `a` redeclarado: `a:y` del cuarto `x` es de `urn:a2`.
    assert_eq!(first(3, "a:y")?, Some("4".into()));
    assert_eq!(reader.nodes("//a:y", None)?.map(|n| n.len()), Some(3));
    Ok(())
}

#[test]
fn wide_root_is_queried_in_document_order() -> TestResult {
    let mut xml = String::from("<r xmlns:c='urn:c'>");
    for i in 0..50_000 {
        xml.push_str(&format!("<c:x n='{i}'><c:v>{i}</c:v></c:x>"));
    }
    xml.push_str("<c:ID>fin</c:ID></r>");
    let reader = XmlReader::load_xml(xml.as_bytes())?;
    assert_eq!(reader.value("/*/c:ID", None)?, "fin");
    let all = reader.nodes("//c:v", None)?.unwrap_or_default();
    assert_eq!(all.len(), 50_000);
    assert_eq!(all.first().map(|n| n.value()), Some("0".into()));
    assert_eq!(all.last().map(|n| n.value()), Some("49999".into()));
    assert_eq!(reader.value("c:x/@n", None)?, "0");
    assert_eq!(reader.nodes("//@n", None)?.map(|n| n.len()), Some(50_000));
    Ok(())
}

#[test]
fn nested_descendant_queries_stay_bounded() -> TestResult {
    // Cadena de 64 niveles (el máximo por defecto) con un hermano por nivel:
    // `//*//*` parte de cada elemento y recorre sus descendientes, un coste
    // acotado por la profundidad.
    let depth = Limits::default().max_depth;
    let mut xml = String::new();
    for _ in 0..depth - 1 {
        xml.push_str("<a><b/>");
    }
    xml.push_str("<a/>");
    for _ in 0..depth - 1 {
        xml.push_str("</a>");
    }
    let reader = XmlReader::load_xml(xml.as_bytes())?;
    let started = Instant::now();
    let nodes = reader.nodes("//*//*", None)?.unwrap_or_default();
    assert_eq!(nodes.len(), 2 * (depth - 1));
    let pairs = reader.nodes("//a//b", None)?.unwrap_or_default();
    assert_eq!(pairs.len(), depth - 1);
    assert!(started.elapsed() < Duration::from_secs(2));
    Ok(())
}
