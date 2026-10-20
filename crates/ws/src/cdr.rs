//! Lectura de CDR y resolución de nombres y tipos de documentos UBL.

use cpeforge_core::model::response::CdrResponse;
use cpeforge_xml_limits::{
    DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE, check_tree_markup, exceeds_depth,
};
use roxmltree::{Document, Node, ParsingOptions};

use crate::WsError;

const CAC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2";
const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
const APPLICATION_RESPONSE: &str =
    "urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2";
/// Tamaño máximo del XML recibido.
pub const MAX_XML_BYTES: usize = 8 * 1024 * 1024;
/// Profundidad máxima de anidamiento de elementos (la raíz es el nivel 1).
///
/// Un CDR de SUNAT no supera la docena de niveles. `roxmltree` analiza de
/// forma recursiva y en compilación debug usa unos 17 KB de pila por nivel:
/// 64 niveles caben con holgura en los 2 MiB de un hilo de Tokio también en
/// debug (con 256, unos 120 niveles ya abortaban el proceso). Es el mismo
/// límite que `cpeforge-xml-parser` usa por defecto.
pub const MAX_XML_DEPTH: usize = 64;
/// Máximo de nodos del árbol XML.
const MAX_XML_NODES: u32 = 100_000;

/// Analiza `xml` sin DTD y dentro de los límites de tamaño, profundidad,
/// nodos, atributos por elemento y ámbitos de espacios de nombres.
fn parse(xml: &str) -> Result<Document<'_>, WsError> {
    if xml.len() > MAX_XML_BYTES {
        return Err(WsError::Xml("documento demasiado grande"));
    }
    if exceeds_depth(xml, MAX_XML_DEPTH) {
        return Err(WsError::Xml("anidamiento demasiado profundo"));
    }
    // `roxmltree` busca atributos duplicados en tiempo cuadrático y copia el
    // ámbito heredado en cada declaración de espacio de nombres.
    if check_tree_markup(xml, DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE).is_err() {
        return Err(WsError::Xml("demasiados atributos o espacios de nombres"));
    }
    Document::parse_with_options(
        xml,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_NODES,
            ..ParsingOptions::default()
        },
    )
    .map_err(|_| WsError::Xml("documento malformado o DTD prohibido"))
}

fn is(node: Node<'_, '_>, ns: &str, name: &str) -> bool {
    node.is_element() && node.tag_name().namespace() == Some(ns) && node.tag_name().name() == name
}

fn child<'a, 'input>(node: Node<'a, 'input>, ns: &str, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|n| is(*n, ns, name))
}

/// Texto de todos los descendientes, en orden de documento.
fn value(node: Option<Node<'_, '_>>) -> Option<String> {
    node.map(|n| {
        n.descendants()
            .filter(Node::is_text)
            .filter_map(|part| part.text())
            .collect()
    })
}

/// Campos del CDR leídos de un árbol ya validado.
fn extract(root: Node<'_, '_>) -> CdrResponse {
    let response = child(root, CAC, "DocumentResponse");
    let node = response.and_then(|n| child(n, CAC, "Response"));
    let reference = response
        .and_then(|n| child(n, CAC, "DocumentReference"))
        .and_then(|n| child(n, CBC, "DocumentDescription"));
    let notes = root
        .children()
        .filter(|n| is(*n, CBC, "Note"))
        .filter_map(|n| value(Some(n)))
        .collect();
    CdrResponse {
        id: value(node.and_then(|n| child(n, CBC, "ReferenceID"))),
        code: value(node.and_then(|n| child(n, CBC, "ResponseCode"))),
        description: value(node.and_then(|n| child(n, CBC, "Description"))),
        notes: Some(notes),
        reference: value(reference),
    }
}

/// Extrae de un `ApplicationResponse` el identificador, código, descripción,
/// notas y referencia del CDR.
///
/// Lector tolerante: no comprueba el elemento raíz y un nodo ausente produce
/// `None`, de modo que un XML bien formado que no es un CDR devuelve campos
/// vacíos. Para exigir la estructura de un CDR, usar [`read_cdr_checked`].
///
/// # Errors
///
/// Rechaza XML vacío, malformado, con DTD o fuera de los límites de tamaño,
/// profundidad o nodos.
pub fn read_cdr(xml: &str) -> Result<CdrResponse, WsError> {
    if xml.is_empty() {
        return Err(WsError::XmlReader(
            "XML del CDR no puede estar vacío.".to_owned(),
        ));
    }
    let doc = parse(xml)?;
    Ok(extract(doc.root_element()))
}

