//! B001-2 con bolsa de plástico
//! (ICBPER) y redondeo.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    // Cliente (el demo lo crea, pero la boleta usa el cliente compartido)
    let _client = Client {
        tipo_doc: s("1"),
        num_doc: s("74002155"),
        rzn_social: s("JUAN BRAIN"),
        ..Client::default()
    };
    let detail = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        cantidad: d(2, 0),
        descripcion: s("PRODUCTO 1"),
        mto_base_igv: d(20_000, 2),
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
        cantidad: d(4, 0),
        descripcion: s("BOLSA DE PLASTICO"),
        mto_valor_unitario: d(5, 2),
        mto_precio_unitario: d(59, 3),
        mto_valor_venta: d(20, 2),
        tip_afe_igv: s("10"),
        mto_base_igv: d(20, 2),
        porcentaje_igv: d(180, 1),
        igv: d(4, 2),
        icbper: d(80, 2), // (cantidad)*(factor ICBPER)
        factor_icbper: d(20, 2),
        total_impuestos: d(84, 2),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("03"),
            serie: s("B001"),
            correlativo: s("2"),
            company: empresa(),
            fecha_emision: c.fecha(),
            tipo_moneda: s("PEN"),
            client: cliente(),
            mto_oper_gravadas: d(20_020, 2),
            mto_igv: d(3604, 2),
            icbper: d(80, 2),
            total_impuestos: d(3684, 2),
            redondeo: d(4, 2),
            mto_imp_venta: d(237, 0),
            details: Some(vec![detail, detail_bolsa]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON DOSCIENTOS TREINTA Y SIETE CON 00/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        valor_venta: d(20_020, 2),
        sub_total: d(23_704, 2),
        ..Invoice::default()
    }))
}
