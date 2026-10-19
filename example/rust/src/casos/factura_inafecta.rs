//! F001-137, operación inafecta.

use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("KG"),
        descripcion: s("PROD 1"),
        cantidad: d(2, 0),
        mto_valor_unitario: d(50, 0),
        mto_valor_venta: d(100, 0),
        mto_base_igv: d(100, 0),
        porcentaje_igv: d(0, 0),
        igv: d(0, 0),
        tip_afe_igv: s("30"),
        total_impuestos: d(0, 0),
        mto_precio_unitario: d(50, 0),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("137"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_inafectas: d(100, 0),
            mto_igv: d(0, 0),
            total_impuestos: d(0, 0),
            mto_imp_venta: d(100, 0),
            details: Some(vec![item]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON CIEN CON OO/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        valor_venta: d(100, 0),
        sub_total: d(100, 0),
        ..Invoice::default()
    }))
}
