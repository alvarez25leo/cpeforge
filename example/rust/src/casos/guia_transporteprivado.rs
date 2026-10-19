//! T001-123, guía 2022
//! por transporte privado con vehículo principal, secundario y conductor.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{
    Despatch, DespatchDetail, Direction, Driver, Shipment, Vehicle,
};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa_gre, s};

pub fn documento(c: &Contexto) -> Resultado {
    let vehiculo_secundario = Vehicle {
        placa: s("ABI456"),
        ..Vehicle::default()
    };
    let vehiculo_principal = Vehicle {
        placa: s("ABC123"),
        secundarios: Some(vec![vehiculo_secundario]), // opcional
        ..Vehicle::default()
    };
    let chofer = Driver {
        tipo: s("Principal"),
        tipo_doc: s("1"),
        nro_doc: s("44004400"),
        licencia: s("0001122020"),
        nombres: s("ROBERTO"),
        apellidos: s("RODRIGUEZ VALENCIA"),
    };
    let envio = Shipment {
        cod_traslado: s("01"), // Cat.20 - Venta
        mod_traslado: s("02"), // Cat.18 - Transp. Privado
        fec_traslado: c.fecha(),
        peso_total: d(125, 1),
        und_peso_total: s("KGM"),
        vehiculo: Some(vehiculo_principal),
        choferes: Some(vec![chofer]),
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
        tipo_doc: s("09"),
        serie: s("T001"),
        correlativo: s("123"),
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
