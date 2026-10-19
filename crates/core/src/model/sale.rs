//! Facturas, boletas, notas y recibos por honorarios.

use crate::document::join_parts;
use crate::model::client::Client;
use crate::model::company::Address;
use crate::model::company::Company;
use crate::model::despatch::Direction;
use crate::{CoreError, Decimal, Document, Zoned};

/// Datos comunes de facturas, boletas y notas.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct BaseSale {
    /// Versión UBL; por defecto `2.0`.
    pub ubl_version: Option<String>,
    /// Tipo de comprobante (catálogo 01).
    pub tipo_doc: Option<String>,
    /// Serie.
    pub serie: Option<String>,
    /// Correlativo.
    pub correlativo: Option<String>,
    /// Fecha y hora de emisión.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_emision: Option<Zoned>,
    /// Emisor.
    pub company: Option<Company>,
    /// Adquirente o usuario.
    pub client: Option<Client>,
    /// Moneda (ISO 4217).
    pub tipo_moneda: Option<String>,
    /// Sumatoria de otros cargos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub sum_otros_cargos: Option<Decimal>,
    /// Total de operaciones gravadas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_gravadas: Option<Decimal>,
    /// Total de operaciones inafectas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_inafectas: Option<Decimal>,
    /// Total de operaciones exoneradas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_exoneradas: Option<Decimal>,
    /// Total de operaciones de exportación.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_exportacion: Option<Decimal>,
    /// Total de operaciones gratuitas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_gratuitas: Option<Decimal>,
    /// IGV de operaciones gratuitas.
    #[cfg_attr(feature = "serde", serde(rename = "mtoIGVGratuitas"))]
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_igv_gratuitas: Option<Decimal>,
    /// Total IGV.
    #[cfg_attr(feature = "serde", serde(rename = "mtoIGV"))]
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_igv: Option<Decimal>,
    /// Base imponible IVAP.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_ivap: Option<Decimal>,
    /// Total IVAP.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_ivap: Option<Decimal>,
    /// Base imponible ISC.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_isc: Option<Decimal>,
    /// Total ISC.
    #[cfg_attr(feature = "serde", serde(rename = "mtoISC"))]
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_isc: Option<Decimal>,
    /// Base imponible de otros tributos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_oth: Option<Decimal>,
    /// Total de otros tributos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_otros_tributos: Option<Decimal>,
    /// Impuesto a las bolsas plásticas (ICBPER).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub icbper: Option<Decimal>,
    /// Total de impuestos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total_impuestos: Option<Decimal>,
    /// Redondeo del importe total.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub redondeo: Option<Decimal>,
    /// Importe total de la venta.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_imp_venta: Option<Decimal>,
    /// Líneas del comprobante, en orden.
    pub details: Option<Vec<SaleDetail>>,
    /// Leyendas.
    pub legends: Option<Vec<Legend>>,
    /// Guías de remisión relacionadas.
    pub guias: Option<Vec<RelatedDocument>>,
    /// Otros documentos relacionados.
    pub rel_docs: Option<Vec<RelatedDocument>>,
    /// Número de orden de compra.
    pub compra: Option<String>,
    /// Forma de pago.
    pub forma_pago: Option<PaymentTerms>,
    /// Cuotas de pago a crédito.
    pub cuotas: Option<Vec<Cuota>>,
}

impl Default for BaseSale {
    fn default() -> Self {
        Self {
            ubl_version: Some("2.0".to_owned()),
            tipo_doc: None,
            serie: None,
            correlativo: None,
            fecha_emision: None,
            company: None,
            client: None,
            tipo_moneda: None,
            sum_otros_cargos: None,
            mto_oper_gravadas: None,
            mto_oper_inafectas: None,
            mto_oper_exoneradas: None,
            mto_oper_exportacion: None,
            mto_oper_gratuitas: None,
            mto_igv_gratuitas: None,
            mto_igv: None,
            mto_base_ivap: None,
            mto_ivap: None,
            mto_base_isc: None,
            mto_isc: None,
            mto_base_oth: None,
            mto_otros_tributos: None,
            icbper: None,
            total_impuestos: None,
            redondeo: None,
            mto_imp_venta: None,
            details: None,
            legends: None,
            guias: None,
            rel_docs: None,
            compra: None,
            forma_pago: None,
            cuotas: None,
        }
    }
}

