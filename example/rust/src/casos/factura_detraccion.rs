//! F001-127, operación sujeta a detracción.

use cpeforge_core::model::sale::{BaseSale, Detraction, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let detail = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("ZZ"),
        descripcion: s("PROD 1"),
        cantidad: d(4, 0),
        mto_valor_unitario: d(200, 0),
        mto_valor_venta: d(800, 0),
        mto_base_igv: d(800, 0),
        porcentaje_igv: d(18, 0),
        igv: d(144, 0),
        tip_afe_igv: s("10"), // Catálogo 07 - Gravado
        total_impuestos: d(144, 0),
        mto_precio_unitario: d(236, 0),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("127"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(800, 0),
            mto_igv: d(144, 0),
            total_impuestos: d(144, 0),
            mto_imp_venta: d(944, 0),
            details: Some(vec![detail]),
            legends: Some(vec![
                Legend {
                    code: s("1000"),
                    value: s("SON NOVECIENTOS CUARENTA Y CUATRO CON OO/100 SOLES"),
                },
                Legend {
                    code: s("2006"),
                    value: s("Operación sujeta a detracción"),
                },
            ]),
            ..BaseSale::default()
        },
        tipo_operacion: s("1001"), // Catálogo 51: Detracción
        valor_venta: d(800, 0),
        sub_total: d(944, 0),
        // MONEDA SIEMPRE EN SOLES
        detraccion: Some(Detraction {
            // Carnes y despojos comestibles
            cod_bien_detraccion: s("014"), // catalog. 54
            // Depósito en cuenta
            cod_medio_pago: s("001"), // catalog. 59
            cta_banco: s("0004-3342343243"),
            percent: d(400, 2),
            mount: d(3776, 2),
            ..Detraction::default()
        }),
        ..Invoice::default()
    }))
}
