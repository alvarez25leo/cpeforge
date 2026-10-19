//! RA de una factura y
//! una nota de crédito.

use cpeforge_core::model::voided::{Voided, VoidedDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let detail1 = VoidedDetail {
        tipo_doc: s("01"),
        serie: s("F001"),
        correlativo: s("02132132"),
        des_motivo_baja: s("ERROR EN CÁLCULOS"),
    };
    let detail2 = VoidedDetail {
        tipo_doc: s("07"),
        serie: s("FC01"),
        correlativo: s("222"),
        des_motivo_baja: s("ERROR DE RUC"),
    };
    Ok(Documento::Voided(Voided {
        correlativo: s("00111"),
        // Fecha Generacion menor que Fecha comunicacion
        fec_generacion: c.dias(-3)?,
        fec_comunicacion: c.dias(-1)?,
        company: empresa(),
        details: Some(vec![detail1, detail2]),
    }))
}
