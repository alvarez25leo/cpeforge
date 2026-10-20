//! L001-2, liquidación de compra de oro (operación 0503): oro de minería
//! artesanal con la retención de renta del 4 % y las cuatro propiedades del
//! derecho minero (catálogo 55). Solo existe en CPEForge.

use cpeforge_core::model::sale::{DetailAttribute, Legend};
use cpeforge_core::model::settlement::{PurchaseSettlement, SettlementDetail, SettlementPlace};

use super::liquidacion_compra::{direccion, vendedor};
use super::{Documento, Resultado};
use crate::datos::{Contexto, d, inicio_emisor, s};

pub fn documento(c: &Contexto) -> Resultado {
    let propiedad = |code: &str, name: &str, value: &str| DetailAttribute {
        code: s(code),
        name: s(name),
        value: s(value),
        ..DetailAttribute::default()
    };
    let mut vendedor = vendedor();
    vendedor.condicion = s("03"); // Catálogo 60: extracción.
    Ok(Documento::Settlement(PurchaseSettlement {
        tipo_operacion: s("0503"), // Catálogo 51: compra de oro.
        serie: s("L001"),
        correlativo: s("2"),
        fecha_emision: c.fecha(),
        tipo_moneda: s("PEN"),
        company: inicio_emisor(&c.ruc),
        vendedor: Some(vendedor),
        lugar_operacion: Some(SettlementPlace {
            address: Some(direccion(
                "170101",
                "MADRE DE DIOS",
                "TAMBOPATA",
                "TAMBOPATA",
                "PLANTA DE ACOPIO KM 12",
            )),
            condicion: s("01"),
        }),
        mto_oper_gravadas: d(1710000, 2),
        mto_igv: d(307800, 2),
        mto_base_ir: d(1710000, 2),
        mto_ir: d(68400, 2),
        total_impuestos: d(307800, 2),
        valor_venta: d(1710000, 2),
        sub_total: d(2017800, 2),
        // 20178.00 − 3078.00 de IGV − 684.00 de renta retenidos.
        mto_imp_venta: d(1641600, 2),
        legends: Some(vec![Legend {
            code: s("1000"),
            value: s("SON DIECISEIS MIL CUATROCIENTOS DIECISEIS CON 00/100 SOLES"),
        }]),
        details: Some(vec![SettlementDetail {
            unidad: s("GRM"),
            cantidad: d(60, 0),
            cod_producto: s("ORO01"),
            cod_prod_sunat: s("11101801"), // Catálogo 25: oro.
            descripcion: s("ORO EN BRUTO, MINERÍA ARTESANAL"),
            mto_valor_unitario: d(28500, 2),
            mto_precio_unitario: d(33630, 2),
            mto_valor_venta: d(1710000, 2),
            tip_afe_igv: s("10"),
            mto_base_igv: d(1710000, 2),
            porcentaje_igv: d(18, 0),
            igv: d(307800, 2),
            mto_base_ir: d(1710000, 2),
            porcentaje_ir: d(4, 0),
            ir: d(68400, 2),
            total_impuestos: d(307800, 2),
            atributos: Some(vec![
                propiedad("6000", "Código único de concesión minera", "010012718"),
                propiedad("6004", "Ley mineral", "85.50"),
                propiedad("6005", "Naturaleza del mineral", "1"),
                propiedad("6006", "Nombre del derecho minero", "MINA DEMO"),
            ]),
            ..SettlementDetail::default()
        }]),
        ..PurchaseSettlement::default()
    }))
}
