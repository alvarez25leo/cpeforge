//! FF01-124, ajuste
//! de montos y fechas de pago con forma de pago al crédito y una cuota.
//!
//! SUNAT exige hoy que una nota con motivo 13 tenga todos los importes en cero
//! (error 3315); el demo de 2023 usaba 236.00. La cuota conserva el monto del
//! nuevo cronograma de pago.

use cpeforge_core::model::sale::{
    BaseSale, Cuota, Legend, Note, PaymentTerms, RelatedDocument, SaleDetail,
};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

fn detalle(codigo: &str, descripcion: &str) -> SaleDetail {
    SaleDetail {
        cod_producto: s(codigo),
        unidad: s("NIU"),
        cantidad: d(2, 0),
        descripcion: s(descripcion),
        mto_base_igv: d(0, 0),
        porcentaje_igv: d(1800, 2),
        igv: d(0, 0),
        tip_afe_igv: s("10"),
        total_impuestos: d(0, 0),
        mto_valor_venta: d(0, 0),
        mto_valor_unitario: d(0, 0),
        mto_precio_unitario: d(0, 0),
        ..SaleDetail::default()
    }
}

pub fn documento(c: &Contexto) -> Resultado {
    Ok(Documento::Note(Note {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("07"),
            serie: s("FF01"),
            correlativo: s("124"),
            fecha_emision: c.fecha(),
            // Forma de Pago
            forma_pago: Some(PaymentTerms::credito(d(236, 0), None)),
            cuotas: Some(vec![Cuota {
                monto: d(236, 0),
                fecha_pago: c.dias(5)?,
                ..Cuota::default()
            }]),
            tipo_moneda: s("PEN"),
            // Guias (Opcional)
            guias: Some(vec![RelatedDocument {
                tipo_doc: s("09"),
                nro_doc: s("0001-213"),
            }]),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(0, 0),
            mto_igv: d(0, 0),
            total_impuestos: d(0, 0),
            mto_imp_venta: d(0, 0),
            details: Some(vec![detalle("C023", "PROD 1"), detalle("C02", "PROD 2")]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON CERO CON 00/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        tip_doc_afectado: s("01"),       // Tipo Doc: Factura
        num_doc_afectado: s("F001-111"), // Factura: Serie-Correlativo
        cod_motivo: s("13"),             // Catalogo. 09: Ajustes – montos y/o fechas de pago
        des_motivo: s("DEVOLUCION POR ITEM"),
        ..Note::default()
    }))
}
