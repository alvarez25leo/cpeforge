//! RC con dos boletas (una con
//! percepción) y una nota de crédito. Añade `porcentaje_igv`, que SUNAT exige
//! hoy (error 2992) y el demo de 2023 no informa.

use cpeforge_core::model::sale::RelatedDocument;
use cpeforge_core::model::summary::{Summary, SummaryDetail, SummaryPerception};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let detiail1 = SummaryDetail {
        tipo_doc: s("03"),
        serie_nro: s("B001-1"),
        estado: s("3"),
        cliente_tipo: s("1"),
        cliente_nro: s("00000000"),
        total: d(129_555, 3),
        mto_oper_gravadas: d(20, 0),
        mto_oper_inafectas: d(244, 1),
        mto_oper_exoneradas: d(50, 0),
        mto_oper_exportacion: d(10_555, 3),
        mto_otros_cargos: d(21, 0),
        // Campo opcional del modelo que no fija el caso base: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        porcentaje_igv: d(18, 0),
        mto_igv: d(36, 1),
        ..SummaryDetail::default()
    };
    let detiail2 = SummaryDetail {
        tipo_doc: s("07"),
        serie_nro: s("B001-4"),
        doc_referencia: Some(RelatedDocument {
            tipo_doc: s("03"),
            nro_doc: s("0001-122"),
        }),
        estado: s("1"),
        cliente_tipo: s("1"),
        cliente_nro: s("00000000"),
        total: d(200, 0),
        mto_oper_gravadas: d(40, 0),
        mto_oper_exoneradas: d(30, 0),
        mto_oper_inafectas: d(120, 0),
        // Campo opcional del modelo que no fija el caso base: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        porcentaje_igv: d(18, 0),
        mto_igv: d(72, 1),
        mto_isc: d(28, 1),
        ..SummaryDetail::default()
    };
    let detiail3 = SummaryDetail {
        tipo_doc: s("03"),
        serie_nro: s("B001-2"),
        estado: s("1"),
        cliente_tipo: s("1"),
        cliente_nro: s("00000000"),
        percepcion: Some(SummaryPerception {
            cod_reg: s("01"),
            tasa: d(200, 2),
            mto_base: d(11900, 2),
            mto: d(238, 2),
            mto_total: d(12138, 2),
        }),
        total: d(119, 0),
        mto_oper_gravadas: d(20, 0),
        mto_oper_inafectas: d(244, 1),
        mto_oper_exoneradas: d(50, 0),
        mto_otros_cargos: d(21, 0),
        // Campo opcional del modelo que no fija el caso base: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        porcentaje_igv: d(18, 0),
        mto_igv: d(36, 1),
        ..SummaryDetail::default()
    };
    // Fecha Generacion menor que Fecha Resumen
    Ok(Documento::Summary(Summary {
        fec_generacion: c.dias(-3)?,
        fec_resumen: c.dias(-1)?,
        correlativo: s("001"),
        company: empresa(),
        details: Some(vec![detiail1, detiail2, detiail3]),
        ..Summary::default()
    }))
}
