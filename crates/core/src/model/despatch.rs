//! Guías de remisión.

use crate::document::join_parts;
use crate::model::client::Client;
use crate::model::company::Company;
use crate::model::sale::DetailAttribute;
use crate::model::sale::RelatedDocument;
use crate::{CoreError, Decimal, Document, Zoned};

/// Guía de remisión.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Despatch {
    /// Versión de la guía.
    pub version: Option<String>,
    /// Tipo de documento (`09` remitente, `31` transportista).
    pub tipo_doc: Option<String>,
    /// Serie.
    pub serie: Option<String>,
    /// Correlativo.
    pub correlativo: Option<String>,
    /// Observación.
    pub observacion: Option<String>,
    /// Fecha de emisión.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_emision: Option<Zoned>,
    /// Emisor.
    pub company: Option<Company>,
    /// Destinatario.
    pub destinatario: Option<Client>,
    /// Tercero.
    pub tercero: Option<Client>,
    /// Comprador.
    pub comprador: Option<Client>,
    /// Datos del envío.
    pub envio: Option<Shipment>,
    /// Documento dado de baja.
    pub doc_baja: Option<RelatedDocument>,
    /// Documento relacionado.
    pub rel_doc: Option<RelatedDocument>,
    /// Documentos adicionales.
    pub add_docs: Option<Vec<AdditionalDoc>>,
    /// Bienes trasladados, en orden.
    pub details: Option<Vec<DespatchDetail>>,
}

/// Documento adicional de una guía.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct AdditionalDoc {
    /// Descripción del tipo.
    pub tipo_desc: Option<String>,
    /// Tipo.
    pub tipo: Option<String>,
    /// Número. Leerlo con [`AdditionalDoc::nro`] reproduce el fallo de la referencia cuando es `None`.
    pub nro: Option<String>,
    /// RUC del emisor.
    pub emisor: Option<String>,
}

/// Bien trasladado.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct DespatchDetail {
    /// Código.
    pub codigo: Option<String>,
    /// Descripción.
    pub descripcion: Option<String>,
    /// Unidad de medida.
    pub unidad: Option<String>,
    /// Cantidad.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub cantidad: Option<Decimal>,
    /// Código de producto SUNAT.
    pub cod_prod_sunat: Option<String>,
    /// Atributos adicionales.
    pub atributos: Option<Vec<DetailAttribute>>,
}

/// Punto de partida o llegada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Direction {
    /// Ubigeo (propiedad `ubigueo`).
    pub ubigueo: Option<String>,
    /// Dirección.
    pub direccion: Option<String>,
    /// Código de establecimiento.
    pub cod_local: Option<String>,
    /// RUC asociado al establecimiento.
    pub ruc: Option<String>,
}

/// Conductor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Driver {
    /// Tipo (`Principal` o `Secundario`).
    pub tipo: Option<String>,
    /// Tipo de documento.
    pub tipo_doc: Option<String>,
    /// Número de documento.
    pub nro_doc: Option<String>,
    /// Nombres.
    pub nombres: Option<String>,
    /// Apellidos.
    pub apellidos: Option<String>,
    /// Licencia de conducir.
    pub licencia: Option<String>,
}

/// Puerto o aeropuerto.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Puerto {
    /// Código.
    pub codigo: Option<String>,
    /// Nombre.
    pub nombre: Option<String>,
}

/// Datos del envío de una guía.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Shipment {
    /// Motivo de traslado (catálogo 20).
    pub cod_traslado: Option<String>,
    /// Descripción del motivo.
    pub des_traslado: Option<String>,
    /// Sustento de la diferencia de peso.
    pub sustento_peso: Option<String>,
    /// Indicador de transbordo.
    pub ind_transbordo: Option<bool>,
    /// Indicadores especiales.
    pub indicadores: Option<Vec<String>>,
    /// Peso de los ítems.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub peso_items: Option<Decimal>,
    /// Peso bruto total.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub peso_total: Option<Decimal>,
    /// Unidad del peso total.
    pub und_peso_total: Option<String>,
    /// Número de bultos.
    pub num_bultos: Option<i64>,
    /// Modalidad de traslado (catálogo 18).
    pub mod_traslado: Option<String>,
    /// Fecha de inicio del traslado. En la referencia el setter exige `DateTime` mutable.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_traslado: Option<Zoned>,
    /// Fecha de entrega de bienes. En la referencia el setter exige `DateTime` mutable.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_entrega_bienes: Option<Zoned>,
    /// Número de contenedor.
    pub num_contenedor: Option<String>,
    /// Contenedores.
    pub contenedores: Option<Vec<String>>,
    /// Código de puerto.
    pub cod_puerto: Option<String>,
    /// Puerto.
    pub puerto: Option<Puerto>,
    /// Aeropuerto.
    pub aeropuerto: Option<Puerto>,
    /// Transportista.
    pub transportista: Option<Transportist>,
    /// Vehículo principal.
    pub vehiculo: Option<Vehicle>,
    /// Conductores.
    pub choferes: Option<Vec<Driver>>,
    /// Punto de llegada.
    pub llegada: Option<Direction>,
    /// Punto de partida.
    pub partida: Option<Direction>,
}

/// Transportista.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Transportist {
    /// Tipo de documento.
    pub tipo_doc: Option<String>,
    /// Número de documento.
    pub num_doc: Option<String>,
    /// Razón social.
    pub rzn_social: Option<String>,
    /// Registro MTC.
    pub nro_mtc: Option<String>,
    /// Placa.
    pub placa: Option<String>,
    /// Tipo de documento del conductor.
    pub chofer_tipo_doc: Option<String>,
    /// Documento del conductor.
    pub chofer_doc: Option<String>,
}

/// Vehículo, con vehículos secundarios anidados.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Vehicle {
    /// Placa.
    pub placa: Option<String>,
    /// Tarjeta única de circulación.
    pub nro_circulacion: Option<String>,
    /// Entidad emisora de la autorización.
    pub cod_emisor: Option<String>,
    /// Número de autorización.
    pub nro_autorizacion: Option<String>,
    /// Vehículos secundarios.
    pub secundarios: Option<Vec<Vehicle>>,
}

impl Document for Despatch {
    /// `RUC-TIPO-SERIE-CORRELATIVO`, por ejemplo `20000000001-09-T001-12`.
    fn name(&self) -> Result<String, CoreError> {
        let company = self
            .company
            .as_ref()
            .ok_or(CoreError::missing("Despatch", "company"))?;
        Ok(join_parts(&[
            company.ruc.as_deref(),
            self.tipo_doc.as_deref(),
            self.serie.as_deref(),
            self.correlativo.as_deref(),
        ]))
    }
}

impl AdditionalDoc {
    /// Número del documento.
    ///
    /// `getNro()` declara un retorno `string` no nulo, así que en la referencia leer un
    /// `nro` nulo lanza `TypeError`; aquí devuelve un error.
    ///
    /// # Errors
    ///
    /// [`CoreError::MissingField`] si `nro` es `None`.
    pub fn nro(&self) -> Result<&str, CoreError> {
        self.nro
            .as_deref()
            .ok_or(CoreError::missing("AdditionalDoc", "nro"))
    }
}

impl Direction {
    /// Punto con ubigeo y dirección (`new Direction($ubigueo, $direccion)`).
    #[must_use]
    pub fn new(ubigueo: Option<String>, direccion: Option<String>) -> Self {
        Self {
            ubigueo,
            direccion,
            cod_local: None,
            ruc: None,
        }
    }
}
