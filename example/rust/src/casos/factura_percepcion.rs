//! F001-167, venta sujeta
//! a percepción.

use cpeforge_core::model::sale::{
    BaseSale, Invoice, Legend, PaymentTerms, SaleDetail, SalePerception,
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
    // Venta
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("167"),
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
            legends: Some(vec![
                Legend {
                    code: s("1000"),
                    value: s("SON DOSCIENTOS TREINTA Y SEIS CON OO/100 SOLES"),
                },
                Legend {
                    code: s("2000"),
                    value: s("COMPROBANTE DE PERCEPCIÓN"),
                },
            ]),
            ..BaseSale::default()
        },
        tipo_operacion: s("2001"), // Percepciones
        valor_venta: d(200, 0),
        sub_total: d(236, 0),
        perception: Some(SalePerception {
            cod_reg: s("51"),
            porcentaje: d(2, 2),
            mto_base: d(200, 0),
            mto: d(400, 2),
            mto_total: d(20_400, 2),
        }),
        ..Invoice::default()
    }))
}
