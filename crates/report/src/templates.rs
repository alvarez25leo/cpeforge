//! Plantillas integradas y carga desde la carpeta del usuario.
//!
//! La carpeta del usuario se consulta antes que las plantillas integradas, de
//! modo que un archivo con el mismo nombre (`invoice.html.twig`,
//! `assets/style.css`) las sustituye.
//!
//! Validación de nombres: se normalizan `\` y `//`, se ignoran las `/`
//! iniciales y se rechaza un nombre que sube por encima de la raíz con `..`,
//! tiene unidad o raíz, o contiene NUL. Además, el archivo encontrado debe
//! seguir dentro de la carpeta del usuario después de resolver enlaces
//! simbólicos y no puede superar [`crate::ReportOptions::max_template_bytes`].

use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use crate::error::ReportError;

/// Plantillas integradas: nombre y contenido. Incluyen material de terceros
/// bajo licencia MIT (ver `LICENSE-THIRD-PARTY` y `NOTICE`).
pub const BUILTIN_TEMPLATES: &[(&str, &str)] = &[
    (
        "invoice.html.twig",
        include_str!("../templates/invoice.html.twig"),
    ),
    (
        "invoice2.html.twig",
        include_str!("../templates/invoice2.html.twig"),
    ),
    (
        "despatch.html.twig",
        include_str!("../templates/despatch.html.twig"),
    ),
    (
        "perception.html.twig",
        include_str!("../templates/perception.html.twig"),
    ),
    (
        "retention.html.twig",
        include_str!("../templates/retention.html.twig"),
    ),
    (
        "summary.html.twig",
        include_str!("../templates/summary.html.twig"),
    ),
    (
        "voided.html.twig",
        include_str!("../templates/voided.html.twig"),
    ),
    (
        "settlement.html.twig",
        include_str!("../templates/settlement.html.twig"),
    ),
    (
        "assets/style.css",
        include_str!("../templates/assets/style.css"),
    ),
];

/// Nombre normalizado y validado como `FilesystemLoader::findTemplate`.
pub(crate) fn normalize(name: &str) -> Result<String, ReportError> {
    let invalid = |reason| ReportError::InvalidTemplateName {
        name: name.to_owned(),
        reason,
    };
    if name.contains('\0') {
        return Err(invalid("contiene NUL"));
    }
    let mut normalized = name.replace('\\', "/");
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    if normalized.starts_with('@') {
        // Nombres con espacio de nombres (`@ns/x`): no hay ninguno registrado.
        return Err(ReportError::TemplateNotFound {
            name: name.to_owned(),
        });
    }
    let trimmed = normalized.trim_start_matches('/');
    // Un prefijo de unidad (`C:`) o una raíz haría que `join` descartara la
    // carpeta del usuario (Windows); se rechaza en todas las plataformas.
    if Path::new(trimmed)
        .components()
        .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
    {
        return Err(invalid("ruta absoluta"));
    }
    let mut level = 0i64;
    for part in trimmed.split('/') {
        if part == ".." {
            level -= 1;
        } else if part != "." {
            level += 1;
        }
        if level < 0 {
            return Err(invalid("sale de las carpetas de plantillas"));
        }
    }
    Ok(trimmed.to_owned())
}

/// Resuelve `.` y `..` sobre las rutas integradas, cuyas únicas carpetas son
/// la raíz y `assets`.
fn builtin_lookup(name: &str) -> Option<&'static str> {
    let mut stack: Vec<&str> = Vec::new();
    let parts: Vec<&str> = name.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        match *part {
            "." => {}
            ".." => {
                stack.pop();
            }
            other => {
                stack.push(other);
                // Un componente intermedio debe ser una carpeta existente.
                let is_dir = stack.len() == 1 && other == "assets";
                if !(last || is_dir) {
                    return None;
                }
            }
        }
    }
    let joined = stack.join("/");
    BUILTIN_TEMPLATES
        .iter()
        .find(|(n, _)| *n == joined)
        .map(|(_, source)| *source)
}

/// Busca la plantilla en la carpeta del usuario y después entre las integradas.
pub(crate) fn load(
    user_dir: Option<&Path>,
    name: &str,
    max_bytes: u64,
) -> Result<Option<String>, ReportError> {
    let normalized = normalize(name)?;
    if let Some(dir) = user_dir {
        let candidate: PathBuf = dir.join(&normalized);
        if candidate.is_file() {
            return read_user(dir, &candidate, name, max_bytes).map(Some);
        }
    }
    Ok(builtin_lookup(&normalized).map(str::to_owned))
}

fn read_user(dir: &Path, path: &Path, name: &str, max_bytes: u64) -> Result<String, ReportError> {
    let io = |message: String| ReportError::TemplateIo {
        name: name.to_owned(),
        message,
    };
    let root = dir.canonicalize().map_err(|e| io(e.to_string()))?;
    let real = path.canonicalize().map_err(|e| io(e.to_string()))?;
    if !real.starts_with(&root) {
        return Err(ReportError::InvalidTemplateName {
            name: name.to_owned(),
            reason: "el enlace simbólico sale de la carpeta de plantillas",
        });
    }
    let file = fs::File::open(&real).map_err(|e| io(e.to_string()))?;
    // El archivo pudo cambiar entre la comprobación y la apertura: solo se
    // leen archivos regulares (no FIFO, dispositivos ni carpetas).
    let metadata = file.metadata().map_err(|e| io(e.to_string()))?;
    if !metadata.is_file() {
        return Err(io("no es un archivo regular".to_owned()));
    }
    let capacity = usize::try_from(metadata.len().min(max_bytes)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| io(e.to_string()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io(format!("supera el límite de {max_bytes} bytes")));
    }
    String::from_utf8(bytes).map_err(|_| io("no es UTF-8".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_validated_like_twig() {
        assert_eq!(normalize("invoice.html.twig").unwrap(), "invoice.html.twig");
        assert_eq!(normalize("/assets//style.css").unwrap(), "assets/style.css");
        assert_eq!(normalize("assets\\style.css").unwrap(), "assets/style.css");
        assert_eq!(
            normalize("a/../invoice.html.twig").unwrap(),
            "a/../invoice.html.twig"
        );
        assert!(matches!(
            normalize("../secret"),
            Err(ReportError::InvalidTemplateName { .. })
        ));
        assert!(matches!(
            normalize("assets/../../x"),
            Err(ReportError::InvalidTemplateName { .. })
        ));
        assert!(matches!(
            normalize("a\0b"),
            Err(ReportError::InvalidTemplateName { .. })
        ));
    }

    #[test]
    fn builtin_paths() {
        assert!(builtin_lookup("invoice.html.twig").is_some());
        assert!(builtin_lookup("assets/style.css").is_some());
        assert!(builtin_lookup("assets/../invoice.html.twig").is_some());
        assert!(builtin_lookup("x/../invoice.html.twig").is_none());
        assert!(builtin_lookup("nothing.html.twig").is_none());
    }
}
