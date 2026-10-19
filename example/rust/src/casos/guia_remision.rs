//! T001-121, guía 2022 por
//! transporte público.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{Despatch, DespatchDetail, Direction, Shipment, Transportist};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa_gre, s};

pub fn documento(c: &Contexto) -> Resultado {
    let transp = Transportist {
        tipo_doc: s("6"),
        num_doc: s("20000000002"),
        rzn_social: s("TRANSPORTES S.A.C"),
        nro_mtc: s("0001"),
        ..Transportist::default()
    };
    let envio = Shipment {
        cod_traslado: s("01"), // Cat.20 - Venta
        mod_traslado: s("01"), // Cat.18 - Transp. Publico
        fec_traslado: c.fecha(),
        // Campo opcional del modelo que no fija el caso base: la API GRE lo exige en
        // transporte público (error 3617).
        fec_entrega_bienes: c.fecha(),
        peso_total: d(125, 1),
        und_peso_total: s("KGM"),
        // num_bultos: Some(2), // Solo válido para importaciones
        llegada: Some(Direction::new(s("150101"), s("AV LIMA"))),
        partida: Some(Direction::new(s("150203"), s("AV ITALIA"))),
        transportista: Some(transp),
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
        correlativo: s("121"),
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
