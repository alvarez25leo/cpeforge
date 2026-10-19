//! T001-124, guía 2022
//! por transporte público con documento adicional y atributos del bien.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::despatch::{
    AdditionalDoc, Despatch, DespatchDetail, Direction, Shipment, Transportist,
};
use cpeforge_core::model::sale::DetailAttribute;

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, empresa_gre, s};

pub fn documento(c: &Contexto) -> Resultado {
    let rel_doc = AdditionalDoc {
        tipo: s("49"),
        tipo_desc: s("Constancia de Depósito - IVAP (Ley 28211)"),
        nro: s("00000001"),
        ..AdditionalDoc::default()
    };
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
        cod_prod_sunat: s("50161509"),
        atributos: Some(vec![
            DetailAttribute {
                code: s("7020"),
                name: s("Partida arancelaria"),
                value: s("1701130000"),
                ..DetailAttribute::default()
            },
            DetailAttribute {
                code: s("7022"),
                name: s("Indicador de bien normalizado"),
                value: s("1"),
                ..DetailAttribute::default()
            },
        ]),
    };
    Ok(Documento::Despatch(Despatch {
        version: s("2022"),
        tipo_doc: s("09"),
        serie: s("T001"),
        correlativo: s("124"),
        fecha_emision: c.fecha(),
        company: empresa_gre(c),
        destinatario: Some(Client {
            tipo_doc: s("6"),
            num_doc: s("20000000002"),
            rzn_social: s("EMPRESA DEST 1"),
            ..Client::default()
        }),
        add_docs: Some(vec![rel_doc]),
        envio: Some(envio),
        details: Some(vec![detail]),
        ..Despatch::default()
    }))
}
