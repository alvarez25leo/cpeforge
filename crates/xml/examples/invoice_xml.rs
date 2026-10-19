//! Genera el XML UBL 2.1 sin firma de una factura.
//!
//! `cargo run -p cpeforge-xml --example invoice_xml`

use std::process::ExitCode;
use std::str::FromStr;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};
use cpeforge_core::{Decimal, Document, Zoned};
use cpeforge_xml::{InvoiceBuilder, RenderOptions};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let amount = |text: &str| Decimal::from_str(text);
    let invoice = Invoice {
        base: BaseSale {
            ubl_version: Some("2.1".into()),
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("123".into()),
            fecha_emision: Some(Zoned::from_str("2024-01-31T23:30:00-05:00[America/Lima]")?),
            tipo_moneda: Some("PEN".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                razon_social: Some("EMPRESA SINTETICA S.A.C.".into()),
                address: Some(Address {
                    ubigueo: Some("150101".into()),
                    direccion: Some("AV. FICTICIA 123".into()),
                    ..Address::default()
                }),
                ..Company::default()
            }),
            client: Some(Client {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000002".into()),
                rzn_social: Some("CLIENTE".into()),
                ..Client::default()
            }),
            mto_oper_gravadas: Some(amount("100.00")?),
            mto_igv: Some(amount("18.00")?),
            total_impuestos: Some(amount("18.00")?),
            mto_imp_venta: Some(amount("118.00")?),
            forma_pago: Some(PaymentTerms::contado()),
            legends: Some(vec![Legend {
                code: Some("1000".into()),
                value: Some("SON CIENTO DIECIOCHO CON 00/100 SOLES".into()),
            }]),
            details: Some(vec![SaleDetail {
                cod_producto: Some("P001".into()),
                unidad: Some("NIU".into()),
                cantidad: Some(amount("2")?),
                descripcion: Some("PRODUCTO".into()),
                mto_valor_unitario: Some(amount("50.00")?),
                mto_base_igv: Some(amount("100.00")?),
                porcentaje_igv: Some(amount("18")?),
                igv: Some(amount("18.00")?),
                tip_afe_igv: Some("10".into()),
                total_impuestos: Some(amount("18.00")?),
                mto_valor_venta: Some(amount("100.00")?),
                mto_precio_unitario: Some(amount("59.00")?),
                ..SaleDetail::default()
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: Some("0101".into()),
        valor_venta: Some(amount("100.00")?),
        sub_total: Some(amount("118.00")?),
        ..Invoice::default()
    };
    let xml = InvoiceBuilder::new(RenderOptions::default()).render(&invoice)?;
    println!("{}.xml ({} bytes)", invoice.name()?, xml.len());
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
