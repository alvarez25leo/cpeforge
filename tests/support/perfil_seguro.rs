//! Validación compartida de los registros `tests/reference/*/differences.json`
//! según el perfil seguro: toda diferencia necesita clasificación, motivo
//! admitido y nota. Los comparadores
//! de cada crate la incluyen con `#[path]`.

use serde_json::Value;

/// Motivos admitidos para una `DIFERENCIA_DELIBERADA`. `conformidad-xsd`:
/// la salida de la referencia no valida contra el esquema UBL/SUNAT y Rust
/// la corrige (comprobado en `crates/xml/tests/schema.rs`).
/// `catalogo-sunat-vigente`: dato que SUNAT actualizó después de la referencia
/// (por ejemplo, textos de `CódigosRetorno` en `cpeforge-xcodes`).
pub(crate) const MOTIVOS: [&str; 12] = [
    "seguridad",
    "conformidad-xsd",
    "catalogo-sunat-vigente",
    "precision-decimal",
    "rechazo-explicito",
    "tipado-estatico",
    "diagnostico-runtime",
    "rango-de-tipo",
    "compresion-equivalente",
    "codificacion-fuera-de-alcance",
    "dependiente-de-iconv",
    "dependiente-de-libxml",
];

/// Comprueba una entrada del registro. Una `DIFERENCIA_DELIBERADA` necesita
/// motivo admitido, `reason` y `compatibility_note`; una `CORREGIR`
/// (pendiente que bloquea `LISTO`) necesita `reason` y `next_step`.
///
/// # Errors
///
/// Describe el primer campo ausente o inválido.
pub(crate) fn validate(entry: &Value) -> Result<(), String> {
    let text = |field: &str| entry[field].as_str().is_some_and(|v| !v.trim().is_empty());
    let required: &[&str] = match entry["classification"].as_str() {
        Some("DIFERENCIA_DELIBERADA") => {
            if !MOTIVOS.contains(&entry["motivo"].as_str().unwrap_or_default()) {
                return Err(format!("motivo fuera del perfil seguro: {entry}"));
            }
            &["reason", "compatibility_note"]
        }
        Some("CORREGIR") => &["reason", "next_step"],
        _ => return Err(format!("classification inválida: {entry}")),
    };
    match required.iter().find(|field| !text(field)) {
        Some(field) => Err(format!("falta {field}: {entry}")),
        None => Ok(()),
    }
}
