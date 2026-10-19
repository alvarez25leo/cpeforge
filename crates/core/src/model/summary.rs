//! Resúmenes diarios.

use crate::document::join_parts;
use crate::model::company::Company;
use crate::model::sale::RelatedDocument;
use crate::time::{format_ymd, to_peru_time};
use crate::{CoreError, Decimal, Document, Zoned};

/// Resumen diario de boletas y notas (RC).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Summary {
    /// Correlativo del resumen.
    pub correlativo: Option<String>,
    /// Fecha de generación de los comprobantes.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_generacion: Option<Zoned>,
    /// Fecha del resumen; determina el identificador en hora de Perú.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_resumen: Option<Zoned>,
    /// Moneda; por defecto `PEN`.
    pub moneda: Option<String>,
    /// Emisor.
    pub company: Option<Company>,
    /// Comprobantes resumidos, en orden.
    pub details: Option<Vec<SummaryDetail>>,
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            correlativo: None,
            fec_generacion: None,
            fec_resumen: None,
            moneda: Some("PEN".to_owned()),
            company: None,
            details: None,
        }
    }
}

/// Comprobante incluido en un resumen diario.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct SummaryDetail {
    /// Tipo de comprobante.
    pub tipo_doc: Option<String>,
    /// Serie y número.
    pub serie_nro: Option<String>,
    /// Tipo de documento del cliente.
    pub cliente_tipo: Option<String>,
    /// Número de documento del cliente.
    pub cliente_nro: Option<String>,
    /// Documento de referencia (notas).
    pub doc_referencia: Option<RelatedDocument>,
    /// Percepción.
    pub percepcion: Option<SummaryPerception>,
    /// Estado del ítem (catálogo 19).
    pub estado: Option<String>,
    /// Importe total.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total: Option<Decimal>,
    /// Porcentaje de IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje_igv: Option<Decimal>,
    /// Operaciones gravadas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_gravadas: Option<Decimal>,
    /// Operaciones inafectas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_inafectas: Option<Decimal>,
    /// Operaciones exoneradas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_exoneradas: Option<Decimal>,
    /// Operaciones de exportación.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_exportacion: Option<Decimal>,
    /// Operaciones gratuitas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_gratuitas: Option<Decimal>,
    /// Otros cargos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_otros_cargos: Option<Decimal>,
    /// IGV.
    #[cfg_attr(feature = "serde", serde(rename = "mtoIGV"))]
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_igv: Option<Decimal>,
    /// IVAP.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_ivap: Option<Decimal>,
    /// ISC.
    #[cfg_attr(feature = "serde", serde(rename = "mtoISC"))]
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_isc: Option<Decimal>,
    /// Otros tributos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_otros_tributos: Option<Decimal>,
    /// ICBPER.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_icbper: Option<Decimal>,
}

/// Percepción de un ítem del resumen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct SummaryPerception {
    /// Régimen de percepción.
    pub cod_reg: Option<String>,
    /// Tasa.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub tasa: Option<Decimal>,
    /// Base imponible.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base: Option<Decimal>,
    /// Monto de la percepción.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto: Option<Decimal>,
    /// Monto total incluida la percepción.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_total: Option<Decimal>,
}

impl Summary {
    /// Identificador `RC-AAAAMMDD-CORRELATIVO`, con la fecha de `fecResumen`
    /// convertida a la hora de Perú (`getXmlId()`).
    ///
    /// # Errors
    ///
    /// [`CoreError::MissingField`] sin `fecResumen`; [`CoreError::TimeZone`]
    /// si la zona de Perú no está disponible.
    pub fn xml_id(&self) -> Result<String, CoreError> {
        let date = self
            .fec_resumen
            .as_ref()
            .ok_or(CoreError::missing("Summary", "fecResumen"))?;
        let date = format_ymd(to_peru_time(date)?.date());
        Ok(join_parts(&[
            Some("RC"),
            Some(&date),
            self.correlativo.as_deref(),
        ]))
    }
}

impl Document for Summary {
    /// `RUC-RC-AAAAMMDD-CORRELATIVO`. Se comprueba el emisor antes que la
    /// fecha, en el mismo orden en que la referencia evalúa la expresión.
    fn name(&self) -> Result<String, CoreError> {
        let company = self
            .company
            .as_ref()
            .ok_or(CoreError::missing("Summary", "company"))?;
        let ruc = company.ruc.as_deref().unwrap_or_default();
        Ok(format!("{ruc}-{}", self.xml_id()?))
    }
}
