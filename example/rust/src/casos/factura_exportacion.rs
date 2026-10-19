//! F001-125, exportación en USD a cliente extranjero.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item = SaleDetail {
        cod_producto: s("P001"),
        cod_prod_sunat: s("44121618"), // Código de producto SUNAT, requerido.
        unidad: s("KG"),
        descripcion: s("PROD 1"),
        cantidad: d(2, 0),
        mto_valor_unitario: d(50, 0),
        mto_valor_venta: d(100, 0),
        mto_base_igv: d(100, 0),
        porcentaje_igv: d(0, 0),
        igv: d(0, 0),
        tip_afe_igv: s("40"),
        total_impuestos: d(0, 0),
        mto_precio_unitario: d(50, 0),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("125"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("USD"),
            company: empresa(),
            // Cliente: extranjero o sin documentos
            client: Some(Client {
                tipo_doc: s("0"),
                num_doc: s("-"),
                rzn_social: s("EXTRANJERO 1"),
                ..Client::default()
            }),
            mto_oper_exportacion: d(100, 0),
            mto_igv: d(0, 0),
            total_impuestos: d(0, 0),
            mto_imp_venta: d(100, 0),
            details: Some(vec![item]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON CIEN CON OO/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0200"), // Tipo de operación: exportación
        valor_venta: d(100, 0),
        sub_total: d(100, 0),
        ..Invoice::default()
    }))
}
