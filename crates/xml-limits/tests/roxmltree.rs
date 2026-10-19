//! Comparación diferencial con `roxmltree` (`allow_dtd: false`): para cada
//! documento que el parser acepta, `exceeds_depth` debe contar exactamente la
//! profundidad real de elementos. Si contara menos, un documento hostil podría
//! pasar el pre-escaneo y desbordar la pila.

use cpeforge_xml_limits::exceeds_depth;

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

/// Fragmentos que no son elementos pero contienen `<`, `>`, `/>` o comillas.
const NOISE: &[&str] = &[
    "texto",
    "a &gt; b",
    "<!-- <x><y> -->",
    "<!---->",
    "<![CDATA[<x><y>]]>",
    "<![CDATA[]]]]>",
    "<?pi <x> ?>",
    "<?pi?>",
    "&amp;",
    "'\"",
];

/// Atributos con valores que imitan marcado.
const ATTRS: &[&str] = &[
    "",
    " x='>'",
    " x=\"/>\"",
    " x='\"' y=\"'\"",
    " x = '1' ",
    " p:x='a' xmlns:p='urn:p'",
];

fn element(rng: &mut Rng, out: &mut String, depth: usize, budget: &mut usize) {
    let attr = ATTRS[rng.below(ATTRS.len())];
    *budget = budget.saturating_sub(1);
    if *budget == 0 || rng.below(6) == 0 {
        out.push_str(&format!("<e{attr}/>"));
        return;
    }
    out.push_str(&format!("<e{depth}{attr}>"));
    for _ in 0..rng.below(4) {
        if rng.below(3) == 0 {
            out.push_str(NOISE[rng.below(NOISE.len())]);
        } else {
            element(rng, out, depth + 1, budget);
        }
    }
    out.push_str(&format!("</e{depth}>"));
}

/// Profundidad real según `roxmltree`, o `None` si rechaza el documento.
fn real_depth(xml: &str) -> Option<usize> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    Some(
        doc.descendants()
            .filter(roxmltree::Node::is_element)
            .map(|node| node.ancestors().filter(roxmltree::Node::is_element).count())
            .max()
            .unwrap_or(0),
    )
}

fn assert_exact(xml: &str) -> bool {
    let Some(depth) = real_depth(xml) else {
        return false;
    };
    assert!(!exceeds_depth(xml, depth), "cuenta de más ({depth}): {xml}");
    assert!(
        depth == 0 || exceeds_depth(xml, depth - 1),
        "cuenta de menos ({depth}): {xml}"
    );
    true
}

#[test]
fn matches_roxmltree_on_generated_documents() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut accepted = 0;
    for _ in 0..3_000 {
        let mut xml = String::new();
        if rng.below(2) == 0 {
            xml.push_str("<?xml version='1.0'?><!-- <z> -->");
        }
        let mut budget = 40;
        element(&mut rng, &mut xml, 1, &mut budget);
        if assert_exact(&xml) {
            accepted += 1;
        }
    }
    assert_eq!(
        accepted, 3_000,
        "el generador solo produce XML bien formado"
    );
}

#[test]
fn matches_roxmltree_on_mutations() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let mut accepted = 0;
    for _ in 0..3_000 {
        let mut xml = String::new();
        let mut budget = 30;
        element(&mut rng, &mut xml, 1, &mut budget);
        let mut bytes = xml.into_bytes();
        for _ in 0..=rng.below(3) {
            let at = rng.below(bytes.len() + 1);
            let piece: &[u8] = match rng.below(8) {
                0 => b"<",
                1 => b">",
                2 => b"/",
                3 => b"'",
                4 => b"\"",
                5 => b"<e>",
                6 => b"</e>",
                _ => b"<!--",
            };
            bytes.splice(at..at, piece.iter().copied());
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if assert_exact(&text) {
            accepted += 1;
        }
    }
    // Una parte de las mutaciones sigue siendo XML válido.
    assert!(accepted > 100, "solo {accepted} mutaciones válidas");
}
