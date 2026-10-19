//! Receptores y otras personas identificadas.

use crate::model::company::Address;

/// Receptor, proveedor, transportista u otra persona identificada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Client {
    /// Tipo de documento de identidad (catálogo 06).
    pub tipo_doc: Option<String>,
    /// Número de documento de identidad.
    pub num_doc: Option<String>,
    /// Razón social o nombres.
    pub rzn_social: Option<String>,
    /// Dirección.
    pub address: Option<Address>,
    /// Correo electrónico.
    pub email: Option<String>,
    /// Teléfono.
    pub telephone: Option<String>,
}
