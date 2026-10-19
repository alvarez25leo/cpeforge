//! F001-227, gravada con ISC (sistema al valor).

use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    // Detalle con ISC
    let item = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        descripcion: s("PROD 1"),
        cantidad: d(2, 0),
        mto_valor_unitario: d(100, 0),
        mto_valor_venta: d(200, 0),
        mto_base_isc: d(200, 0),
        tip_sis_isc: s("01"),       // Catalog 08: Sistema al Valor
        porcentaje_isc: d(1700, 2), // 17%
        isc: d(34, 0),              // 200 * 0.17 (17%)
        mto_base_igv: d(234, 0),    // ValorVenta + ISC
        porcentaje_igv: d(18, 0),
        igv: d(4212, 2),      // MtoBaseIGV * 18%
        tip_afe_igv: s("10"), // Catalog: 07
        total_impuestos: d(7612, 2),
        mto_precio_unitario: d(13806, 2), // (ValorVenta + TotalImpuestos) / Cantidad
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            // El demo usa "127", que choca con factura-detraccion; aquí se usa "227".
            correlativo: s("227"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(200, 0),
            mto_igv: d(4212, 2),
            mto_base_isc: d(200, 0), // Sumatoria MtoBaseISC de los detalles
            mto_isc: d(34, 0),
            total_impuestos: d(7612, 2),
            mto_imp_venta: d(27612, 2),
            details: Some(vec![item]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON DOSCIENTOS SETENTA Y SEIS CON 12/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        fec_vencimiento: c.fecha(),
        tipo_operacion: s("0101"),
        valor_venta: d(200, 0),
        sub_total: d(27612, 2),
        ..Invoice::default()
    }))
}
