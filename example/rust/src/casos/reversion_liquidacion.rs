//! RR de la liquidación de compra L001-1: la liquidación se revierte con el
//! resumen de reversiones, en el servicio de retenciones y percepciones.
//! Solo existe en CPEForge.

use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, inicio_emisor, s};

pub fn documento(c: &Contexto) -> Resultado {
    Ok(Documento::Reversion(Reversion {
        voided: Voided {
            correlativo: s("002"),
            // Mismo día de la liquidación revertida.
            fec_generacion: c.fecha(),
            fec_comunicacion: c.fecha(),
            company: inicio_emisor(&c.ruc),
            details: Some(vec![VoidedDetail {
                tipo_doc: s("04"),
                serie: s("L001"),
                correlativo: s("1"),
                des_motivo_baja: s("ERROR EN LA CANTIDAD ACOPIADA"),
            }]),
        },
    }))
}
