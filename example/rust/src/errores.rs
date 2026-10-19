//! Casos que producen rechazos tributarios en SUNAT BETA.
//!
//! Parten de ejemplos positivos existentes para cambiar solo el dato que
//! dispara la regla; cada nombre tiene un constructor de la referencia equivalente.

use cpeforge_core::model::sale::{Cuota, Note, PaymentTerms};
use cpeforge_core::model::summary::Summary;

use crate::casos::{CASOS, Constructor, Documento, Resultado};
use crate::datos::{Contexto, d, s};

fn positivo(nombre: &str, contexto: &Contexto) -> Resultado {
    let construir: Constructor = CASOS
        .iter()
        .find(|(caso, _)| *caso == nombre)
        .map(|(_, construir)| *construir)
        .ok_or_else(|| format!("No existe el caso positivo {nombre}"))?;
    construir(contexto)
}

fn nota_motivo13_con_total(contexto: &Contexto) -> Resultado {
    let Documento::Note(mut nota): Documento = positivo("nota-credito", contexto)? else {
        return Err("nota-credito no construyó una nota".into());
    };
    let Note {
        base, cod_motivo, ..
    } = &mut nota;
    base.correlativo = s("901");
    *cod_motivo = s("13");
    base.forma_pago = Some(PaymentTerms::credito(d(236, 0), None));
    base.cuotas = Some(vec![Cuota {
        monto: d(236, 0),
        fecha_pago: contexto.dias(5)?,
        ..Cuota::default()
    }]);
    Ok(Documento::Note(nota))
}

fn resumen_sin_tasa(contexto: &Contexto) -> Resultado {
    let Documento::Summary(mut resumen): Documento = positivo("resumen", contexto)? else {
        return Err("resumen no construyó un resumen".into());
    };
    let Summary {
        correlativo,
        details,
        ..
    } = &mut resumen;
    *correlativo = s("901");
    if let Some(details) = details {
        for detalle in details {
            detalle.porcentaje_igv = None;
        }
    }
    Ok(Documento::Summary(resumen))
}

fn guia_sin_fecha_entrega(contexto: &Contexto) -> Resultado {
    let Documento::Despatch(mut guia): Documento = positivo("guia-remision", contexto)? else {
        return Err("guia-remision no construyó una guía".into());
    };
    guia.correlativo = s("901");
    let envio = guia.envio.as_mut().ok_or("guía sin envío")?;
    envio.fec_entrega_bienes = None;
    Ok(Documento::Despatch(guia))
}

/// Casos de rechazo, en el mismo orden que `ksort` de la referencia.
pub const CASOS_ERROR: &[(&str, Constructor)] = &[
    ("guia-sin-fecha-entrega", guia_sin_fecha_entrega),
    ("nota-motivo13-con-total", nota_motivo13_con_total),
    ("resumen-sin-tasa", resumen_sin_tasa),
];
