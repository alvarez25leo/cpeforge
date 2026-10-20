//! Liquidación de compra electrónica (tipo `04`).
//!
//! La emite el comprador cuando adquiere productos primarios (agropecuarios,
//! pesca artesanal, madera, minería aurífera artesanal, desechos metálicos,
//! entre otros) a una persona natural sin RUC. Su XML es un UBL 2.1
//! `SelfBilledInvoice` (anexo N.º 9-A, literal e), de la R. S.
//! 123-2022/SUNAT):
//!
//! - el emisor, que es el **comprador**, va en `AccountingCustomerParty`;
//! - el **vendedor** va en `AccountingSupplierParty`, con su documento de
//!   identidad (DNI, carné de extranjería o pasaporte) y la condición de su
//!   domicilio (catálogo 60);
//! - el lugar de la operación va en `DeliveryTerms`;
//! - cada línea lleva el IGV y la retención del impuesto a la renta
//!   (tributo `3000`), que el comprador retiene;
//! - el importe neto a pagar es el subtotal menos el IGV y la renta
//!   retenidos.
//!
//! Los importes los calcula quien arma el modelo, como en el resto de
//! comprobantes; el XML los imprime tal cual.

use crate::document::join_parts;
use crate::model::company::{Address, Company};
use crate::model::sale::{DetailAttribute, Legend, RelatedDocument};
use crate::{CoreError, Decimal, Document, Zoned};

/// Tipo de documento de la liquidación de compra (catálogo 01).
pub const SETTLEMENT_TYPE: &str = "04";

/// Liquidación de compra electrónica.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct PurchaseSettlement {
    /// Tipo de operación (catálogo 51): `0501` compra interna, `0502`
    /// anticipos, `0503` compra de oro.
    pub tipo_operacion: Option<String>,
    /// Serie: `L` y tres letras o dígitos (`L001`).
    pub serie: Option<String>,
    /// Correlativo, de 1 a 8 dígitos.
    pub correlativo: Option<String>,
    /// Fecha y hora de emisión.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub fecha_emision: Option<Zoned>,
    /// Moneda (catálogo 02).
    pub tipo_moneda: Option<String>,
    /// Emisor electrónico (comprador).
    pub company: Option<Company>,
    /// Vendedor sin RUC.
    pub vendedor: Option<SettlementSeller>,
    /// Lugar donde se realiza la operación.
    pub lugar_operacion: Option<SettlementPlace>,
    /// Guías de remisión relacionadas (`09` o `31`).
    pub guias: Option<Vec<RelatedDocument>>,
    /// Otros documentos relacionados (catálogo 12).
    pub rel_docs: Option<Vec<RelatedDocument>>,
    /// Total valor de venta de operaciones gravadas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_gravadas: Option<Decimal>,
    /// Total valor de venta de operaciones exoneradas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_exoneradas: Option<Decimal>,
    /// Total valor de venta de operaciones inafectas.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_oper_inafectas: Option<Decimal>,
    /// Sumatoria de IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_igv: Option<Decimal>,
    /// Base de la retención del impuesto a la renta.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_ir: Option<Decimal>,
    /// Sumatoria de la retención del impuesto a la renta.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_ir: Option<Decimal>,
    /// Monto total de tributos: el IGV (la renta retenida no se suma).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total_impuestos: Option<Decimal>,
    /// Total valor de venta (gravadas, exoneradas e inafectas).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub valor_venta: Option<Decimal>,
    /// Subtotal: valor de venta más tributos.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub sub_total: Option<Decimal>,
    /// Redondeo del importe total, de existir.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub redondeo: Option<Decimal>,
    /// Importe total neto: subtotal − IGV − renta retenida + redondeo.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_imp_venta: Option<Decimal>,
    /// Leyendas (catálogo 52), como el importe en letras (`1000`).
    pub legends: Option<Vec<Legend>>,
    /// Líneas, en orden.
    pub details: Option<Vec<SettlementDetail>>,
}

