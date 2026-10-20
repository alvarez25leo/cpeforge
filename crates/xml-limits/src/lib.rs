//! Límites XML comprobados sobre el texto, antes de construir el árbol:
//! anidamiento ([`exceeds_depth`]) y marcado peligroso para libxml2
//! ([`check_markup`]).
//!
//! `roxmltree` (0.21.1, la última publicada) analiza los elementos de forma
//! recursiva y no limita la profundidad: con un hilo de 2 MiB (los workers de
//! Tokio) desborda la pila hacia los 120 niveles en debug y los 3 100 en
//! release, y el desbordamiento aborta el proceso sin que pueda capturarse.
//! [`exceeds_depth`] recorre el texto una sola vez, sin recursión ni memoria
//! adicional, y permite rechazar el documento antes de que el parser recurra.
//!
//! Lo comparten los crates que analizan XML externo con `roxmltree`
//! (`cpeforge-ws`, `cpeforge-xml-parser`); cada uno elige su límite y su
//! error. No tiene equivalente en la referencia: libxml2 limita la profundidad
//! por su cuenta (256 niveles, 2048 con `LIBXML_PARSEHUGE`).
//!
//! ```
//! use cpeforge_xml_limits::exceeds_depth;
//!
//! let deep = format!("{}{}", "<a>".repeat(5_000), "</a>".repeat(5_000));
//! assert!(exceeds_depth(&deep, 256));
//! assert!(!exceeds_depth("<r><a/><a/></r>", 2));
//! ```

mod markup;

pub use markup::{DEFAULT_MAX_ATTRIBUTES, MarkupRejection, check_markup};

static LIBXML_INIT: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Ejecuta `init` con un candado único en todo el proceso.
///
/// Antes de libxml2 2.11, `xmlInitParser` no es seguro si dos hilos lo llaman
/// a la vez. Cada crate que enlaza libxml2 (`cpeforge-cpe-validator`,
/// `cpeforge-ubl-validator`) inicializa la biblioteca dentro de este candado,
/// así sus inicializaciones nunca se solapan aunque cada uno tenga su propio
/// `OnceLock`.
pub fn with_libxml_init_lock<T>(init: impl FnOnce() -> T) -> T {
    let _guard = LIBXML_INIT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init()
}

/// Indica si algún elemento de `xml` está anidado a más de `max` niveles
/// (el elemento raíz es el nivel 1).
///
/// El recorrido sigue la sintaxis XML que el parser acepta: omite
/// comentarios, secciones CDATA, instrucciones de procesamiento y
/// declaraciones `<!…>`, y respeta las comillas de los atributos, de modo que
/// un `>` o un `/>` dentro de un valor no altera la cuenta. Una construcción
/// sin cerrar termina el recorrido: el parser rechazará después ese documento.
/// Las etiquetas de cierre sobrantes no bajan de cero.
///
/// Para todo documento que `roxmltree` acepta, la profundidad contada es la
/// real; en uno que rechaza, la cuenta coincide hasta el primer error, que es
/// donde el parser deja de recurrir. Solo es una cota válida si el parser
/// **no expande entidades declaradas** (`roxmltree` con `allow_dtd: false`):
/// el texto de reemplazo de una entidad no aparece en el recorrido.
#[must_use]
pub fn exceeds_depth(xml: &str, max: usize) -> bool {
    let bytes = xml.as_bytes();
    let mut depth = 0usize;
    let mut pos = 0usize;
    while let Some(offset) = xml.get(pos..).and_then(|rest| rest.find('<')) {
        let start = pos + offset;
        let rest = xml.get(start..).unwrap_or_default();
        let end = if rest.starts_with("<!--") {
            skip_past(xml, start + 4, "-->")
        } else if rest.starts_with("<![CDATA[") {
            skip_past(xml, start + 9, "]]>")
        } else if rest.starts_with("<?") {
            skip_past(xml, start + 2, "?>")
        } else if rest.starts_with("<!") {
            declaration_end(bytes, start + 2)
        } else if rest.starts_with("</") {
            depth = depth.saturating_sub(1);
            skip_past(xml, start + 2, ">")
        } else {
            match start_tag_end(bytes, start + 1) {
                Some((end, self_closing)) => {
                    // Un elemento vacío (`<c/>`) también ocupa un nivel.
                    let level = depth + 1;
                    if level > max {
                        return true;
                    }
                    if !self_closing {
                        depth = level;
                    }
                    Some(end)
                }
                None => None,
            }
        };
        match end {
            Some(end) => pos = end,
            None => return false,
        }
    }
    false
}