/// Lee el primer XML del ZIP y luego el CDR con [`read_cdr`].
///
/// # Errors
///
/// ZIP/XML inválido o sin entrada XML.
pub fn read_cdr_zip(bytes: &[u8]) -> Result<CdrResponse, WsError> {
    read_cdr(&zip_xml(bytes)?)
}

fn zip_xml(bytes: &[u8]) -> Result<String, WsError> {
    let xml = crate::zip::first_xml(bytes)?.ok_or(WsError::Xml("CDR sin XML"))?;
    String::from_utf8(xml).map_err(|_| WsError::Xml("CDR no es UTF-8"))
}

/// Como [`read_cdr`], pero exige la estructura mínima de un CDR.
///
/// Distingue los casos que el lector tolerante confunde:
///
/// | Entrada | Resultado |
/// | --- | --- |
/// | XML malformado, con DTD o fuera de límites | [`WsError::Xml`] |
/// | XML válido cuya raíz no es `ApplicationResponse` (UBL) | [`WsError::InvalidResponse`] |
/// | CDR sin `DocumentResponse/Response`, `ResponseCode` o `ReferenceID` | [`WsError::InvalidResponse`] |
/// | CDR completo, aceptado o rechazado | `Ok`; clasificar con [`crate::outcome::TaxStatus::from_cdr`] |
///
/// Un `ResponseCode` o un `ReferenceID` formados solo por espacios cuentan
/// como ausentes. Los campos devueltos son idénticos a los de [`read_cdr`].
///
/// # Errors
///
/// Los indicados en la tabla, además de los de [`read_cdr`].
pub fn read_cdr_checked(xml: &str) -> Result<CdrResponse, WsError> {
    if xml.is_empty() {
        return Err(WsError::XmlReader(
            "XML del CDR no puede estar vacío.".to_owned(),
        ));
    }
    let doc = parse(xml)?;
    let root = doc.root_element();
    if !is(root, APPLICATION_RESPONSE, "ApplicationResponse") {
        return Err(WsError::InvalidResponse(
            "CDR: la raíz no es ApplicationResponse",
        ));
    }
    let response = child(root, CAC, "DocumentResponse")
        .ok_or(WsError::InvalidResponse("CDR sin DocumentResponse"))?;
    child(response, CAC, "Response").ok_or(WsError::InvalidResponse("CDR sin Response"))?;
    let cdr = extract(root);
    let present = |field: &Option<String>| field.as_deref().is_some_and(|v| !v.trim().is_empty());
    if !present(&cdr.code) {
        return Err(WsError::InvalidResponse("CDR sin ResponseCode"));
    }
    if !present(&cdr.id) {
        return Err(WsError::InvalidResponse("CDR sin ReferenceID"));
    }
    Ok(cdr)
}

/// Lee el primer XML del ZIP y luego el CDR con [`read_cdr_checked`].
///
/// # Errors
///
/// ZIP/XML inválido, sin entrada XML o CDR incompleto.
pub fn read_cdr_zip_checked(bytes: &[u8]) -> Result<CdrResponse, WsError> {
    read_cdr_checked(&zip_xml(bytes)?)
}

/// Comprueba que el CDR responde al comprobante esperado.
///
/// `expected` es el `ReferenceID` que SUNAT devuelve: la serie y número del
/// comprobante (`F001-123`) o el identificador del resumen o la baja
/// (`RC-20261017-1`). Se comparan sin espacios en los extremos.
///
/// # Errors
///
/// [`WsError::InvalidResponse`] si el CDR no trae `ReferenceID` o es de otro
/// documento.
pub fn ensure_cdr_reference(cdr: &CdrResponse, expected: &str) -> Result<(), WsError> {
    match cdr.id.as_deref().map(str::trim) {
        Some(id) if id == expected.trim() => Ok(()),
        Some(_) => Err(WsError::InvalidResponse("CDR de otro comprobante")),
        None => Err(WsError::InvalidResponse("CDR sin ReferenceID")),
    }
}

