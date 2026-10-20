//! V001-1, guía de remisión transportista (tipo 31): el transportista que
//! emite la guía traslada los bienes de un remitente, sustentados por su GRE
//! remitente T001-121. Solo existe en CPEForge.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{
    AdditionalDoc, Despatch, DespatchDetail, Direction, Driver, Shipment, Transportist, Vehicle,
};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa_gre, s};

pub fn documento(c: &Contexto) -> Resultado {
    let envio = Shipment {
        fec_traslado: c.fecha(),
        peso_total: d(125, 1),
        und_peso_total: s("KGM"),
        // Registro MTC del transportista que emite la guía.
        transportista: Some(Transportist {
            nro_mtc: s("0001"),
            ..Transportist::default()
        }),
        vehiculo: Some(Vehicle {
            placa: s("ABC123"),
            nro_circulacion: s("15M21028250E"), // Tarjeta Única de Circulación (10 a 15 caracteres).
            ..Vehicle::default()
        }),
        choferes: Some(vec![Driver {
            tipo: s("Principal"),
            tipo_doc: s("1"),
            nro_doc: s("44556677"),
            nombres: s("JUAN"),
            apellidos: s("PEREZ"),
            licencia: s("Q44556677"),
        }]),
        llegada: Some(Direction::new(s("150101"), s("AV LIMA"))),
        partida: Some(Direction::new(s("150203"), s("AV ITALIA"))),
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
        tipo_doc: s("31"), // Catálogo 01: guía de remisión transportista.
        serie: s("V001"),
        correlativo: s("1"),
        fecha_emision: c.fecha(),
        company: empresa_gre(c),
        remitente: Some(Client {
            tipo_doc: s("6"),
            num_doc: s("20000000002"),
            rzn_social: s("EMPRESA REMITENTE"),
            ..Client::default()
        }),
        destinatario: Some(Client {
            tipo_doc: s("6"),
            num_doc: s("20000000003"),
            rzn_social: s("EMPRESA DEST 1"),
            ..Client::default()
        }),
        // GRE remitente que sustenta el traslado (catálogo 61, código 09).
        add_docs: Some(vec![AdditionalDoc {
            tipo_desc: s("Guía de Remisión Remitente"),
            tipo: s("09"),
            nro: s("T001-121"),
            emisor: s("20000000002"),
        }]),
        envio: Some(envio),
        details: Some(vec![detail]),
        ..Despatch::default()
    }))
}