/// Vendedor de una liquidación de compra.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct SettlementSeller {
    /// Tipo de documento de identidad (catálogo 06): `1`, `4` o `7`.
    pub tipo_doc: Option<String>,
    /// Número de documento.
    pub num_doc: Option<String>,
    /// Apellidos y nombres.
    pub rzn_social: Option<String>,
    /// Domicilio; su `cod_local` no se usa.
    pub address: Option<Address>,
    /// Condición del domicilio (catálogo 60): `01` punto de venta, `02`
    /// producción, `03` extracción, `04` explotación, `05` otros.
    pub condicion: Option<String>,
}

/// Lugar donde se realiza la operación.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct SettlementPlace {
    /// Dirección; su `cod_local` no se usa.
    pub address: Option<Address>,
    /// Condición del lugar (catálogo 60).
    pub condicion: Option<String>,
}

/// Línea de una liquidación de compra.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct SettlementDetail {
    /// Unidad de medida (catálogo 03).
    pub unidad: Option<String>,
    /// Cantidad.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub cantidad: Option<Decimal>,
    /// Código interno del producto.
    pub cod_producto: Option<String>,
    /// Código de producto SUNAT (catálogo 25, UNSPSC de 8 dígitos).
    pub cod_prod_sunat: Option<String>,
    /// Descripción del producto comprado.
    pub descripcion: Option<String>,
    /// Valor unitario (sin IGV).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_valor_unitario: Option<Decimal>,
    /// Precio unitario (con IGV).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_precio_unitario: Option<Decimal>,
    /// Valor de venta: valor unitario × cantidad.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_valor_venta: Option<Decimal>,
    /// Tipo de afectación del IGV (catálogo 07): `10`, `20` o `30`.
    pub tip_afe_igv: Option<String>,
    /// Base del IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_igv: Option<Decimal>,
    /// Tasa del IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje_igv: Option<Decimal>,
    /// IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub igv: Option<Decimal>,
    /// Base de la retención del impuesto a la renta.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub mto_base_ir: Option<Decimal>,
    /// Tasa de la retención: `1.5`, o `4` en la compra de oro.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub porcentaje_ir: Option<Decimal>,
    /// Renta retenida.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub ir: Option<Decimal>,
    /// Tributos de la línea: el IGV.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::decimal_option"))]
    pub total_impuestos: Option<Decimal>,
    /// Propiedades del ítem (catálogo 55), como las de la compra de oro
    /// (`6000`, `6004`, `6005` y `6006`).
    pub atributos: Option<Vec<DetailAttribute>>,
}

impl Document for PurchaseSettlement {
    /// `RUC-04-SERIE-CORRELATIVO`, por ejemplo `20000000001-04-L001-1`.
    fn name(&self) -> Result<String, CoreError> {
        let company = self
            .company
            .as_ref()
            .ok_or(CoreError::missing("PurchaseSettlement", "company"))?;
        Ok(join_parts(&[
            company.ruc.as_deref(),
            Some(SETTLEMENT_TYPE),
            self.serie.as_deref(),
            self.correlativo.as_deref(),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::{PurchaseSettlement, SETTLEMENT_TYPE};
    use crate::model::company::Company;
    use crate::{CoreError, Document};

    #[test]
    fn name_uses_the_buyer_ruc_and_type_04() {
        let doc = PurchaseSettlement {
            serie: Some("L001".into()),
            correlativo: Some("7".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                ..Company::default()
            }),
            ..PurchaseSettlement::default()
        };
        assert_eq!(SETTLEMENT_TYPE, "04");
        assert_eq!(doc.name().unwrap_or_default(), "20000000001-04-L001-7");
    }

    #[test]
    fn name_without_company_is_an_error() {
        let err = PurchaseSettlement::default().name().unwrap_err();
        assert_eq!(
            err,
            CoreError::MissingField {
                model: "PurchaseSettlement",
                field: "company"
            }
        );
    }
}