/// Factura o boleta de venta.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
pub struct Invoice {
    /// Campos heredados de `BaseSale`.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub base: BaseSale,
    /// Tipo de operación (catálogo 51).
    pub tipo_operacion: Option<String>,
    /// Fecha de vencimiento.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_vencimiento: Option<Zoned>,
    /// Sumatoria de descuentos globales.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub sum_dscto_global: Option<Decimal>,
    /// Total de descuentos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_descuentos: Option<Decimal>,
    /// Sumatoria de otros descuentos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub sum_otros_descuentos: Option<Decimal>,
    /// Descuentos globales.
    pub descuentos: Option<Vec<Charge>>,
    /// Cargos globales.
    pub cargos: Option<Vec<Charge>>,
    /// Total de cargos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_cargos: Option<Decimal>,
    /// Total de anticipos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total_anticipos: Option<Decimal>,
    /// Percepción de la venta.
    pub perception: Option<SalePerception>,
    /// Guía de remisión embebida.
    pub guia_embebida: Option<EmbededDespatch>,
    /// Anticipos.
    pub anticipos: Option<Vec<Prepayment>>,
    /// Detracción.
    pub detraccion: Option<Detraction>,
    /// Vendedor (venta itinerante).
    pub seller: Option<Client>,
    /// Valor de venta.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub valor_venta: Option<Decimal>,
    /// Subtotal (precio de venta).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub sub_total: Option<Decimal>,
    /// Observación.
    pub observacion: Option<String>,
    /// Dirección de entrega.
    pub direccion_entrega: Option<Address>,
}

/// Nota de crédito o débito.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
pub struct Note {
    /// Campos heredados de `BaseSale`.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub base: BaseSale,
    /// Código de motivo (catálogos 09/10).
    pub cod_motivo: Option<String>,
    /// Descripción del motivo.
    pub des_motivo: Option<String>,
    /// Tipo del comprobante afectado.
    pub tip_doc_afectado: Option<String>,
    /// Serie-número del comprobante afectado. La propiedad se llama `numDocfectado` (sic).
    #[cfg_attr(feature = "serde", serde(rename = "numDocfectado"))]
    pub num_doc_afectado: Option<String>,
    /// Percepción.
    pub perception: Option<SalePerception>,
    /// Valor de venta.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub valor_venta: Option<Decimal>,
    /// Subtotal (precio de venta).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub sub_total: Option<Decimal>,
}

/// Línea de un comprobante de venta.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct SaleDetail {
    /// Unidad de medida (catálogo 03).
    pub unidad: Option<String>,
    /// Cantidad.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub cantidad: Option<Decimal>,
    /// Código interno del producto.
    pub cod_producto: Option<String>,
    /// Código de producto SUNAT.
    pub cod_prod_sunat: Option<String>,
    /// Código de producto GS1.
    #[cfg_attr(feature = "serde", serde(rename = "codProdGS1"))]
    pub cod_prod_gs1: Option<String>,
    /// Descripción.
    pub descripcion: Option<String>,
    /// Valor unitario (sin impuestos).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_valor_unitario: Option<Decimal>,
    /// Cargos de la línea.
    pub cargos: Option<Vec<Charge>>,
    /// Descuentos de la línea.
    pub descuentos: Option<Vec<Charge>>,
    /// Descuento.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub descuento: Option<Decimal>,
    /// Base imponible IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_igv: Option<Decimal>,
    /// Porcentaje de IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje_igv: Option<Decimal>,
    /// IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub igv: Option<Decimal>,
    /// Tipo de afectación del IGV (catálogo 07).
    pub tip_afe_igv: Option<String>,
    /// Base imponible ISC.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_isc: Option<Decimal>,
    /// Porcentaje de ISC.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje_isc: Option<Decimal>,
    /// ISC.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub isc: Option<Decimal>,
    /// Sistema de cálculo del ISC (catálogo 08).
    pub tip_sis_isc: Option<String>,
    /// Base imponible de otros tributos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_oth: Option<Decimal>,
    /// Porcentaje de otros tributos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje_oth: Option<Decimal>,
    /// Otros tributos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub otro_tributo: Option<Decimal>,
    /// ICBPER.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub icbper: Option<Decimal>,
    /// Factor ICBPER; por defecto `0.3`.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub factor_icbper: Option<Decimal>,
    /// Total de impuestos de la línea.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total_impuestos: Option<Decimal>,
    /// Precio unitario (con impuestos).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_precio_unitario: Option<Decimal>,
    /// Valor de venta de la línea.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_valor_venta: Option<Decimal>,
    /// Valor referencial de operaciones gratuitas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_valor_gratuito: Option<Decimal>,
    /// Atributos adicionales.
    pub atributos: Option<Vec<DetailAttribute>>,
}

