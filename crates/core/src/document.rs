use crate::CoreError;

/// Documento electrónico con nombre de archivo SUNAT.
///
/// El nombre se usa para los archivos XML/ZIP que generan y envían otros
/// crates.
pub trait Document {
    /// Nombre del documento sin extensión, por ejemplo `20000000001-01-F001-123`.
    ///
    /// # Errors
    ///
    /// [`CoreError::MissingField`] si falta el emisor (o, para resúmenes y
    /// bajas, la fecha), los mismos casos en que la referencia lanza un error.
    fn name(&self) -> Result<String, CoreError>;
}

/// Une las partes con `-` tratando `None` como cadena vacía, igual que
/// `join('-', [...])` con valores `null`.
pub(crate) fn join_parts(parts: &[Option<&str>]) -> String {
    let len: usize = parts.iter().map(|p| p.map_or(0, str::len) + 1).sum();
    let mut out = String::with_capacity(len);
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            out.push('-');
        }
        out.push_str(part.unwrap_or_default());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::join_parts;

    #[test]
    fn join_parts_treats_none_as_empty() {
        assert_eq!(join_parts(&[Some("a"), None, Some(""), Some("b")]), "a---b");
        assert_eq!(join_parts(&[]), "");
        assert_eq!(join_parts(&[None]), "");
    }
}
