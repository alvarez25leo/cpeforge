//! Genera el XML de una guía de remisión versión 2022.
//!
//! `cargo run -p cpeforge-xml --example despatch_2022_xml`

use std::process::ExitCode;
use std::str::FromStr;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::despatch::{
    Despatch, DespatchDetail, Direction, Driver, Shipment, Vehicle,
};
use cpeforge_core::{Decimal, Document, Zoned};
use cpeforge_xml::{DespatchBuilder, RenderOptions};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let despatch = Despatch {
        version: Some("2022".into()),
        tipo_doc: Some("09".into()),
        serie: Some("T001".into()),
        correlativo: Some("1".into()),
        fecha_emision: Some(Zoned::from_str("2024-06-15T10:00:00-05:00[America/Lima]")?),
        company: Some(Company {
            ruc: Some("20000000001".into()),
            razon_social: Some("EMPRESA SINTETICA".into()),
            ..Company::default()
        }),
        destinatario: Some(Client {
            tipo_doc: Some("6".into()),
            num_doc: Some("20000000002".into()),
            rzn_social: Some("DESTINATARIO".into()),
            ..Client::default()
        }),
        envio: Some(Shipment {
            cod_traslado: Some("01".into()),
            mod_traslado: Some("02".into()),
            peso_total: Some(Decimal::from_str("12.5")?),
            und_peso_total: Some("KGM".into()),
            fec_traslado: Some(Zoned::from_str("2024-06-16T08:00:00-05:00[America/Lima]")?),
            vehiculo: Some(Vehicle {
                placa: Some("ABC123".into()),
                ..Vehicle::default()
            }),
            choferes: Some(vec![Driver {
                tipo: Some("Principal".into()),
                tipo_doc: Some("1".into()),
                nro_doc: Some("00000002".into()),
                nombres: Some("JUAN".into()),
                apellidos: Some("PRUEBA".into()),
                licencia: Some("Q00000002".into()),
            }]),
            llegada: Some(Direction::new(
                Some("150101".into()),
                Some("AV. LLEGADA 1".into()),
            )),
            partida: Some(Direction::new(
                Some("150203".into()),
                Some("AV. PARTIDA 2".into()),
            )),
            ..Shipment::default()
        }),
        details: Some(vec![DespatchDetail {
            codigo: Some("P001".into()),
            descripcion: Some("PRODUCTO".into()),
            unidad: Some("NIU".into()),
            cantidad: Some(Decimal::from(2)),
            ..DespatchDetail::default()
        }]),
        ..Despatch::default()
    };
    let xml = DespatchBuilder::new(RenderOptions::default()).render(&despatch)?;
    println!("{}.xml ({} bytes)", despatch.name()?, xml.len());
    println!("{xml}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
