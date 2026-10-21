//! Tabla de restricciones obtenida ejecutando los 40 loaders de la referencia fijados.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::ValidatorError;

/// Restricción Symfony y sus opciones observables.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Rule {
    /// Nombre de la restricción.
    pub name: String,
    /// Opciones públicas efectivas de Symfony 6.4.48.
    pub options: Value,
}

/// Restricciones de una propiedad en el orden de declaración de la referencia.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FieldRules {
    /// Nombre de la propiedad.
    pub field: String,
    /// Restricciones, en orden.
    pub rules: Vec<Rule>,
}

/// Metadata de un tipo de modelo y del perfil elegido.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Metadata {
    /// Variante efectiva de loader (`base` o `v21`).
    pub loader: String,
    /// Si hay loader de la referencia para el tipo.
    pub has_metadata: bool,
    /// Propiedades restringidas, en orden.
    pub fields: Vec<FieldRules>,
}

type Profile = BTreeMap<String, Metadata>;
type RuleTable = BTreeMap<String, Profile>;
static TABLE: OnceLock<Result<RuleTable, String>> = OnceLock::new();

fn table() -> Result<&'static RuleTable, ValidatorError> {
    TABLE
        .get_or_init(|| {
            serde_json::from_str(include_str!("rules.json")).map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| ValidatorError::RuleTable(error.clone()))
}

pub(crate) fn metadata(profile: &str, model: &str) -> Result<Metadata, ValidatorError> {
    Ok(metadata_ref(profile, model)?
        .cloned()
        .unwrap_or_else(|| Metadata {
            loader: String::new(),
            has_metadata: false,
            fields: Vec::new(),
        }))
}

/// Metadata de la tabla sin copiarla; `None` si el tipo no tiene loader.
pub(crate) fn metadata_ref(
    profile: &str,
    model: &str,
) -> Result<Option<&'static Metadata>, ValidatorError> {
    Ok(table()?.get(profile).and_then(|classes| classes.get(model)))
}

/// Los 33 loaders base y los siete loaders adicionales/sustituidos de UBL 2.1.
pub fn loader_names(profile: &str) -> Result<Vec<String>, ValidatorError> {
    let table = table()?;
    Ok(table
        .get(profile)
        .map(|classes| {
            classes
                .iter()
                .filter(|(_, metadata)| metadata.has_metadata)
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default())
}
