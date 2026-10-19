//! T001-120, guía 2022 de
//! traslado entre establecimientos de la misma empresa en vehículo M1/L.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{Despatch, DespatchDetail, Direction, Shipment};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa_gre, s};

// El demo usa 20123456789, su emisor GRE de entonces. En un traslado entre
// establecimientos de la misma empresa, el destinatario y los anexos de
// partida y llegada deben ser el emisor (errores 2554 y 3414).
pub fn documento(c: &Contexto) -> Resultado {
    let envio = Shipment {
        cod_traslado: s("04"), // Cat.20 - Traslado entre establecimientos de la misma empresa
        mod_traslado: s("02"), // Cat.18 - Transp. Privado
        indicadores: Some(vec!["SUNAT_Envio_IndicadorTrasladoVehiculoM1L".to_owned()]), // Transp M1 y L
        fec_traslado: c.fecha(),
        peso_total: d(125, 1),
        und_peso_total: s("KGM"),
        llegada: Some(Direction {
            ruc: Some(c.gre_ruc.clone()),
            cod_local: s("00002"), // Código de establecimiento anexo
            ..Direction::new(s("150101"), s("AV LIMA"))
        }),
        partida: Some(Direction {
            ruc: Some(c.gre_ruc.clone()),
            cod_local: s("00001"),
            ..Direction::new(s("150203"), s("AV ITALIA"))
        }),
        ..Shipment::default()
    };
    let detail = DespatchDetail {
        cantidad: d(2, 0),
        unidad: s("ZZ"),
        descripcion: s("PROD 1"),
        codigo: s("PROD1"),
        ..DespatchDetail::default()
    };
    Ok(Documento::Despatch(Despatch {
        version: s("2022"),
        tipo_doc: s("09"),
        serie: s("T001"),
        correlativo: s("120"),
        fecha_emision: c.fecha(),
        company: empresa_gre(c),
        // misma empresa
        destinatario: Some(Client {
            tipo_doc: s("6"),
            num_doc: Some(c.gre_ruc.clone()),
            rzn_social: s("CPEFORGE SAC"),
            ..Client::default()
        }),
        envio: Some(envio),
        details: Some(vec![detail]),
        ..Despatch::default()
    }))
}
