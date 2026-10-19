//! T001-122, guía 2022 por
//! transporte privado en vehículo de categoría M1 o L.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{Despatch, DespatchDetail, Direction, Shipment};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa_gre, s};

pub fn documento(c: &Contexto) -> Resultado {
    let envio = Shipment {
        cod_traslado: s("01"), // Cat.20 - Venta
        indicadores: Some(vec!["SUNAT_Envio_IndicadorTrasladoVehiculoM1L".to_owned()]),
        mod_traslado: s("02"), // Cat.18 - Transp. Privado
        fec_traslado: c.fecha(),
        peso_total: d(125, 1),
        und_peso_total: s("KGM"),
        llegada: Some(Direction::new(s("150101"), s("AV LIMA"))),
        partida: Some(Direction::new(s("150203"), s("AV ITALIA"))),
        ..Shipment::default()
    };
    let detail = DespatchDetail {
        cantidad: d(5, 1),
        unidad: s("KGM"),
        descripcion: s("PROD 1"),
        codigo: s("PROD1"),
        ..DespatchDetail::default()
    };
    Ok(Documento::Despatch(Despatch {
        version: s("2022"),
        tipo_doc: s("09"),
        serie: s("T001"),
        correlativo: s("122"),
        fecha_emision: c.fecha(),
        company: empresa_gre(c),
        destinatario: Some(Client {
            tipo_doc: s("6"),
            num_doc: s("20000000002"),
            rzn_social: s("EMPRESA DEST 1"),
            ..Client::default()
        }),
        envio: Some(envio),
        details: Some(vec![detail]),
        ..Despatch::default()
    }))
}
