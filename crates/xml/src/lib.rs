//! XML UBL sin firma de comprobantes electrónicos peruanos.
//!
//! Recibe los modelos de [`cpeforge_core`] y produce el XML UBL de los
//! comprobantes con las 13 plantillas de referencia (los antecedentes y la
//! versión de referencia están en el README):
//!
//! | Builder | Plantillas |
//! | --- | --- |
//! | [`InvoiceBuilder`] | `invoice2.0` (con `embededDespatch`), `invoice2.1` |
//! | [`NoteBuilder`] | `notacr2.0`, `notacr2.1`, `notadb2.0`, `notadb2.1` |
//! | [`DespatchBuilder`] | `despatch`, `despatch2022` |
//! | [`SummaryBuilder`] | `summary` |
//! | [`VoidedBuilder`] | `voided` (baja `RA` y reversión `RR`) |
//! | [`RetentionBuilder`] / [`PerceptionBuilder`] | `retention`, `perception` |
//! | [`SettlementBuilder`] | liquidación de compra (`SelfBilledInvoice`), propia de CPEForge |
//!
//! No firma, no comprime ni envía: el XML devuelto deja vacío el
//! `ext:ExtensionContent` reservado para la firma.
//!
//! # Comportamientos que se conservan
//!
//! - Los importes se imprimen como la referencia imprime el `float` equivalente
//!   ([`format`](mod@format)); los tributos por afectación usan la misma tabla.
//! - Las fechas se imprimen en hora de Perú. Una fecha impresa nula se
//!   sustituye por la hora actual ([`RenderOptions::now`]).
//! - Con [`Autoescape::Html`] (valor por defecto) los textos se escapan como
//!   HTML salvo los que van en `CDATA` con `|raw`; con
//!   [`Autoescape::Disabled`] (modo de la referencia) se insertan tal cual y
//!   un `&` o `<` produce XML mal formado, como en la referencia.
//! - Un `]]>` dentro de un texto en `CDATA` se divide en dos secciones
//!   (`]]]]><![CDATA[>`) para no cerrar la sección ni permitir insertar
//!   elementos; la referencia genera ahí XML mal formado.
//! - Con `strict_variables`, leer un dato de un objeto nulo es un error; si
//!   no, se imprime vacío.
//!
//! # Ejemplo
//!
//! ```
//! use cpeforge_core::model::company::Company;
//! use cpeforge_core::model::sale::{BaseSale, Invoice};
//! use cpeforge_xml::{InvoiceBuilder, RenderOptions};
//!
//! let invoice = Invoice {
//!     base: BaseSale {
//!         ubl_version: Some("2.1".into()),
//!         serie: Some("F001".into()),
//!         correlativo: Some("1".into()),
//!         fecha_emision: Some("2024-01-31T23:30:00-05:00[America/Lima]".parse().unwrap()),
//!         company: Some(Company { ruc: Some("20000000001".into()), ..Company::default() }),
//!         ..BaseSale::default()
//!     },
//!     ..Invoice::default()
//! };
//! let xml = InvoiceBuilder::new(RenderOptions::default()).render(&invoice).unwrap();
//! assert!(xml.starts_with(r#"<?xml version="1.0" encoding="utf-8"?><Invoice "#));
//! assert!(xml.contains("<cbc:ID>F001-1</cbc:ID><cbc:IssueDate>2024-01-31</cbc:IssueDate>"));
//! ```

mod builder;
mod error;
pub mod format;
mod options;
mod render;
mod templates;

pub use builder::{
    DespatchBuilder, InvoiceBuilder, NoteBuilder, PerceptionBuilder, RetentionBuilder,
    SettlementBuilder, SummaryBuilder, VoidedBuilder,
};
pub use error::XmlError;
pub use options::{Autoescape, RenderOptions};
