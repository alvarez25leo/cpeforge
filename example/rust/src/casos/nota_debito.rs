//! FF01-123 por aumento en el
//! valor.

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
        mto_precio_unitario: d(59, 0),
        ..SaleDetail::default()
    }
}

pub fn documento(c: &Contexto) -> Resultado {
    Ok(Documento::Note(Note {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("08"),
            serie: s("FF01"),
            fecha_emision: c.fecha(),
            correlativo: s("123"),
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
        tip_doc_afectado: s("01"),
        num_doc_afectado: s("F001-111"),
        cod_motivo: s("02"),
        des_motivo: s("AUMENTO EN EL VALOR"),
        ..Note::default()
    }))
}
