//! F001-226, venta sujeta al IVAP.

use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item = SaleDetail {
        cod_producto: s("A001"),
        unidad: s("SA"), // Codigo unidad de saco
        descripcion: s("SACOS DE ARROZ"),
        cantidad: d(90_000, 2),
        mto_valor_unitario: d(10_000, 2),
        mto_valor_venta: d(9_000_000, 2),
        mto_base_igv: d(9_000_000, 2),
        porcentaje_igv: d(4, 0), // Tasa IVAP
        igv: d(360_000, 2),
        tip_afe_igv: s("17"), // Tipo Afectacion IVAP
        total_impuestos: d(360_000, 2),
        mto_precio_unitario: d(10_400, 2),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("226"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_base_ivap: d(9_000_000, 2), // Base IVAP
            mto_ivap: d(360_000, 2),        // Suma IVAP
            total_impuestos: d(360_000, 2),
            mto_imp_venta: d(9_360_000, 2),
            details: Some(vec![item]),
            legends: Some(vec![
                Legend {
                    code: s("1000"),
                    value: s("SON TRESCIENTOS TREINTA Y SEIS CON OO/100 SOLES"),
                },
                Legend {
                    code: s("2007"), // Leyenda IVAP
                    value: s("Operación sujeta al IVAP"),
                },
            ]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        valor_venta: d(9_000_000, 2),
        sub_total: d(9_360_000, 2),
        ..Invoice::default()
    }))
}