impl Default for SaleDetail {
    fn default() -> Self {
        Self {
            unidad: None,
            cantidad: None,
            cod_producto: None,
            cod_prod_sunat: None,
            cod_prod_gs1: None,
            descripcion: None,
            mto_valor_unitario: None,
            cargos: None,
            descuentos: None,
            descuento: None,
            mto_base_igv: None,
            porcentaje_igv: None,
            igv: None,
            tip_afe_igv: None,
            mto_base_isc: None,
            porcentaje_isc: None,
            isc: None,
            tip_sis_isc: None,
            mto_base_oth: None,
            porcentaje_oth: None,
            otro_tributo: None,
            icbper: None,
            factor_icbper: Some(Decimal::new(3, 1)),
            total_impuestos: None,
            mto_precio_unitario: None,
            mto_valor_venta: None,
            mto_valor_gratuito: None,
            atributos: None,
        }
    }
}

/// Cargo o descuento.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Charge {
    /// Código de tipo (catálogo 53).
    pub cod_tipo: Option<String>,
    /// Factor.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub factor: Option<Decimal>,
    /// Monto.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub monto: Option<Decimal>,
    /// Monto base.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub monto_base: Option<Decimal>,
}

/// Cuota de pago a crédito.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Cuota {
    /// Moneda.
    pub moneda: Option<String>,
    /// Monto.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub monto: Option<Decimal>,
    /// Fecha de pago.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_pago: Option<Zoned>,
}

/// Atributo adicional de una línea.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct DetailAttribute {
    /// Código.
    pub code: Option<String>,
    /// Nombre.
    pub name: Option<String>,
    /// Valor.
    pub value: Option<String>,
    /// Fecha de inicio. En la referencia el setter exige `DateTime` mutable.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_inicio: Option<Zoned>,
    /// Fecha de fin. En la referencia el setter exige `DateTime` mutable.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fec_fin: Option<Zoned>,
    /// Duración.
    pub duracion: Option<i64>,
}

/// Detracción (SPOT).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Detraction {
    /// Porcentaje.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub percent: Option<Decimal>,
    /// Monto (propiedad `mount`).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mount: Option<Decimal>,
    /// Cuenta del Banco de la Nación.
    pub cta_banco: Option<String>,
    /// Medio de pago (catálogo 59).
    pub cod_medio_pago: Option<String>,
    /// Bien o servicio sujeto a detracción (catálogo 54).
    pub cod_bien_detraccion: Option<String>,
    /// Valor referencial.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub value_ref: Option<Decimal>,
}

/// Referencia a otro documento (guía, documento relacionado o de referencia).
///
/// Se llama `RelatedDocument` para no confundirlo con el trait
/// [`crate::Document`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct RelatedDocument {
    /// Tipo de documento.
    pub tipo_doc: Option<String>,
    /// Número de documento.
    pub nro_doc: Option<String>,
}

/// Guía de remisión embebida en una factura (nombre `EmbededDespatch`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct EmbededDespatch {
    /// Punto de llegada.
    pub llegada: Option<Direction>,
    /// Punto de partida.
    pub partida: Option<Direction>,
    /// Transportista.
    pub transportista: Option<Client>,
    /// Licencia del conductor.
    pub nro_licencia: Option<String>,
    /// Placa del vehículo.
    pub transp_placa: Option<String>,
    /// Código de autorización del vehículo.
    pub transp_code_auth: Option<String>,
    /// Marca del vehículo.
    pub transp_marca: Option<String>,
    /// Modalidad de traslado.
    pub mod_traslado: Option<String>,
    /// Peso bruto.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub peso_bruto: Option<Decimal>,
    /// Unidad del peso bruto.
    pub und_peso_bruto: Option<String>,
}

