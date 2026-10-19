//! Comunicaciones de baja y resúmenes de reversión.

use crate::document::join_parts;
use crate::model::company::Company;
use crate::time::{format_ymd, to_peru_time};
use crate::{CoreError, Document, Zoned};

/// Comunicación de baja (RA).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Voided {
    /// Correlativo.
    pub correlativo: Option<String>,
    /// Fecha de generación de los comprobantes.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_generacion: Option<Zoned>,
    /// Fecha de comunicación; determina el identificador en hora de Perú.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_comunicacion: Option<Zoned>,
    /// Emisor.
    pub company: Option<Company>,
    /// Comprobantes dados de baja, en orden.
    pub details: Option<Vec<VoidedDetail>>,
}

/// Comprobante dado de baja.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct VoidedDetail {
    /// Tipo de comprobante.
    pub tipo_doc: Option<String>,
    /// Serie.
    pub serie: Option<String>,
    /// Correlativo.
    pub correlativo: Option<String>,
    /// Motivo de la baja.
    pub des_motivo_baja: Option<String>,
}

impl Voided {
    fn identifier(&self, model: &'static str, prefix: &str) -> Result<String, CoreError> {
        let date = self
            .fec_comunicacion
            .as_ref()
            .ok_or(CoreError::missing(model, "fecComunicacion"))?;
        let date = format_ymd(to_peru_time(date)?.date());
        Ok(join_parts(&[
            Some(prefix),
            Some(&date),
            self.correlativo.as_deref(),
        ]))
    }

    fn document_name(&self, model: &'static str, prefix: &str) -> Result<String, CoreError> {
        let company = self
            .company
            .as_ref()
            .ok_or(CoreError::missing(model, "company"))?;
        let ruc = company.ruc.as_deref().unwrap_or_default();
        Ok(format!("{ruc}-{}", self.identifier(model, prefix)?))
    }

    /// Identificador `RA-AAAAMMDD-CORRELATIVO`, con la fecha de
    /// `fecComunicacion` convertida a la hora de Perú (`getXmlId()`).
    ///
    /// # Errors
    ///
    /// [`CoreError::MissingField`] sin `fecComunicacion`;
    /// [`CoreError::TimeZone`] si la zona de Perú no está disponible.
    pub fn xml_id(&self) -> Result<String, CoreError> {
        self.identifier("Voided", "RA")
    }
}

impl Document for Voided {
    /// `RUC-RA-AAAAMMDD-CORRELATIVO`. Se comprueba el emisor antes que la fecha.
    fn name(&self) -> Result<String, CoreError> {
        self.document_name("Voided", "RA")
    }
}

/// Resumen de reversiones (RR).
///
/// Contiene una baja ([`Voided`]) y solo cambia el prefijo del identificador.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Reversion {
    /// Datos de la comunicación, idénticos a los de una baja.
    pub voided: Voided,
}

impl Reversion {
    /// Identificador `RR-AAAAMMDD-CORRELATIVO` en hora de Perú.
    ///
    /// # Errors
    ///
    /// Los mismos que [`Voided::xml_id`].
    pub fn xml_id(&self) -> Result<String, CoreError> {
        self.voided.identifier("Reversion", "RR")
    }
}

impl Document for Reversion {
    /// `RUC-RR-AAAAMMDD-CORRELATIVO`.
    fn name(&self) -> Result<String, CoreError> {
        self.voided.document_name("Reversion", "RR")
    }
}
