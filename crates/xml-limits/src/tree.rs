//! Preescaneo lineal antes de un parser que construye el árbol en memoria
//! (`roxmltree`).
//!
//! `roxmltree` 0.21.1 compara cada atributo con los anteriores del mismo
//! elemento para detectar duplicados (tiempo cuadrático: 40 000 atributos
//! tardan casi un segundo y los 8 MiB de un CDR, minutos). Y cada elemento que
//! declara un espacio de nombres copia las referencias de todos los que
//! hereda: 250 `xmlns` en la raíz y miles de hijos que redeclaran un prefijo
//! multiplican la memoria por cientos. [`check_tree_markup`] recorre el texto
//! una vez y rechaza ambos casos antes de construir el árbol.

use crate::markup::MarkupRejection;

/// Coste máximo recomendado de los ámbitos de espacios de nombres (ver
/// [`check_tree_markup`]). Un comprobante UBL declara una docena de prefijos
/// en la raíz; aun redeclarándolos en cada elemento, una factura de miles de
/// líneas queda por debajo del millón.
pub const DEFAULT_MAX_NAMESPACE_SCOPE: usize = 4_000_000;

/// Recorre `xml` una vez y rechaza los elementos con más de
/// `max_attributes` atributos (contando las declaraciones `xmlns`) y los
/// documentos cuyo coste de ámbitos supera `max_namespace_scope`.
///
/// El coste suma, por cada elemento que declara espacios de nombres, los
/// espacios en ámbito tras la declaración (los heredados más los propios):
/// una cota por exceso de las referencias que copia un parser que guarda el
/// ámbito completo de cada elemento. Los elementos que no declaran nada no
/// suman.
///
/// Recibe el texto ya decodificado y no comprueba la codificación ni las
/// declaraciones `<!…>`: el parser rechaza un `DOCTYPE` (con
/// `allow_dtd: false`) y el recorrido se detiene en él, igual que en una
/// construcción sin cerrar, porque el parser no pasa de ahí.
///
/// ```
/// use cpeforge_xml_limits::{
///     DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE, MarkupRejection, check_tree_markup,
/// };
///
/// let limits = (DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE);
/// assert_eq!(check_tree_markup("<a xmlns:p='urn:p' p:x='1'/>", limits.0, limits.1), Ok(()));
/// let many: String = (0..300).map(|i| format!(" a{i}=''")).collect();
/// assert_eq!(
///     check_tree_markup(&format!("<a{many}/>"), limits.0, limits.1),
///     Err(MarkupRejection::TooManyAttributes)
/// );
/// ```
///
/// # Errors
///
/// [`MarkupRejection::TooManyAttributes`] o
/// [`MarkupRejection::TooManyNamespaces`] con el primer límite superado.
pub fn check_tree_markup(
    xml: &str,
    max_attributes: usize,
    max_namespace_scope: usize,
) -> Result<(), MarkupRejection> {
    let bytes = xml.as_bytes();
    // Espacios en ámbito (cota) de cada elemento abierto.
    let mut open: Vec<usize> = Vec::new();
    let mut cost = 0usize;
    let mut pos = 0usize;
    while let Some(offset) = bytes.get(pos..).and_then(|rest| memchr(b'<', rest)) {
        let start = pos + offset;
        let rest = &bytes[start..];
        let end = if rest.starts_with(b"<!--") {
            skip_past(bytes, start + 4, b"-->")
        } else if rest.starts_with(b"<![CDATA[") {
            skip_past(bytes, start + 9, b"]]>")
        } else if rest.starts_with(b"<?") {
            skip_past(bytes, start + 2, b"?>")
        } else if rest.starts_with(b"<!") {
            None
        } else if rest.starts_with(b"</") {
            open.pop();
            skip_past(bytes, start + 2, b">")
        } else {
            match start_tag(bytes, start + 1) {
                Some(tag) => {
                    if tag.attributes > max_attributes {
                        return Err(MarkupRejection::TooManyAttributes);
                    }
                    let inherited = open.last().copied().unwrap_or(0);
                    let in_scope = inherited.saturating_add(tag.namespaces);
                    if tag.namespaces > 0 {
                        cost = cost.saturating_add(in_scope);
                        if cost > max_namespace_scope {
                            return Err(MarkupRejection::TooManyNamespaces);
                        }
                    }
                    if !tag.self_closing {
                        open.push(in_scope);
                    }
                    Some(tag.end)
                }
                None => None,
            }
        };
        match end {
            Some(end) => pos = end,
            None => break,
        }
    }
    Ok(())
}

