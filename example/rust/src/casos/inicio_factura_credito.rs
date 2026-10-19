//! F001-3: factura a crédito en dos cuotas con líneas gravada, inafecta,
//! gratuita y bolsa plástica (ICBPER).
//!
//! Gravadas 102.00 (100.00 + bolsa 2.00), IGV 18.36, ICBPER 2.00,
//! inafectas 50.00, gratuitas 20.00 (su IGV 3.60 no se cobra).
//! Total = 102.00 + 50.00 + 18.36 + 2.00 = 172.36.

use cpeforge_core::Decimal;
use cpeforge_core::model::sale::{
    BaseSale, Cuota, Invoice, Legend, PaymentTerms, RelatedDocument, SaleDetail,
};

use super::{Documento, Resultado};
use crate::datos::{
    Contexto, d, inicio_base, inicio_cliente_empresa, inicio_linea_bolsa, inicio_linea_gravada, s,
};

pub fn documento(c: &Contexto) -> Resultado {
    let inafecta = SaleDetail {
        cod_producto: s("P003"),
        cod_prod_sunat: s("10191509"), // Catálogo 25 (UNSPSC)
        unidad: s("NIU"),
        cantidad: d(2, 0),
        descripcion: s("LIBRO IMPRESO"),
        mto_valor_unitario: d(2500, 2),
        mto_valor_venta: d(5000, 2),
        mto_base_igv: d(5000, 2),
        porcentaje_igv: d(0, 0),
        igv: d(0, 0),
        tip_afe_igv: s("30"), // Inafecto - Operación onerosa
        total_impuestos: d(0, 0),
        mto_precio_unitario: d(2500, 2),
        ..SaleDetail::default()
    };
    let gratuita = SaleDetail {
        cod_producto: s("P004"),
        unidad: s("NIU"),
        cantidad: d(1, 0),
        descripcion: s("MUESTRA GRATIS"),
        mto_valor_unitario: d(0, 0),
        mto_valor_gratuito: d(2000, 2),
        mto_valor_venta: d(2000, 2),
        mto_base_igv: d(2000, 2),
        porcentaje_igv: d(18, 0),
        igv: d(360, 2),
        tip_afe_igv: s("11"), // Gravado - Retiro por premio (gratuito)
        total_impuestos: d(360, 2),
        mto_precio_unitario: d(0, 0),
        ..SaleDetail::default()
    };
    let cuota = |dias: i64| -> Result<Cuota, jiff::Error> {
        Ok(Cuota {
            monto: d(8618, 2),
            fecha_pago: c.medianoche(dias)?,
            ..Cuota::default()
        })
    };

    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            forma_pago: Some(PaymentTerms::credito(d(17236, 2), None)),
            cuotas: Some(vec![cuota(30)?, cuota(60)?]),
            compra: s("OC-2024-0015"),
            guias: Some(vec![RelatedDocument {
                tipo_doc: s("09"),
                nro_doc: s("T001-00000015"),
            }]),
            mto_oper_gravadas: d(10200, 2),
            mto_oper_inafectas: d(5000, 2),
            mto_oper_gratuitas: d(2000, 2),
            mto_igv_gratuitas: d(360, 2),
            mto_igv: d(1836, 2),
            icbper: d(200, 2),
            total_impuestos: d(2036, 2),
            mto_imp_venta: d(17236, 2),
            details: Some(vec![
                inicio_linea_gravada(
                    "P001",
                    "PRODUCTO 1",
                    Decimal::new(2, 0),
                    Decimal::new(5000, 2),
                    Decimal::new(10000, 2),
                    Decimal::new(1800, 2),
                    Decimal::new(5900, 2),
                ),
                inafecta,
                gratuita,
                inicio_linea_bolsa(
                    Decimal::new(4, 0),
                    Decimal::new(200, 2),
                    Decimal::new(36, 2),
                    Decimal::new(200, 2),
                ),
            ]),
            legends: Some(vec![
                Legend {
                    code: s("1000"),
                    value: s("SON CIENTO SETENTA Y DOS CON 36/100 SOLES"),
                },
                Legend {
                    code: s("1002"),
                    value: s(
                        "TRANSFERENCIA GRATUITA DE UN BIEN Y/O SERVICIO PRESTADO GRATUITAMENTE",
                    ),
                },
            ]),
            ..inicio_base("01", "F001", "3", c, inicio_cliente_empresa())
        },
        tipo_operacion: s("0101"),
        observacion: s("FACTURA DE EJEMPLO A CREDITO"),
        valor_venta: d(15200, 2),
        sub_total: d(17236, 2),
        ..Invoice::default()
    }))
}
