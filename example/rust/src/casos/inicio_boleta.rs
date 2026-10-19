//! B001-3: boleta a persona natural (DNI).
//!
//! Gravadas 81.00 (30.00 + 50.00 + bolsa 1.00), IGV 14.58, ICBPER 1.00.
//! Total = 81.00 + 14.58 + 1.00 = 96.58.

use cpeforge_core::Decimal;
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms};

use super::{Documento, Resultado};
use crate::datos::{
    Contexto, d, inicio_base, inicio_cliente_persona, inicio_linea_bolsa, inicio_linea_gravada, s,
};

pub fn documento(c: &Contexto) -> Resultado {
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            forma_pago: Some(PaymentTerms::contado()),
            mto_oper_gravadas: d(8100, 2),
            mto_igv: d(1458, 2),
            icbper: d(100, 2),
            total_impuestos: d(1558, 2),
            mto_imp_venta: d(9658, 2),
            details: Some(vec![
                inicio_linea_gravada(
                    "C023",
                    "CUADERNO A4",
                    Decimal::new(3, 0),
                    Decimal::new(1000, 2),
                    Decimal::new(3000, 2),
                    Decimal::new(540, 2),
                    Decimal::new(1180, 2),
                ),
                inicio_linea_gravada(
                    "C024",
                    "LAPICERO AZUL X 10",
                    Decimal::new(2, 0),
                    Decimal::new(2500, 2),
                    Decimal::new(5000, 2),
                    Decimal::new(900, 2),
                    Decimal::new(2950, 2),
                ),
                inicio_linea_bolsa(
                    Decimal::new(2, 0),
                    Decimal::new(100, 2),
                    Decimal::new(18, 2),
                    Decimal::new(100, 2),
                ),
            ]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON NOVENTA Y SEIS CON 58/100 SOLES"),
            }]),
            ..inicio_base("03", "B001", "3", c, inicio_cliente_persona())
        },
        tipo_operacion: s("0101"),
        valor_venta: d(8100, 2),
        sub_total: d(9658, 2),
        ..Invoice::default()
    }))
}
