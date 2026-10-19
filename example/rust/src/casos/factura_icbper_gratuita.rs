//! F001-130, gravada con bolsa plástica gratuita (ICBPER).

use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let detail = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        cantidad: d(2, 0),
        descripcion: s("PRODUCTO 1"),
        mto_base_igv: d(20000, 2),
        porcentaje_igv: d(180, 1),
        igv: d(36, 0),
        total_impuestos: d(36, 0),
        tip_afe_igv: s("10"),
        mto_valor_venta: d(200, 0),
        mto_valor_unitario: d(100, 0),
        mto_precio_unitario: d(118, 0),
        ..SaleDetail::default()
    };

    let detail_bolsa = SaleDetail {
        cod_producto: s("P002"),
        unidad: s("NIU"),
        cantidad: d(2, 0),
        descripcion: s("BOLSA DE PLASTICO"),
        mto_valor_unitario: d(0, 2),
        mto_valor_gratuito: d(5, 2),
        mto_valor_venta: d(10, 2),
        tip_afe_igv: s("13"), // catalog: 07, código de afectación gratuito
        mto_base_igv: d(10, 2),
        porcentaje_igv: d(180, 1),
        igv: d(2, 2),
        factor_icbper: d(20, 2), // Factor ICBPER del año actual
        icbper: d(40, 2),        // (cantidad)*(factor ICBPER)
        total_impuestos: d(42, 2),
        ..SaleDetail::default()
    };

    let legend = Legend {
        code: s("1000"),
        value: s("SON DOSCIENTOS TREINTA Y SIETE CON 00/100 SOLES"),
    };

    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("130"),
            company: empresa(),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            client: cliente(),
            mto_oper_gravadas: d(200, 0),
            mto_oper_gratuitas: d(10, 2),
            mto_igv_gratuitas: d(2, 2),
            mto_igv: d(36, 0),
            icbper: d(40, 2),
            total_impuestos: d(3640, 2),
            mto_imp_venta: d(23640, 2),
            details: Some(vec![detail, detail_bolsa]),
            legends: Some(vec![legend]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        valor_venta: d(200, 0),
        sub_total: d(23640, 2),
        ..Invoice::default()
    }))
}