/// Leyenda (catálogo 52).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Legend {
    /// Código.
    pub code: Option<String>,
    /// Texto.
    pub value: Option<String>,
}

/// Forma de pago. Las clases de la referencia `FormaPagoContado` y `FormaPagoCredito` son los constructores [`PaymentTerms::contado`] y [`PaymentTerms::credito`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct PaymentTerms {
    /// Moneda.
    pub moneda: Option<String>,
    /// Tipo: `Contado`, `Credito` u otro texto.
    pub tipo: Option<String>,
    /// Monto pendiente de pago.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub monto: Option<Decimal>,
}

/// Anticipo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Prepayment {
    /// Tipo del documento del anticipo.
    pub tipo_doc_rel: Option<String>,
    /// Número del documento del anticipo.
    pub nro_doc_rel: Option<String>,
    /// Total del anticipo.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total: Option<Decimal>,
}

/// Recibo por honorarios electrónico.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Receipt {
    /// Emisor (persona natural).
    pub person: Option<Company>,
    /// Receptor.
    pub receptor: Option<Client>,
    /// Serie.
    pub serie: Option<String>,
    /// Correlativo.
    pub correlativo: Option<String>,
    /// Fecha de emisión.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_emision: Option<Zoned>,
    /// Concepto.
    pub concepto: Option<String>,
    /// Monto en letras.
    pub monto_letras: Option<String>,
    /// Subtotal.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub sub_total: Option<Decimal>,
    /// Retención.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub retencion: Option<Decimal>,
    /// Porcentaje de retención.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje: Option<Decimal>,
    /// Total.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total: Option<Decimal>,
}

/// Percepción asociada a una venta.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct SalePerception {
    /// Régimen de percepción.
    pub cod_reg: Option<String>,
    /// Porcentaje.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje: Option<Decimal>,
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

impl BaseSale {
    /// `RUC-TIPO-SERIE-CORRELATIVO`, con `None` como cadena vacía.
    fn document_name(&self, model: &'static str) -> Result<String, CoreError> {
        let company = self
            .company
            .as_ref()
            .ok_or(CoreError::missing(model, "company"))?;
        Ok(join_parts(&[
            company.ruc.as_deref(),
            self.tipo_doc.as_deref(),
            self.serie.as_deref(),
            self.correlativo.as_deref(),
        ]))
    }
}

/// `BaseSale` es una clase concreta que implementa `DocumentInterface` en la referencia.
impl Document for BaseSale {
    fn name(&self) -> Result<String, CoreError> {
        self.document_name("BaseSale")
    }
}

impl Document for Invoice {
    /// `RUC-TIPO-SERIE-CORRELATIVO`, por ejemplo `20000000001-01-F001-123`.
    fn name(&self) -> Result<String, CoreError> {
        self.base.document_name("Invoice")
    }
}

impl Document for Note {
    /// `RUC-TIPO-SERIE-CORRELATIVO`, por ejemplo `20000000001-07-FC01-45`.
    fn name(&self) -> Result<String, CoreError> {
        self.base.document_name("Note")
    }
}

impl Document for Receipt {
    /// `RHE` + RUC + correlativo, sin separadores ni serie.
    fn name(&self) -> Result<String, CoreError> {
        let person = self
            .person
            .as_ref()
            .ok_or(CoreError::missing("Receipt", "person"))?;
        let mut name = String::from("RHE");
        name.push_str(person.ruc.as_deref().unwrap_or_default());
        name.push_str(self.correlativo.as_deref().unwrap_or_default());
        Ok(name)
    }
}

impl PaymentTerms {
    /// Valor de `tipo` para pagos al contado.
    pub const CONTADO: &'static str = "Contado";
    /// Valor de `tipo` para pagos a crédito.
    pub const CREDITO: &'static str = "Credito";

    /// Pago al contado (`new FormaPagoContado()`): sin moneda ni monto.
    #[must_use]
    pub fn contado() -> Self {
        Self {
            moneda: None,
            tipo: Some(Self::CONTADO.to_owned()),
            monto: None,
        }
    }

    /// Pago a crédito (`new FormaPagoCredito($monto, $moneda)`).
    #[must_use]
    pub fn credito(monto: Option<Decimal>, moneda: Option<String>) -> Self {
        Self {
            moneda,
            tipo: Some(Self::CREDITO.to_owned()),
            monto,
        }
    }
}
