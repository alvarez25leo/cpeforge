//! Construye una factura con dos líneas y muestra su nombre de archivo SUNAT.
//!
//! `cargo run -p cpeforge-core --example invoice_model`

use std::process::ExitCode;
use std::str::FromStr;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::{Decimal, Document, Zoned};

fn detail(code: &str, quantity: Decimal, unit_value: Decimal) -> SaleDetail {
    let value = quantity * unit_value;
    let igv = (value * Decimal::new(18, 2)).round_dp(2);
    SaleDetail {
        cod_producto: Some(code.to_owned()),
        unidad: Some("NIU".to_owned()),
        cantidad: Some(quantity),
        descripcion: Some(format!("Producto {code}")),
        mto_valor_unitario: Some(unit_value),
        mto_base_igv: Some(value),
        porcentaje_igv: Some(Decimal::new(18, 0)),
        igv: Some(igv),
        tip_afe_igv: Some("10".to_owned()),
        total_impuestos: Some(igv),
        mto_valor_venta: Some(value),
        ..SaleDetail::default()
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let company = Company {
        ruc: Some("20000000001".to_owned()),
        razon_social: Some("EMPRESA SINTETICA S.A.C.".to_owned()),
        address: Some(Address {
            ubigueo: Some("150101".to_owned()),
            ..Address::default()
        }),
        ..Company::default()
    };
    // Los importes de la factura los calcula quien la construye: core solo los transporta.
    let details = vec![
        detail("P001", Decimal::from(2), Decimal::from_str("50.00")?),
        detail(
            "P002",
            Decimal::from_str("1.5")?,
            Decimal::from_str("20.00")?,
        ),
    ];
    let gravadas: Decimal = details.iter().filter_map(|d| d.mto_valor_venta).sum();
    let igv: Decimal = details.iter().filter_map(|d| d.igv).sum();

    let invoice = Invoice {
        base: BaseSale {
            ubl_version: Some("2.1".to_owned()),
            tipo_doc: Some("01".to_owned()),
            serie: Some("F001".to_owned()),
            correlativo: Some("123".to_owned()),
            fecha_emision: Some(Zoned::from_str("2024-01-31T23:30:00-05:00[America/Lima]")?),
            company: Some(company.clone()),
            client: Some(Client {
                tipo_doc: Some("6".to_owned()),
                num_doc: Some("20000000002".to_owned()),
                rzn_social: Some("CLIENTE SINTETICO S.A.".to_owned()),
                ..Client::default()
            }),
            tipo_moneda: Some("PEN".to_owned()),
            mto_oper_gravadas: Some(gravadas),
            mto_igv: Some(igv),
            total_impuestos: Some(igv),
            mto_imp_venta: Some(gravadas + igv),
            details: Some(details),
            legends: Some(vec![Legend {
                code: Some("1000".to_owned()),
                value: Some("SON CIENTO CUARENTA Y ...".to_owned()),
            }]),
            forma_pago: Some(PaymentTerms::contado()),
            ..BaseSale::default()
        },
        tipo_operacion: Some("0101".to_owned()),
        ..Invoice::default()
    };
    println!("factura: {} (total {})", invoice.name()?, gravadas + igv);

    // El identificador del resumen usa la fecha de Perú aunque la fecha esté en UTC.
    let summary = Summary {
        correlativo: Some("001".to_owned()),
        fec_resumen: Some(Zoned::from_str("2024-02-01T03:00:00+00:00[UTC]")?),
        company: Some(company),
        ..Summary::default()
    };
    println!("resumen: {}", summary.name()?);
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
