//! RC en dólares con una boleta.
//! Añade `porcentaje_igv`, que SUNAT exige hoy (error 2992) y el demo no informa.

use cpeforge_core::model::summary::{Summary, SummaryDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let detiail = SummaryDetail {
        tipo_doc: s("03"),
        serie_nro: s("B001-1"),
        estado: s("3"),
        cliente_tipo: s("1"),
        cliente_nro: s("00000000"),
        total: d(129, 0),
        mto_oper_gravadas: d(20, 0),
        mto_oper_inafectas: d(244, 1),
        mto_oper_exoneradas: d(50, 0),
        mto_oper_exportacion: d(10, 0),
        mto_otros_cargos: d(21, 0),
        // Campo opcional del modelo que no fija el caso base: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        porcentaje_igv: d(18, 0),
        mto_igv: d(36, 1),
        ..SummaryDetail::default()
    };
    Ok(Documento::Summary(Summary {
        fec_generacion: c.dias(-3)?,
        fec_resumen: c.dias(-1)?,
        correlativo: s("211"),
        moneda: s("USD"),
        company: empresa(),
        details: Some(vec![detiail]),
    }))
}
