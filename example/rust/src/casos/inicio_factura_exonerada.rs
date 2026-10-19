//! F001-2: factura exonerada.

use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, inicio_base, inicio_cliente_empresa, s};

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
        igv: d(0, 0),
        tip_afe_igv: s("20"), // Exonerado - Operación onerosa
        total_impuestos: d(0, 0),
        mto_precio_unitario: d(100, 0),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            forma_pago: Some(PaymentTerms::contado()),
            mto_oper_exoneradas: d(200, 0),
            mto_igv: d(0, 0),
            total_impuestos: d(0, 0),
            mto_imp_venta: d(200, 0),
            details: Some(vec![detail]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON DOSCIENTOS CON 00/100 SOLES"),
            }]),
            ..inicio_base("01", "F001", "2", c, inicio_cliente_empresa())
        },
        tipo_operacion: s("0101"),
        fec_vencimiento: c.fecha(),
        valor_venta: d(200, 0),
        sub_total: d(200, 0),
        ..Invoice::default()
    }))
}
