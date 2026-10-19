//! Casos de ejemplo. Cada módulo genera un comprobante con el mismo nombre
//! y construye el documento a partir del [`Contexto`].

use std::error::Error;

use cpeforge::DocumentRef;
use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{Invoice, Note};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};

use crate::datos::Contexto;

mod boleta;
mod boleta_icbper;
mod comunicacion_baja;
mod factura;
mod factura_anticipo;
mod factura_beatose;
mod factura_compleja;
mod factura_contingencia;
mod factura_credito;
mod factura_descuento_global;
mod factura_descuento_linea;
mod factura_descuento_nobase;
mod factura_detraccion;
mod factura_exonerado;
mod factura_exportacion;
mod factura_gratuita;
mod factura_icbper;
mod factura_icbper_gratuita;
mod factura_inafecta;
mod factura_isc;
mod factura_ivap;
mod factura_percepcion;
mod factura_rel_guia_remision;
mod factura_retencion;
mod guia_extra_atributos;
mod guia_misma_empresa;
mod guia_remision;
mod guia_transportem1l;
mod guia_transporteprivado;
mod inicio_boleta;
mod inicio_factura_credito;
mod inicio_factura_exonerada;
mod inicio_factura_gravada;
mod nota_credito;
mod nota_credito_boleta;
mod nota_credito_forma_pago;
mod nota_debito;
mod percepcion;
mod resumen;
mod resumen_contingencia;
mod resumen_usd;
mod retencion;
mod reversion;

/// Documento construido por un caso.
// Cada documento se construye, firma y envía una vez: el tamaño de la variante
// mayor no importa aquí y `Box` solo complicaría los casos.
#[allow(clippy::large_enum_variant)]
pub enum Documento {
    /// Factura o boleta.
    Invoice(Invoice),
    /// Nota de crédito o débito.
    Note(Note),
    /// Guía de remisión.
    Despatch(Despatch),
    /// Resumen diario.
    Summary(Summary),
    /// Comunicación de baja.
    Voided(Voided),
    /// Reversión.
    Reversion(Reversion),
    /// Percepción.
    Perception(Perception),
    /// Retención.
    Retention(Retention),
}

impl Documento {
    /// Referencia que aceptan `See` y `Api`.
    pub fn as_ref(&self) -> DocumentRef<'_> {
        match self {
            Self::Invoice(d) => DocumentRef::Invoice(d),
            Self::Note(d) => DocumentRef::Note(d),
            Self::Despatch(d) => DocumentRef::Despatch(d),
            Self::Summary(d) => DocumentRef::Summary(d),
            Self::Voided(d) => DocumentRef::Voided(d),
            Self::Reversion(d) => DocumentRef::Reversion(d),
            Self::Perception(d) => DocumentRef::Perception(d),
            Self::Retention(d) => DocumentRef::Retention(d),
        }
    }
}

/// Resultado de un caso.
pub type Resultado = Result<Documento, Box<dyn Error>>;

/// Función que construye el documento de un caso.
pub type Constructor = fn(&Contexto) -> Resultado;

/// Casos en orden de nombre (el de `ksort` en la referencia).
pub const CASOS: &[(&str, Constructor)] = &[
    ("boleta", boleta::documento),
    ("boleta-icbper", boleta_icbper::documento),
    ("comunicacion-baja", comunicacion_baja::documento),
    ("factura", factura::documento),
    ("factura-anticipo", factura_anticipo::documento),
    ("factura-beatose", factura_beatose::documento),
    ("factura-compleja", factura_compleja::documento),
    ("factura-contingencia", factura_contingencia::documento),
    ("factura-credito", factura_credito::documento),
    (
        "factura-descuento-global",
        factura_descuento_global::documento,
    ),
    (
        "factura-descuento-linea",
        factura_descuento_linea::documento,
    ),
    (
        "factura-descuento-nobase",
        factura_descuento_nobase::documento,
    ),
    ("factura-detraccion", factura_detraccion::documento),
    ("factura-exonerado", factura_exonerado::documento),
    ("factura-exportacion", factura_exportacion::documento),
    ("factura-gratuita", factura_gratuita::documento),
    ("factura-icbper", factura_icbper::documento),
    (
        "factura-icbper-gratuita",
        factura_icbper_gratuita::documento,
    ),
    ("factura-inafecta", factura_inafecta::documento),
    ("factura-isc", factura_isc::documento),
    ("factura-ivap", factura_ivap::documento),
    ("factura-percepcion", factura_percepcion::documento),
    (
        "factura-rel-guia-remision",
        factura_rel_guia_remision::documento,
    ),
    ("factura-retencion", factura_retencion::documento),
    ("guia-extra-atributos", guia_extra_atributos::documento),
    ("guia-misma-empresa", guia_misma_empresa::documento),
    ("guia-remision", guia_remision::documento),
    ("guia-transporteM1L", guia_transportem1l::documento),
    ("guia-transportePrivado", guia_transporteprivado::documento),
    ("inicio-boleta", inicio_boleta::documento),
    ("inicio-factura-credito", inicio_factura_credito::documento),
    (
        "inicio-factura-exonerada",
        inicio_factura_exonerada::documento,
    ),
    ("inicio-factura-gravada", inicio_factura_gravada::documento),
    ("nota-credito", nota_credito::documento),
    ("nota-credito-boleta", nota_credito_boleta::documento),
    (
        "nota-credito-forma-pago",
        nota_credito_forma_pago::documento,
    ),
    ("nota-debito", nota_debito::documento),
    ("percepcion", percepcion::documento),
    ("resumen", resumen::documento),
    ("resumen-contingencia", resumen_contingencia::documento),
    ("resumen-usd", resumen_usd::documento),
    ("retencion", retencion::documento),
    ("reversion", reversion::documento),
];
