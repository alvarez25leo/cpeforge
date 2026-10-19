//! F001-223 con
//! guía de remisión relacionada.

use cpeforge_core::model::sale::{
    BaseSale, Invoice, Legend, PaymentTerms, RelatedDocument, SaleDetail,
};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let guia_remision = RelatedDocument {
        tipo_doc: s("09"),    // Guia de Remision remitente: 09, catalogo 01
        nro_doc: s("T001-2"), // Serie y correlativo de la guia de remision
    };
    let item = SaleDetail {
        cod_producto: s("P001"),
        unidad: s("NIU"),
        descripcion: s("PROD 1"),
        cantidad: d(2, 0),
        mto_valor_unitario: d(100, 0),
        mto_valor_venta: d(200, 0),
        mto_base_igv: d(200, 0),
        porcentaje_igv: d(18, 0),
        igv: d(36, 0),
        tip_afe_igv: s("10"),
        total_impuestos: d(36, 0),
        mto_precio_unitario: d(118, 0),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("223"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            guias: Some(vec![
                guia_remision, // Incluir guia remision.
            ]),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(200, 0),
            mto_igv: d(36, 0),
            total_impuestos: d(36, 0),
            mto_imp_venta: d(236, 0),
            details: Some(vec![item]),
            legends: Some(vec![Legend {
                code: s("1000"),
                value: s("SON TRESCIENTOS TREINTA Y SEIS CON OO/100 SOLES"),
            }]),
            ..BaseSale::default()
        },
        fec_vencimiento: c.fecha(),
        tipo_operacion: s("0101"),
        valor_venta: d(200, 0),
        sub_total: d(236, 0),
        ..Invoice::default()
    }))
}