/// Posición siguiente a la primera aparición de `needle` desde `from`.
fn skip_past(xml: &str, from: usize, needle: &str) -> Option<usize> {
    xml.get(from..)?
        .find(needle)
        .map(|offset| from + offset + needle.len())
}

/// Fin de una etiqueta de apertura: posición tras `>` y si termina en `/>`.
fn start_tag_end(bytes: &[u8], from: usize) -> Option<(usize, bool)> {
    let mut quote: Option<u8> = None;
    let mut previous = 0u8;
    for (index, &byte) in bytes.get(from..)?.iter().enumerate() {
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b'>' => return Some((from + index + 1, previous == b'/')),
            None => {}
        }
        previous = byte;
    }
    None
}

/// Fin de una declaración `<!…>` (por ejemplo `DOCTYPE`), con su subconjunto
/// interno entre corchetes y valores entre comillas.
fn declaration_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    let mut brackets = 0usize;
    for (index, &byte) in bytes.get(from..)?.iter().enumerate() {
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None => match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'[' => brackets += 1,
                b']' => brackets = brackets.saturating_sub(1),
                b'>' if brackets == 0 => return Some(from + index + 1),
                _ => {}
            },
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::exceeds_depth;

    fn nested(depth: usize) -> String {
        format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth))
    }

    #[test]
    fn counts_real_nesting() {
        assert!(!exceeds_depth(&nested(3), 3));
        assert!(exceeds_depth(&nested(4), 3));
        assert!(!exceeds_depth("<a><b/><b/><b/><b/></a>", 2));
        assert!(!exceeds_depth("<a/><a/>", 1));
        assert!(!exceeds_depth("<a><b><c/></b></a>", 3));
        assert!(exceeds_depth("<a><b><c/></b></a>", 2));
        assert!(exceeds_depth("<a><b><c></c></b></a>", 2));
        assert!(!exceeds_depth("", 0));
    }

    #[test]
    fn ignores_markup_inside_other_constructs() {
        let hidden = "<!-- <a><a><a> --><![CDATA[<a><a>]]><?pi <a><a> ?><r/>";
        assert!(!exceeds_depth(hidden, 1));
        assert!(!exceeds_depth("<!DOCTYPE r [<!ENTITY e '<a><a>'>]><r/>", 1));
        assert!(!exceeds_depth(
            "<!DOCTYPE a [<!ELEMENT a ANY>]><a><b/></a>",
            2
        ));
        assert!(!exceeds_depth("<r a='>' b=\"/>\"><s/></r>", 2));
        let mixed = "<a><!-- <b><c> --><b x='>' y=\"<\"/><![CDATA[<d><e>]]><?p <f>?></a>";
        assert!(!exceeds_depth(mixed, 2));
        assert!(exceeds_depth(mixed, 1));
    }

    #[test]
    fn quoted_self_close_does_not_hide_depth() {
        // Si `/>` dentro de un atributo contara como cierre, este documento
        // parecería plano.
        let tricky = "<a x='/>'>".repeat(10);
        assert!(exceeds_depth(&tricky, 9));
    }

    #[test]
    fn surplus_closing_tags_do_not_go_negative() {
        let doc = format!("</x></x></x>{}", nested(3));
        assert!(exceeds_depth(&doc, 2));
    }

    #[test]
    fn unterminated_constructs_stop_the_scan() {
        // El parser falla en la construcción abierta sin recurrir más allá.
        let open_tag = format!("{}<a", "<a>".repeat(3));
        assert!(!exceeds_depth(&open_tag, 3));
        assert!(!exceeds_depth("<a><!-- <b><c><d>", 1));
        assert!(!exceeds_depth("<a x='<b><c><d>", 0));
    }

    #[test]
    fn long_inputs_without_recursion() {
        assert!(exceeds_depth(&"<a>".repeat(1_000_000), 256));
        assert!(!exceeds_depth(&"<a/>".repeat(1_000_000), 1));
    }
}
