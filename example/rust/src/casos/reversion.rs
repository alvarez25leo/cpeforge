//! RR de una retención y una
//! percepción.

use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item1 = VoidedDetail {
        tipo_doc: s("20"),
        serie: s("R001"),
        correlativo: s("122"),
        des_motivo_baja: s("ERROR DE SISTEMA"),
    };
    let item2 = VoidedDetail {
        tipo_doc: s("40"),
        serie: s("P001"),
        correlativo: s("111"),
        des_motivo_baja: s("ERROR DE RUC"),
    };
    Ok(Documento::Reversion(Reversion {
        voided: Voided {
            correlativo: s("001"),
            fec_generacion: c.dias(-3)?,
            fec_comunicacion: c.dias(-1)?,
            company: empresa(),
            details: Some(vec![item1, item2]),
        },
    }))
}
