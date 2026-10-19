//! FE01-1 con
//! descuento por línea que afecta la base imponible.

use cpeforge_core::model::sale::{BaseSale, Charge, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item_descuento = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        descripcion: s("Cuadernos"),
        cantidad: d(10, 0),
        descuentos: Some(vec![Charge {
            cod_tipo: s("00"),    // Catalog. 53 (00: Descuento que afecta la Base Imponible)
            monto_base: d(20, 0), // cantidad * valor unitario
            factor: d(2, 1),      // 20% descuento
            monto: d(4, 0),
        }]),
        mto_valor_unitario: d(2, 0),
        mto_valor_venta: d(16, 0), // cantidad * valor unitario - descuento (que afecta la base)
        mto_base_igv: d(16, 0),
        porcentaje_igv: d(18, 0),
        igv: d(288, 2),
        tip_afe_igv: s("10"),
        total_impuestos: d(288, 2),
        mto_precio_unitario: d(189, 2), // (Valor venta + Total Impuestos) / Cantidad
        ..SaleDetail::default()
    };
    let item2 = SaleDetail {
        cod_producto: s("P002"),
        unidad: s("NIU"),
        descripcion: s("Radio"),
        cantidad: d(1, 0),
        mto_valor_unitario: d(50, 0),
        mto_valor_venta: d(50, 0),
        mto_base_igv: d(50, 0),
        porcentaje_igv: d(18, 0),
        igv: d(9, 0),
        tip_afe_igv: s("10"),
        total_impuestos: d(9, 0),
        mto_precio_unitario: d(59, 0),
        ..SaleDetail::default()
    };
    // Factura con descuento por linea (que afecta la base imponible).
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("FE01"),
            correlativo: s("1"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(66, 0), // suma de v. venta (items).
            mto_igv: d(1188, 2),
            total_impuestos: d(1188, 2),
            mto_imp_venta: d(7788, 2),
            details: Some(vec![item_descuento, item2]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON SETENTA Y SIETE CON 88/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"), // Cat.51
        valor_venta: d(66, 0),
        sub_total: d(7788, 2),
        ..Invoice::default()
    }))
}
