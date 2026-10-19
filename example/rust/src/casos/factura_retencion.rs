//! F001-168 con retención
//! del 3 % (descuento código 62).

use cpeforge_core::model::sale::{BaseSale, Charge, Invoice, Legend, PaymentTerms, SaleDetail};

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
    // Venta
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("168"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            client: cliente(),
            company: empresa(),
            mto_oper_gravadas: d(200, 0),
            mto_igv: d(36, 0),
            total_impuestos: d(36, 0),
            mto_imp_venta: d(236, 0),
            details: Some(vec![detail]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON DOSCIENTOS TREINTA Y SEIS CON OO/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        descuentos: Some(vec![Charge {
            cod_tipo: s("62"), // Catalog. 53
            monto_base: d(236, 0),
            factor: d(3, 2), // 3%
            monto: d(708, 2),
        }]),
        valor_venta: d(200, 0),
        sub_total: d(236, 0),
        ..Invoice::default()
    }))
}
