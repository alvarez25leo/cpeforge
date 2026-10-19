//! F001-84 con
//! descuento global que afecta la base imponible.

use cpeforge_core::model::sale::{BaseSale, Charge, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item1 = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        descripcion: s("Cuadernos"),
        cantidad: d(10, 0),
        mto_valor_unitario: d(2, 0),
        mto_valor_venta: d(20, 0),
        mto_base_igv: d(20, 0),
        porcentaje_igv: d(18, 0),
        igv: d(36, 1),
        tip_afe_igv: s("10"),
        total_impuestos: d(36, 1),
        mto_precio_unitario: d(236, 2),
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
    // Factura con descuento global (que afecta la base imponible).
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("84"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(67, 0), // suma de v. venta (items) - descuento global.
            mto_igv: d(1206, 2),
            total_impuestos: d(1206, 2),
            mto_imp_venta: d(7906, 2),
            details: Some(vec![item1, item2]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON SETENTA Y NUEVE CON O6/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        fec_vencimiento: c.fecha(),
        tipo_operacion: s("0101"),
        descuentos: Some(vec![Charge {
            cod_tipo: s("02"), // Catalog. 53
            monto_base: d(3, 0),
            factor: d(1, 0),
            monto: d(3, 0),
        }]),
        valor_venta: d(67, 0),
        sub_total: d(7906, 2),
        ..Invoice::default()
    }))
}
