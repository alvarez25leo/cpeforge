//! F001-143 con deducción
//! de un anticipo (F001-111).

use cpeforge_core::model::sale::{
    BaseSale, Charge, Invoice, Legend, PaymentTerms, Prepayment, SaleDetail,
};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let detail = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        descripcion: s("PROD 1"),
        cantidad: d(2, 0),
        mto_valor_unitario: d(100, 0),
        mto_valor_venta: d(200, 0),
        mto_base_igv: d(200, 0),
        porcentaje_igv: d(18, 0),
        igv: d(36, 0),
        tip_afe_igv: s("10"),
        total_impuestos: d(36, 0),
        mto_precio_unitario: d(118, 0),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("143"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            client: cliente(),
            company: empresa(),
            // Sumatoria de Valor Venta (detalles) menos descuentos globales (anticipo): 200 - 100
            mto_oper_gravadas: d(100, 0),
            mto_igv: d(18, 0),
            total_impuestos: d(18, 0),
            mto_imp_venta: d(136, 0), // subTotal - Anticipos: 236 - 100
            details: Some(vec![detail]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON DOSCIENTOS TREINTA Y SEIS CON OO/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        descuentos: Some(vec![Charge {
            cod_tipo: s("04"),
            factor: d(1, 0),
            monto: d(100, 0), // anticipo
            monto_base: d(100, 0),
        }]),
        valor_venta: d(200, 0), // sumatoria de valor venta (detalle)
        sub_total: d(236, 0),   // ValorVenta + (sumatoria de valor venta detalle) * 18% (IGV)
        anticipos: Some(vec![Prepayment {
            tipo_doc_rel: s("02"), // catalog. 12
            nro_doc_rel: s("F001-111"),
            total: d(100, 0),
        }]),
        total_anticipos: d(100, 0),
        ..Invoice::default()
    }))
}
