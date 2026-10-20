//! F001-150 con un cargo global que no afecta la base imponible: recargo al
//! consumo del 10 % (catálogo 53, código 46).

use cpeforge_core::model::sale::{BaseSale, Charge, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let item = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        descripcion: s("Menú ejecutivo"),
        cantidad: d(2, 0),
        mto_valor_unitario: d(50, 0),
        mto_valor_venta: d(100, 0),
        mto_base_igv: d(100, 0),
        porcentaje_igv: d(18, 0),
        igv: d(18, 0),
        tip_afe_igv: s("10"),
        total_impuestos: d(18, 0),
        mto_precio_unitario: d(59, 0),
        ..SaleDetail::default()
    };
    // El recargo se suma al total a pagar (ChargeTotalAmount), sin IGV.
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("150"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(100, 0),
            mto_igv: d(18, 0),
            total_impuestos: d(18, 0),
            sum_otros_cargos: d(10, 0),
            mto_imp_venta: d(128, 0), // 118 + 10 de recargo.
            details: Some(vec![item]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON CIENTO VEINTIOCHO CON 00/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: s("0101"),
        cargos: Some(vec![Charge {
            cod_tipo: s("46"), // Catálogo 53: recargo al consumo y/o propinas.
            factor: d(10, 2),
            monto: d(10, 0),
            monto_base: d(100, 0),
        }]),
        valor_venta: d(100, 0),
        sub_total: d(118, 0),
        ..Invoice::default()
    }))
}
