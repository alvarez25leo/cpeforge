//! R001-123.

use cpeforge_core::model::retention::{Exchange, Payment, Retention, RetentionDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let pay = Payment {
        moneda: s("PEN"),
        fecha: c.fecha(),
        importe: d(100, 0),
    };
    let cambio = Exchange {
        fecha: c.fecha(),
        factor: d(1, 0),
        moneda_obj: s("PEN"),
        moneda_ref: s("PEN"),
    };
    let detail = RetentionDetail {
        tipo_doc: s("01"),
        num_doc: s("F001-1"),
        fecha_emision: c.fecha(),
        fecha_retencion: c.fecha(),
        moneda: s("PEN"),
        imp_total: d(210, 0),
        imp_pagar: d(200, 0),
        imp_retenido: d(10, 0),
        pagos: Some(vec![pay]),
        tipo_cambio: Some(cambio),
    };
    Ok(Documento::Retention(Retention {
        serie: s("R001"),
        correlativo: s("123"),
        fecha_emision: c.fecha(),
        company: empresa(),
        proveedor: cliente(),
        observacion: s("NOTA /><!-- HI -->"),
        imp_retenido: d(10, 0),
        imp_pagado: d(200, 0),
        regimen: s("01"),
        tasa: d(3, 0),
        details: Some(vec![detail]),
    }))
}
