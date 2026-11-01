//! F001-81004, detracción por servicio de transporte de carga (operación
//! `1004`, bien o servicio `027`) con el origen, el destino, el detalle del
//! viaje y los valores referenciales de la línea.
//!
//! Extensión de CPEForge: la referencia no tiene este caso. Correlativo alto
//! porque el RUC de SUNAT BETA es compartido.

use cpeforge_core::model::despatch::Direction;
use cpeforge_core::model::sale::{
    BaseSale, CargoTransport, Detraction, Invoice, Legend, PaymentTerms, SaleDetail,
};

use super::{Documento, Resultado};
use crate::datos::{Contexto, cliente, d, empresa, s};

fn punto(ubigeo: &str, direccion: &str) -> Option<Direction> {
    Some(Direction {
        ubigueo: s(ubigeo),
        direccion: s(direccion),
        ..Direction::default()
    })
}

pub fn documento(c: &Contexto) -> Resultado {
    let detail = SaleDetail {
        cod_producto: s("TRC01"),
        unidad: s("ZZ"),
        descripcion: s("SERVICIO DE TRANSPORTE DE CARGA LIMA - AREQUIPA"),
        cantidad: d(1, 0),
        mto_valor_unitario: d(1000, 0),
        mto_valor_venta: d(1000, 0),
        mto_base_igv: d(1000, 0),
        porcentaje_igv: d(18, 0),
        igv: d(180, 0),
        tip_afe_igv: s("10"), // Catálogo 07 - Gravado
        total_impuestos: d(180, 0),
        mto_precio_unitario: d(1180, 0),
        transporte_carga: Some(CargoTransport {
            origen: punto("150101", "AV. LOS OLIVOS 123, LIMA"),
            destino: punto("040101", "CALLE MERCADERES 456, AREQUIPA"),
            detalle_viaje: s("CARGA GENERAL EN BODEGA, LIMA - AREQUIPA"),
            valor_ref_servicio: d(1100, 0),
            valor_ref_carga_efectiva: d(1050, 0),
            valor_ref_carga_util: d(1200, 0),
        }),
        ..SaleDetail::default()
    };
    Ok(Documento::Invoice(Invoice {
        base: BaseSale {
            ubl_version: s("2.1"),
            tipo_doc: s("01"),
            serie: s("F001"),
            correlativo: s("81004"),
            fecha_emision: c.fecha(),
            forma_pago: Some(PaymentTerms::contado()),
            tipo_moneda: s("PEN"),
            company: empresa(),
            client: cliente(),
            mto_oper_gravadas: d(1000, 0),
            mto_igv: d(180, 0),
            total_impuestos: d(180, 0),
            mto_imp_venta: d(1180, 0),
            details: Some(vec![detail]),
            legends: Some(vec![
                Legend {
                    code: s("1000"),
                    value: s("SON MIL CIENTO OCHENTA CON 00/100 SOLES"),
                },
                Legend {
                    code: s("2006"),
                    value: s("Operación sujeta a detracción"),
                },
            ]),
            ..BaseSale::default()
        },
        tipo_operacion: s("1004"), // Catálogo 51: detracción, transporte de carga
        valor_venta: d(1000, 0),
        sub_total: d(1180, 0),
        // MONEDA SIEMPRE EN SOLES
        detraccion: Some(Detraction {
            cod_bien_detraccion: s("027"), // Catálogo 54: transporte de carga
            cod_medio_pago: s("001"),      // Catálogo 59: depósito en cuenta
            cta_banco: s("0004-3342343243"),
            percent: d(400, 2),
            mount: d(4720, 2),
            ..Detraction::default()
        }),
        ..Invoice::default()
    }))
}
