//! Comprobantes de retención.

use crate::document::join_parts;
use crate::model::client::Client;
use crate::model::company::Company;
use crate::{CoreError, Decimal, Document, Zoned};

/// Tipo de cambio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Exchange {
    /// Moneda de referencia.
    pub moneda_ref: Option<String>,
    /// Moneda objetivo.
    pub moneda_obj: Option<String>,
    /// Factor de cambio.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub factor: Option<Decimal>,
    /// Fecha del tipo de cambio.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha: Option<Zoned>,
}

/// Pago o cobro.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Payment {
    /// Moneda.
    pub moneda: Option<String>,
    /// Importe.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub importe: Option<Decimal>,
    /// Fecha.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha: Option<Zoned>,
}

/// Comprobante de retención.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Retention {
    /// Serie.
    pub serie: Option<String>,
    /// Correlativo.
    pub correlativo: Option<String>,
    /// Fecha de emisión.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_emision: Option<Zoned>,
    /// Emisor (agente de retención).
    pub company: Option<Company>,
    /// Proveedor.
    pub proveedor: Option<Client>,
    /// Régimen de retención.
    pub regimen: Option<String>,
    /// Tasa.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub tasa: Option<Decimal>,
    /// Importe retenido.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_retenido: Option<Decimal>,
    /// Importe pagado.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_pagado: Option<Decimal>,
    /// Observación.
    pub observacion: Option<String>,
    /// Comprobantes relacionados, en orden.
    pub details: Option<Vec<RetentionDetail>>,
}

/// Comprobante relacionado de una retención.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct RetentionDetail {
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
    /// Pagos.
    pub pagos: Option<Vec<Payment>>,
    /// Fecha de retención.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_retencion: Option<Zoned>,
    /// Importe retenido.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_retenido: Option<Decimal>,
    /// Importe por pagar.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub imp_pagar: Option<Decimal>,
    /// Tipo de cambio.
    pub tipo_cambio: Option<Exchange>,
}

impl Document for Retention {
    /// `RUC-20-SERIE-CORRELATIVO`.
    fn name(&self) -> Result<String, CoreError> {
        let company = self
            .company
            .as_ref()
            .ok_or(CoreError::missing("Retention", "company"))?;
        Ok(join_parts(&[
            company.ruc.as_deref(),
            Some("20"),
            self.serie.as_deref(),
            self.correlativo.as_deref(),
        ]))
    }
}
