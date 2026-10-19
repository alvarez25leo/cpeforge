//! BB01-123, anula la
//! boleta B001-12.

use cpeforge_core::model::sale::{BaseSale, Legend, Note, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

fn detalle(codigo: &str, descripcion: &str) -> SaleDetail {
    SaleDetail {
        cod_producto: s(codigo),
        unidad: s("NIU"),
        cantidad: d(2, 0),
        descripcion: s(descripcion),
        mto_base_igv: d(100, 0),
        porcentaje_igv: d(1800, 2),
        igv: d(18, 0),
        tip_afe_igv: s("10"),
        total_impuestos: d(18, 0),
        mto_valor_venta: d(100, 0),
        mto_valor_unitario: d(50, 0),
        mto_precio_unitario: d(56, 0),
        ..SaleDetail::default()
    }
}

pub fn documento(c: &Contexto) -> Resultado {
    Ok(Documento::Note(Note {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("07"),     // Tipo Doc: Nota de Credito
            serie: s("BB01"),      // Serie NCR
            correlativo: s("123"), // Correlativo NCR
            fecha_emision: c.fecha(),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(200, 0),
            mto_igv: d(36, 0),
            total_impuestos: d(36, 0),
            mto_imp_venta: d(236, 0),
            details: Some(vec![detalle("C023", "PROD 1"), detalle("C02", "PROD 2")]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON DOSCIENTOS TREINTA Y SEIS CON 00/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tip_doc_afectado: s("03"),      // Tipo Doc: Boleta
        num_doc_afectado: s("B001-12"), // Boleta: Serie-Correlativo
        cod_motivo: s("01"),            // Catalogo. 09
        des_motivo: s("ANULACION DE LA OPERACION"),
        ..Note::default()
    }))
}