/// Identificador del modelo de documento según el elemento raíz.
///
/// Devuelve nombres de tipo estables (`CPEForge\Model\Sale\Invoice`, …) que
/// otros crates usan para elegir el lector; una raíz desconocida da `""`.
/// `VoidedDocuments` es una baja si su `cbc:ID` empieza por `RA` y una
/// reversión en otro caso.
///
/// # Errors
///
/// XML inválido, DTD o fuera de límites.
pub fn resolve_type(xml: &str) -> Result<&'static str, WsError> {
    Ok(type_of(parse(xml)?.root_element()))
}

/// [`resolve_type`] y [`extract_filename`] con un único análisis del XML.
///
/// El resultado externo falla como ambas funciones (XML inválido, DTD o fuera
/// de límites); el nombre conserva su propio resultado, para que el llamador
/// pueda comprobar primero el tipo y después el nombre, como al llamarlas por
/// separado.
///
/// # Errors
///
/// XML inválido, DTD o fuera de límites.
pub fn resolve_type_and_filename(
    xml: &str,
) -> Result<(&'static str, Result<String, WsError>), WsError> {
    let doc = parse(xml)?;
    let root = doc.root_element();
    Ok((type_of(root), filename_of(root)))
}

fn type_of(root: Node<'_, '_>) -> &'static str {
    match root.tag_name().name() {
        "Invoice" => "CPEForge\\Model\\Sale\\Invoice",
        "CreditNote" | "DebitNote" => "CPEForge\\Model\\Sale\\Note",
        "DespatchAdvice" => "CPEForge\\Model\\Despatch\\Despatch",
        "Perception" => "CPEForge\\Model\\Perception\\Perception",
        "Retention" => "CPEForge\\Model\\Retention\\Retention",
        "SummaryDocuments" => "CPEForge\\Model\\Summary\\Summary",
        "VoidedDocuments" => {
            if value(child(root, CBC, "ID"))
                .unwrap_or_default()
                .starts_with("RA")
            {
                "CPEForge\\Model\\Voided\\Voided"
            } else {
                "CPEForge\\Model\\Voided\\Reversion"
            }
        }
        _ => "",
    }
}

/// Nombre de archivo SUNAT `RUC-tipo-ID` (o `RUC-ID` para resúmenes y bajas).
///
/// El RUC se toma del emisor según el tipo de documento y, en facturas y
/// notas, según la versión UBL (2.0 o 2.1).
///
/// # Errors
///
/// XML inválido, DTD, fuera de límites o versión UBL distinta de 2.0/2.1.
pub fn extract_filename(xml: &str) -> Result<String, WsError> {
    filename_of(parse(xml)?.root_element())
}

fn filename_of(root: Node<'_, '_>) -> Result<String, WsError> {
    let name = root.tag_name().name();
    let ruc = match name {
        "Perception" | "Retention" => child(root, CAC, "AgentParty")
            .and_then(|n| child(n, CAC, "PartyIdentification"))
            .and_then(|n| child(n, CBC, "ID")),
        "DespatchAdvice" => child(root, CAC, "DespatchSupplierParty")
            .and_then(|n| child(n, CBC, "CustomerAssignedAccountID")),
        _ => match value(child(root, CBC, "UBLVersionID")).as_deref() {
            Some("2.0") => child(root, CAC, "AccountingSupplierParty")
                .and_then(|n| child(n, CBC, "CustomerAssignedAccountID")),
            Some("2.1") => child(root, CAC, "AccountingSupplierParty")
                .and_then(|n| child(n, CAC, "Party"))
                .and_then(|n| child(n, CAC, "PartyIdentification"))
                .and_then(|n| child(n, CBC, "ID")),
            other => {
                return Err(WsError::XmlReader(format!(
                    "UBL version {} no soportada.",
                    other.unwrap_or_default()
                )));
            }
        },
    };
    let ruc = value(ruc).unwrap_or_default();
    let id = value(child(root, CBC, "ID")).unwrap_or_default();
    let kind = match name {
        "Invoice" => value(child(root, CBC, "InvoiceTypeCode")).unwrap_or_default(),
        "CreditNote" => "07".to_owned(),
        "DebitNote" => "08".to_owned(),
        "DespatchAdvice" => "09".to_owned(),
        "Perception" => "40".to_owned(),
        "Retention" => "20".to_owned(),
        _ => String::new(),
    };
    if kind.is_empty() {
        Ok(format!("{ruc}-{id}"))
    } else {
        Ok(format!("{ruc}-{kind}-{id}"))
    }
}
