//! FD01-1 con
//! descuentos (línea y global) que no afectan la base imponible.

use cpeforge_core::model::sale::{BaseSale, Charge, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item = SaleDetail {
        cod_producto: s("P004"),
        unidad: s("NIU"),
        descripcion: s("PRODUCTO 4"),
        cantidad: d(10, 0),
        descuentos: Some(vec![Charge {
            cod_tipo: s("01"),    // Catalog. 53 (00: Descuento que no afecta la Base Imponible)
            monto_base: d(20, 0), // cantidad * valor unitario
            factor: d(2, 1),      // 20% descuento
            monto: d(4, 0),
        }]),
        mto_valor_unitario: d(2, 0),
        mto_valor_venta: d(20, 0), // cantidad * valor unitario
        mto_base_igv: d(20, 0),
        porcentaje_igv: d(18, 0), // 18%
        igv: d(36, 1),
        tip_afe_igv: s("10"),
        total_impuestos: d(36, 1), // IGV + ISC + OTH + ICBPER
        // (Valor venta + Total Impuestos - Descuentos que no afectan la base) / Cantidad
        mto_precio_unitario: d(196, 2),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("FD01"),
            correlativo: s("1"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(20, 0),
            mto_igv: d(36, 1),
            total_impuestos: d(36, 1),
            mto_imp_venta: d(166, 1), // SubTotal - SumOtrosDescuentos: 23.6 - 7.0 = 16.60
            details: Some(vec![item]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON SETENTA Y SIETE CON 88/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"), // Cat.51
        descuentos: Some(vec![Charge {
            cod_tipo: s("03"), // Catalog. 53 (03: Descuento global que no afecta la Base Imponible)
            monto_base: d(3, 0),
            factor: d(1, 0),
            monto: d(3, 0), // Mto Dscto
        }]),
        // suma descuentos que no afectan la base (dscto. detalles + dscto. global): 4.0 + 3.0
        sum_otros_descuentos: d(70, 1),
        valor_venta: d(20, 0),
        sub_total: d(236, 1),
        ..Invoice::default()
    }))
}
