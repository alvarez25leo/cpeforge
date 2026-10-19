//! F001-126 con líneas
//! gravada, exonerada, inafecta y gratuitas.

use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

pub fn documento(c: &Contexto) -> Resultado {
    let items = vec![
        // Gravado
        SaleDetail {
            cod_producto: s("P001"),
            unidad: s("NIU"),
            descripcion: s("PROD 1"),
            cantidad: d(2, 0),
            mto_valor_unitario: d(100, 0),
            mto_valor_venta: d(200, 0),
            mto_base_igv: d(200, 0),
            porcentaje_igv: d(18, 0),
            igv: d(36, 0),
            tip_afe_igv: s("10"), // Catalog 07: Gravado
            total_impuestos: d(36, 0),
            mto_precio_unitario: d(118, 0),
            ..SaleDetail::default()
        },
        // Exonerado
        SaleDetail {
            cod_producto: s("P002"),
            unidad: s("KG"),
            descripcion: s("PROD 2"),
            cantidad: d(2, 0),
            mto_valor_unitario: d(50, 0),
            mto_valor_venta: d(100, 0),
            mto_base_igv: d(100, 0),
            porcentaje_igv: d(0, 0),
            igv: d(0, 0),
            tip_afe_igv: s("20"), // Catalog 07: Exonerado
            total_impuestos: d(0, 0),
            mto_precio_unitario: d(50, 0),
            ..SaleDetail::default()
        },
        // Inafecto
        SaleDetail {
            cod_producto: s("P003"),
            unidad: s("NIU"),
            descripcion: s("PROD 3"),
            cantidad: d(2, 0),
            mto_valor_unitario: d(100, 0),
            mto_valor_venta: d(200, 0),
            mto_base_igv: d(200, 0),
            porcentaje_igv: d(0, 0),
            igv: d(0, 0),
            tip_afe_igv: s("30"), // Catalog 07: Inafecto
            total_impuestos: d(0, 0),
            mto_precio_unitario: d(100, 0),
            ..SaleDetail::default()
        },
        // Gravado gratuito
        SaleDetail {
            cod_producto: s("P004"),
            unidad: s("NIU"),
            descripcion: s("PROD 4"),
            cantidad: d(1, 0),
            mto_valor_unitario: d(0, 0),
            mto_valor_gratuito: d(100, 0),
            mto_valor_venta: d(100, 0),
            mto_base_igv: d(100, 0),
            porcentaje_igv: d(18, 0),
            igv: d(18, 0),
            tip_afe_igv: s("13"), // Catalog 07: Gravado - Retiro,
            total_impuestos: d(18, 0),
            mto_precio_unitario: d(0, 0),
            ..SaleDetail::default()
        },
        // Inafecto gratuito
        SaleDetail {
            cod_producto: s("P005"),
            unidad: s("NIU"),
            descripcion: s("PROD 5"),
            cantidad: d(2, 0),
            mto_valor_unitario: d(0, 0),
            mto_valor_gratuito: d(100, 0),
            mto_valor_venta: d(200, 0),
            mto_base_igv: d(200, 0),
            porcentaje_igv: d(0, 0),
            igv: d(0, 0),
            tip_afe_igv: s("32"), // Catalog 07: Inafecto - Retiro,
            total_impuestos: d(0, 0),
            mto_precio_unitario: d(0, 0),
            ..SaleDetail::default()
        },
    ];
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("126"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(200, 0),
            mto_oper_exoneradas: d(100, 0),
            mto_oper_inafectas: d(200, 0),
            mto_oper_gratuitas: d(300, 0),
            mto_igv: d(36, 0),
            mto_igv_gratuitas: d(18, 0),
            total_impuestos: d(36, 0), // IGV + ISC + OTH
            mto_imp_venta: d(536, 0),
            details: Some(items),
            legends: Some(vec![
                Legend {
                    code: s("1000"),
                    value: s("SON QUINIENTOS TREINTA Y SEIS CON OO/100 SOLES"),
                },
                Legend {
                    code: s("1002"),
                    value: s(
                        "TRANSFERENCIA GRATUITA DE UN BIEN Y/O SERVICIO PRESTADO GRATUITAMENTE",
                    ),
                },
            ]),
            ..BaseSale::default()
        },
        fec_vencimiento: c.fecha(),
        tipo_operacion: s("0101"),
        valor_venta: d(500, 0),
        sub_total: d(536, 0),
        ..Invoice::default()
    }))
}