/// Etiqueta de apertura recorrida.
struct StartTag {
    /// Posición tras el `>`.
    end: usize,
    /// Termina en `/>`.
    self_closing: bool,
    /// `=` fuera de comillas.
    attributes: usize,
    /// Atributos `xmlns` o `xmlns:…`.
    namespaces: usize,
}

/// Recorre la etiqueta desde el nombre del elemento hasta el `>` que la
/// cierra, fuera de comillas. El nombre de cada atributo es la última palabra
/// antes de su `=` (admite espacios alrededor del `=`).
fn start_tag(bytes: &[u8], from: usize) -> Option<StartTag> {
    let mut quote: Option<u8> = None;
    let mut previous = 0u8;
    let mut word_start: Option<usize> = None;
    let mut word = (0usize, 0usize);
    let mut attributes = 0usize;
    let mut namespaces = 0usize;
    for (index, &byte) in bytes.get(from..)?.iter().enumerate() {
        let at = from + index;
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None => match byte {
                b'>' => {
                    return Some(StartTag {
                        end: at + 1,
                        self_closing: previous == b'/',
                        attributes,
                        namespaces,
                    });
                }
                b'"' | b'\'' | b'=' | b'/' | b' ' | b'\t' | b'\r' | b'\n' => {
                    if let Some(start) = word_start.take() {
                        word = (start, at);
                    }
                    match byte {
                        b'"' | b'\'' => quote = Some(byte),
                        b'=' => {
                            attributes += 1;
                            let name = bytes.get(word.0..word.1).unwrap_or_default();
                            if name == b"xmlns" || name.starts_with(b"xmlns:") {
                                namespaces += 1;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {
                    if word_start.is_none() {
                        word_start = Some(at);
                    }
                }
            },
        }
        previous = byte;
    }
    None
}

fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

/// Posición tras la primera aparición de `marker` desde `from`.
fn skip_past(bytes: &[u8], from: usize, marker: &[u8]) -> Option<usize> {
    bytes
        .get(from..)?
        .windows(marker.len())
        .position(|w| w == marker)
        .map(|p| from + p + marker.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(xml: &str, attributes: usize, scope: usize) -> Result<(), MarkupRejection> {
        check_tree_markup(xml, attributes, scope)
    }

    #[test]
    fn counts_attributes_outside_quotes() {
        assert_eq!(check("<a x='1=2' y = \"3\"/>", 2, 0), Ok(()));
        assert_eq!(
            check("<a x='1' y='2' z='3'/>", 2, 0),
            Err(MarkupRejection::TooManyAttributes)
        );
        // Comentarios, CDATA e instrucciones no cuentan.
        assert_eq!(
            check(
                "<r><!-- <a x='' y=''/> --><![CDATA[<a x='' y=''/>]]><?p x='' y=''?></r>",
                0,
                0
            ),
            Ok(())
        );
    }

    #[test]
    fn namespace_scope_cost_is_cumulative() {
        // Raíz con 2 prefijos (coste 2) y dos hijos que declaran uno más
        // (coste 3 cada uno): total 8.
        let xml = "<r xmlns:a='urn:a' xmlns='urn:d'><x xmlns:z='u'/><y xmlns:z='u'></y></r>";
        assert_eq!(check(xml, 8, 8), Ok(()));
        assert_eq!(check(xml, 8, 7), Err(MarkupRejection::TooManyNamespaces));
        // El ámbito de un hermano cerrado no se hereda.
        let siblings = "<r><a xmlns:p='1'><b xmlns:q='2'/></a><c xmlns:s='3'/></r>";
        assert_eq!(check(siblings, 8, 4), Ok(()));
        assert_eq!(
            check(siblings, 8, 3),
            Err(MarkupRejection::TooManyNamespaces)
        );
        // `xmlnsx` no es una declaración; los espacios alrededor del `=` sí.
        assert_eq!(check("<a xmlnsx='1' xmlns:p = '2'/>", 8, 1), Ok(()));
        assert_eq!(
            check("<a xmlns:p = '2'/>", 8, 0),
            Err(MarkupRejection::TooManyNamespaces)
        );
    }

    #[test]
    fn stops_where_the_parser_stops() {
        let attack: String = "<x xmlns:z='u'/>".repeat(100);
        assert_eq!(check(&format!("<!DOCTYPE r><r>{attack}</r>"), 8, 1), Ok(()));
        assert_eq!(check(&format!("<r><!-- {attack}"), 8, 1), Ok(()));
        assert_eq!(check("<r x='>", 0, 0), Ok(()));
    }
}
