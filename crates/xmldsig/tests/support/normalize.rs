//! Normalización de salidas del oráculo de referencia y del ejecutor Rust.
//!
//! - [`normalize_random`]: valores aleatorios por diseño (GUID de `Id`,
//!   `CipherValue`, clave de sesión). Se aplica a la comparación referencia↔Rust con
//!   las mismas claves; todo lo demás, incluidas las firmas RSA (PKCS#1 v1.5
//!   es determinista), se compara byte a byte.
//! - [`redact_keys`]: además retira todo lo que depende de las claves
//!   efímeras (PEM, `SignatureValue`, `X509Certificate`, firmas, huellas) para
//!   guardar fixtures sin material criptográfico.

use std::collections::BTreeMap;

use serde_json::Value;

const RANDOM_TAGS: [&str; 1] = ["CipherValue"];
const GUID_DEPENDENT_TAGS: [&str; 2] = ["DigestValue", "SignatureValue"];
const KEY_TAGS: [&str; 3] = ["SignatureValue", "X509Certificate", "CipherValue"];

fn is_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

/// Posición de un GUID `pfxXXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX` desde `start`.
fn find_guid(s: &str, start: usize) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut i = start;
    while let Some(off) = s[i..].find("pfx") {
        let p = i + off;
        let body = p + 3;
        let groups = [8, 4, 4, 4, 12];
        let mut q = body;
        let mut ok = true;
        for (gi, len) in groups.iter().enumerate() {
            if q + len > bytes.len() || !bytes[q..q + len].iter().all(|b| is_hex(*b)) {
                ok = false;
                break;
            }
            q += len;
            if gi < 4 {
                if bytes.get(q) != Some(&b'-') {
                    ok = false;
                    break;
                }
                q += 1;
            }
        }
        if ok {
            return Some((p, q));
        }
        i = p + 3;
    }
    None
}

fn replace_guids(s: &str, map: &mut BTreeMap<String, String>) -> (String, bool) {
    let mut out = String::new();
    let mut last = 0;
    let mut found = false;
    while let Some((a, b)) = find_guid(s, last) {
        found = true;
        out.push_str(&s[last..a]);
        let guid = &s[a..b];
        let n = map.len();
        let placeholder = map
            .entry(guid.to_owned())
            .or_insert_with(|| format!("<guid-{}>", n + 1))
            .clone();
        out.push_str(&placeholder);
        last = b;
    }
    out.push_str(&s[last..]);
    (out, found)
}

/// Reemplaza el contenido de `<prefijo:Tag …>contenido</prefijo:Tag>`.
fn redact_tags(s: &str, tags: &[&str], replacement: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    'outer: while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        let name_end = after
            .find(|c: char| c == '>' || c.is_whitespace() || c == '/')
            .unwrap_or(after.len());
        let name = &after[..name_end];
        let local = name.rsplit(':').next().unwrap_or(name);
        if !name.starts_with('/') && tags.contains(&local) {
            if let Some(gt) = after.find('>') {
                if !after[..gt].ends_with('/') {
                    let content = &after[gt + 1..];
                    let close = format!("</{name}>");
                    if let Some(end) = content.find(&close) {
                        out.push_str(&rest[..lt + 1 + gt + 1]);
                        out.push_str(replacement);
                        out.push_str(&close);
                        rest = &content[end + close.len()..];
                        continue 'outer;
                    }
                }
            }
        }
        out.push_str(&rest[..=lt]);
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Sustituye bloques PEM completos por `<pem:TIPO>`.
fn redact_pem(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("-----BEGIN ") {
        let after = &rest[start + 11..];
        let Some(dash) = after.find("-----") else {
            break;
        };
        let kind = after[..dash].to_owned();
        let end_marker = format!("-----END {kind}-----");
        let Some(end) = rest[start..].find(&end_marker) else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&format!("<pem:{kind}>"));
        rest = &rest[start + end + end_marker.len()..];
    }
    out.push_str(rest);
    out
}

fn walk(v: &mut Value, f: &mut dyn FnMut(Option<&str>, &mut Value)) {
    match v {
        Value::Object(map) => {
            for (k, child) in map.iter_mut() {
                f(Some(k), child);
                walk(child, f);
            }
        }
        Value::Array(list) => {
            for child in list {
                f(None, child);
                walk(child, f);
            }
        }
        _ => {}
    }
}

/// Un documento XML no UTF-8 viaja como `{"b64": …}`; para poder redactarlo
/// se convierte en texto leyendo cada byte como ISO-8859-1, con el prefijo
/// `<latin1>`. Solo afecta a valores que empiezan por `<`.
fn latin1_document(v: &mut Value) {
    use base64::Engine as _;
    let Some(b64) = v
        .as_object()
        .filter(|m| m.len() == 1)
        .and_then(|m| m.get("b64"))
        .and_then(Value::as_str)
    else {
        return;
    };
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) else {
        return;
    };
    if bytes.first() == Some(&b'<') {
        let text: String = bytes.iter().copied().map(char::from).collect();
        *v = Value::String(format!("<latin1>{text}"));
    }
}

/// Normaliza la aleatoriedad de un resultado de caso (`{"result"|"error"}`).
pub(crate) fn normalize_random(case: &mut Value) {
    let mut map = BTreeMap::new();
    let mut found_guid = false;
    let mut root = std::mem::take(case);
    let mut wrapper = serde_json::json!({ "v": root });
    latin1_document(&mut wrapper["v"]);
    walk(&mut wrapper, &mut |key, v| {
        latin1_document(v);
        if key == Some("session_key") {
            *v = Value::String("<aleatorio>".into());
            return;
        }
        if let Value::String(s) = v {
            let (replaced, found) = replace_guids(s, &mut map);
            found_guid |= found;
            *s = redact_tags(&replaced, &RANDOM_TAGS, "<aleatorio>");
        }
    });
    if found_guid {
        walk(&mut wrapper, &mut |_, v| {
            if let Value::String(s) = v {
                *s = redact_tags(s, &GUID_DEPENDENT_TAGS, "<depende-de-guid>");
            }
        });
    }
    root = wrapper["v"].take();
    *case = root;
}

/// Retira valores que dependen de las claves efímeras.
pub(crate) fn redact_keys(case: &mut Value) {
    latin1_document(case);
    walk(case, &mut |key, v| {
        latin1_document(v);
        if matches!(
            key,
            Some("signature" | "thumbprint" | "subjectKeyIdentifier" | "authorityKeyIdentifier")
        ) && !v.is_null()
        {
            *v = Value::String("<depende-de-clave>".into());
            return;
        }
        if let Value::String(s) = v {
            *s = redact_tags(&redact_pem(s), &KEY_TAGS, "<depende-de-clave>");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redactions() {
        let (s, found) = replace_guids(
            "a pfx0123abcd-0123-4567-89ab-0123456789ab b",
            &mut BTreeMap::new(),
        );
        assert!(found);
        assert_eq!(s, "a <guid-1> b");
        assert_eq!(
            redact_tags(
                "<ds:SignatureValue>QUJD</ds:SignatureValue>",
                &["SignatureValue"],
                "X"
            ),
            "<ds:SignatureValue>X</ds:SignatureValue>"
        );
        assert_eq!(
            redact_pem("x-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\ny"),
            "x<pem:CERTIFICATE>\ny"
        );
    }
}
