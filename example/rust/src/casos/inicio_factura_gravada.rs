//! F001-1: factura gravada al contado.

use cpeforge_core::Decimal;
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, inicio_base, inicio_cliente_empresa, inicio_linea_gravada, s};

pub fn documento(c: &Contexto) -> Resultado {
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            forma_pago: Some(PaymentTerms::contado()),
            mto_oper_gravadas: d(10000, 2),
            mto_igv: d(1800, 2),
            total_impuestos: d(1800, 2),
            mto_imp_venta: d(11800, 2),
            details: Some(vec![inicio_linea_gravada(
                "P001",
                "PRODUCTO 1",
                Decimal::new(2, 0),
                Decimal::new(5000, 2),
                Decimal::new(10000, 2),
                Decimal::new(1800, 2),
                Decimal::new(5900, 2),
            )]),
            // Monto en letras - Catálogo 52
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON CIENTO DIECIOCHO CON 00/100 SOLES"),
            }]),
            ..inicio_base("01", "F001", "1", c, inicio_cliente_empresa())
        },
        tipo_operacion: s("0101"), // Venta interna - Catálogo 51
        valor_venta: d(10000, 2),
        sub_total: d(11800, 2),
        ..Invoice::default()
    }))
}
