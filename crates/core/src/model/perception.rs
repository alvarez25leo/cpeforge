//! Comprobantes de percepción.

use crate::document::join_parts;
use crate::model::client::Client;
use crate::model::company::Company;
use crate::model::retention::Exchange;
use crate::model::retention::Payment;
use crate::{CoreError, Decimal, Document, Zoned};

/// Comprobante de percepción.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Perception {
    /// Serie.
    pub serie: Option<String>,
    /// Correlativo.
    pub correlativo: Option<String>,
    /// Fecha de emisión.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_emision: Option<Zoned>,
    /// Emisor (agente de percepción).
    pub company: Option<Company>,
    /// Cliente (propiedad `proveedor`).
    pub proveedor: Option<Client>,
    /// Régimen de percepción.
    pub regimen: Option<String>,
    /// Tasa.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub tasa: Option<Decimal>,
    /// Importe percibido.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_percibido: Option<Decimal>,
    /// Importe cobrado.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_cobrado: Option<Decimal>,
    /// Observación.
    pub observacion: Option<String>,
    /// Comprobantes relacionados, en orden.
    pub details: Option<Vec<PerceptionDetail>>,
}

/// Comprobante relacionado de una percepción.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct PerceptionDetail {
    /// Tipo de documento.
    pub tipo_doc: Option<String>,
    /// Número de documento.
    pub num_doc: Option<String>,
    /// Fecha de emisión.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_emision: Option<Zoned>,
    /// Importe total.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_total: Option<Decimal>,
    /// Moneda.
    pub moneda: Option<String>,
    /// Cobros.
    pub cobros: Option<Vec<Payment>>,
    /// Fecha de percepción.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_percepcion: Option<Zoned>,
    /// Importe percibido.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_percibido: Option<Decimal>,
    /// Importe por cobrar.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_cobrar: Option<Decimal>,
    /// Tipo de cambio.
    pub tipo_cambio: Option<Exchange>,
}

impl Document for Perception {
    /// `RUC-40-SERIE-CORRELATIVO`.
    fn name(&self) -> Result<String, CoreError> {
        let company = self
            .company
            .as_ref()
            .ok_or(CoreError::missing("Perception", "company"))?;
        Ok(join_parts(&[
            company.ruc.as_deref(),
            Some("40"),
            self.serie.as_deref(),
            self.correlativo.as_deref(),
        ]))
    }
}
