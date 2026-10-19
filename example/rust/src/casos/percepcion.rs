//! P001-123.

use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use cpeforge_core::model::retention::{Exchange, Payment};

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
    let detail = PerceptionDetail {
        tipo_doc: s("01"),
        num_doc: s("F001-1"),
        fecha_emision: c.fecha(),
        fecha_percepcion: c.fecha(),
        moneda: s("PEN"),
        imp_total: d(200, 0),
        imp_cobrar: d(210, 0),
        imp_percibido: d(10, 0),
        cobros: Some(vec![pay]),
        tipo_cambio: Some(cambio),
    };
    Ok(Documento::Perception(Perception {
        serie: s("P001"),
        correlativo: s("123"),
        fecha_emision: c.fecha(),
        observacion: s("NOTA PRUEBA />"),
        company: empresa(),
        proveedor: cliente(),
        imp_percibido: d(10, 0),
        imp_cobrado: d(210, 0),
        regimen: s("01"),
        tasa: d(2, 0),
        details: Some(vec![detail]),
    }))
}
