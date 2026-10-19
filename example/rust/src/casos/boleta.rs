//! B001-1 a persona con DNI.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    // Cliente
    let client = Client {
        tipo_doc: s("1"),
        num_doc: s("20203030"),
        rzn_social: s("PERSON 1"),
        ..Client::default()
    };
    let item1 = SaleDetail {
        cod_producto: s("C023"),
        unidad: s("NIU"),
        cantidad: d(2, 0),
        descripcion: s("PROD 1"),
        mto_base_igv: d(100, 0),
        porcentaje_igv: d(18, 0),
        igv: d(18, 0),
        tip_afe_igv: s("10"),
        total_impuestos: d(18, 0),
        mto_valor_venta: d(100, 0),
        mto_valor_unitario: d(50, 0),
        mto_precio_unitario: d(59, 0),
        ..SaleDetail::default()
    };
    // Venta
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("03"),
            serie: s("B001"),
            correlativo: s("1"),
            fecha_emision: c.fecha(),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: Some(client),
            mto_oper_gravadas: d(100, 0),
            mto_igv: d(18, 0),
            total_impuestos: d(18, 0),
            mto_imp_venta: d(118, 0),
            details: Some(vec![item1]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON CIENTO DIECIOCHO CON 00/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        valor_venta: d(100, 0),
        sub_total: d(118, 0),
        ..Invoice::default()
    }))
}
