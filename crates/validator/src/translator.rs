//! Equivalente de `MessageTranslator` con proveedor del contrato de `core`.

use cpeforge_core::ports::ErrorCodeProvider;
use std::collections::BTreeMap;

use crate::ValidatorError;

/// Traduce códigos consultando primero al proveedor y luego los recursos locales.
pub struct MessageTranslator<P> {
    provider: P,
    resources: BTreeMap<String, String>,
}

impl<P> std::fmt::Debug for MessageTranslator<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageTranslator")
            .field("messages", &self.resources.len())
            .finish_non_exhaustive()
    }
}

impl<P> MessageTranslator<P>
where
    P: ErrorCodeProvider,
    P::Error: std::fmt::Display,
{
    /// Crea un traductor sin recursos adicionales.
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            resources: BTreeMap::new(),
        }
    }

    /// Añade recursos. La referencia `array_merge` reindexa claves numéricas como `1003`;
    /// esas claves no se pueden recuperar por ID y se omiten aquí.
    pub fn add_resources(&mut self, resources: impl IntoIterator<Item = (String, String)>) {
        for (id, text) in resources {
            if !id
                .parse::<i64>()
                .is_ok_and(|number| number.to_string() == id)
            {
                self.resources.insert(id, text);
            }
        }
    }

    /// Devuelve `en`, el locale fijo de la referencia de referencia.
    pub fn locale(&self) -> &'static str {
        "en"
    }

    /// Traduce un identificador. Parámetros, dominio y locale son ignorados por la referencia.
    pub fn trans(
        &self,
        id: &str,
        _parameters: &BTreeMap<String, String>,
        _domain: Option<&str>,
        _locale: Option<&str>,
    ) -> Result<String, ValidatorError> {
        if let Some(value) = self
            .provider
            .value(Some(id))
            .map_err(|error| ValidatorError::Provider(error.to_string()))?
        {
            if !value.is_empty() && value != "0" {
                return Ok(value);
            }
        }
        Ok(self
            .resources
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.to_owned()))
    }
}
