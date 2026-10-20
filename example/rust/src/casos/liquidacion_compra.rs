//! L001-1, liquidación de compra (tipo 04, operación 0501): el comprador
//! adquiere café pergamino y sacos a un productor sin RUC y le retiene el IGV
//! y el 1.5 % de renta. Solo existe en CPEForge.

use cpeforge_core::model::company::Address;
use cpeforge_core::model::sale::Legend;
use cpeforge_core::model::settlement::{
    PurchaseSettlement, SettlementDetail, SettlementPlace, SettlementSeller,
};

use super::{Documento, Resultado};
use crate::datos::{Contexto, d, inicio_emisor, s};

pub fn documento(c: &Contexto) -> Resultado {
    Ok(Documento::Settlement(PurchaseSettlement {
        tipo_operacion: s("0501"), // Catálogo 51: compra interna.
        serie: s("L001"),
        correlativo: s("1"),
        fecha_emision: c.fecha(),
        tipo_moneda: s("PEN"),
        company: inicio_emisor(&c.ruc),
        vendedor: Some(vendedor()),
        lugar_operacion: Some(SettlementPlace {
            address: Some(direccion(
                "150101",
                "LIMA",
                "LIMA",
                "LIMA",
                "AV. VILLA NUEVA 221, ALMACÉN",
            )),
            condicion: s("01"), // Catálogo 60: punto de venta.
        }),
        mto_oper_gravadas: d(70000, 2),
        mto_oper_exoneradas: d(20000, 2),
        mto_igv: d(12600, 2),
        mto_base_ir: d(90000, 2),
        mto_ir: d(1350, 2),
        total_impuestos: d(12600, 2),
        valor_venta: d(90000, 2),
        sub_total: d(102600, 2),
        // 1026.00 − 126.00 de IGV − 13.50 de renta retenidos.
        mto_imp_venta: d(88650, 2),
        legends: Some(vec![Legend {
            code: s("1000"),
            value: s("SON OCHOCIENTOS OCHENTA Y SEIS CON 50/100 SOLES"),
        }]),
        details: Some(vec![
            SettlementDetail {
                unidad: s("KGM"),
                cantidad: d(200, 0),
                cod_producto: s("CAF01"),
                cod_prod_sunat: s("50201706"), // Catálogo 25: café.
                descripcion: s("CAFÉ PERGAMINO SECO, PRIMERA"),
                mto_valor_unitario: d(350, 2),
                mto_precio_unitario: d(413, 2),
                mto_valor_venta: d(70000, 2),
                tip_afe_igv: s("10"),
                mto_base_igv: d(70000, 2),
                porcentaje_igv: d(18, 0),
                igv: d(12600, 2),
                mto_base_ir: d(70000, 2),
                porcentaje_ir: d(15, 1),
                ir: d(1050, 2),
                total_impuestos: d(12600, 2),
                ..SettlementDetail::default()
            },
            SettlementDetail {
                unidad: s("NIU"),
                cantidad: d(10, 0),
                cod_producto: s("SAC01"),
                cod_prod_sunat: s("24111500"), // Catálogo 25: sacos.
                descripcion: s("SACOS DE YUTE USADOS"),
                mto_valor_unitario: d(2000, 2),
                mto_precio_unitario: d(2000, 2),
                mto_valor_venta: d(20000, 2),
                tip_afe_igv: s("20"),
                mto_base_igv: d(20000, 2),
                porcentaje_igv: d(0, 0),
                igv: d(0, 2),
                mto_base_ir: d(20000, 2),
                porcentaje_ir: d(15, 1),
                ir: d(300, 2),
                total_impuestos: d(0, 2),
                ..SettlementDetail::default()
            },
        ]),
        ..PurchaseSettlement::default()
    }))
}

/// Productor sin RUC con DNI.
pub fn vendedor() -> SettlementSeller {
    SettlementSeller {
        tipo_doc: s("1"), // Catálogo 06: DNI.
        num_doc: s("40123456"),
        rzn_social: s("QUISPE MAMANI JUAN CARLOS"),
        address: Some(direccion(
            "120301",
            "JUNÍN",
            "CHANCHAMAYO",
            "CHANCHAMAYO",
            "SECTOR SAN RAMÓN, PARCELA 12",
        )),
        condicion: s("02"), // Catálogo 60: producción.
    }
}

/// Dirección peruana con su ubigeo.
pub fn direccion(
    ubigeo: &str,
    departamento: &str,
    provincia: &str,
    distrito: &str,
    linea: &str,
) -> Address {
    Address {
        ubigueo: s(ubigeo),
        departamento: s(departamento),
        provincia: s(provincia),
        distrito: s(distrito),
        direccion: s(linea),
        ..Address::default()
    }
}
