//! RC con una boleta de
//! contingencia (serie numérica 0001). Usa el correlativo 002 en lugar del 001
//! del demo y añade `porcentaje_igv`, que SUNAT exige hoy (error 2992).

use cpeforge_core::model::summary::{Summary, SummaryDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let detiail = SummaryDetail {
        tipo_doc: s("03"),
        serie_nro: s("0001-1"),
        estado: s("3"),
        cliente_tipo: s("1"),
        cliente_nro: s("00000000"),
        total: d(119_555, 3),
        mto_oper_gravadas: d(20, 0),
        mto_oper_inafectas: d(244, 1),
        mto_oper_exoneradas: d(50, 0),
        mto_otros_cargos: d(21_555, 3),
        // Campo opcional del modelo que no fija el caso base: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        porcentaje_igv: d(18, 0),
        mto_igv: d(36, 1),
        ..SummaryDetail::default()
    };
    Ok(Documento::Summary(Summary {
        fec_generacion: c.dias(-1)?,
        fec_resumen: c.dias(-1)?,
        // El demo usa "001", que choca con el caso `resumen` del mismo día.
        correlativo: s("002"),
        company: empresa(),
        details: Some(vec![detiail]),
        ..Summary::default()
    }))
}
