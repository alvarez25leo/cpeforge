//! Representación impresa HTML de comprobantes electrónicos peruanos.
//!
//! - [`HtmlReport`] implementa [`cpeforge_core::ports::Report`] con siete
//!   plantillas integradas (`invoice`, `invoice2`, `despatch`,
//!   `perception`, `retention`, `summary`, `voided`) y su CSS. La plantilla
//!   inicial es siempre `invoice.html.twig`; se cambia con
//!   [`HtmlReport::set_template`]. Una carpeta del usuario
//!   ([`ReportOptions::template_dir`]) tiene prioridad sobre las integradas.
//! - [`DefaultTemplateResolver`] elige la plantilla por tipo de documento.
//! - [`qr`] genera el SVG del código QR de ventas, guías y URL.
//! - [`format`](mod@format) convierte importes y fechas en el texto que
//!   muestran las plantillas.
//! - [`xml_utils`] lee el `DigestValue` de un XML firmado.
//!
//! **No** genera PDF, no firma y no envía a SUNAT.
//!
//! # Motor de plantillas
//!
//! Las plantillas se ejecutan con [MiniJinja](https://docs.rs/minijinja).
//! Las integradas usan tests y filtros propios para las comparaciones laxas,
//! la verdad de `"0"` y la concatenación de valores nulos (ver el README
//! del crate). Las plantillas propias se escriben en la sintaxis
//! de MiniJinja con los filtros, funciones y tests documentados allí.
//!
//! # Seguridad
//!
//! - `user.header` y `user.footer` se imprimen sin escapar: son
//!   [`TrustedHtml`] y el llamador decide que son seguros.
//! - Los demás textos se escapan (`&`, `<`, `>`, `"` y `'`).
//! - Las plantillas del usuario no pueden salir de su carpeta (`..`, rutas
//!   absolutas o con unidad, enlaces simbólicos), deben ser archivos
//!   regulares y tienen tamaño máximo; el logo también.
//!
//! # Concurrencia
//!
//! [`HtmlReport`] es `Send + Sync` y `render_html` toma `&self`: un mismo
//! motor se comparte entre hilos (por ejemplo con `Arc`) y reutiliza las
//! plantillas ya compiladas. La caché del motor solo guarda las plantillas
//! que se cargan, acotadas por las integradas y los archivos de
//! [`ReportOptions::template_dir`]; cambiar un archivo exige crear otro
//! [`HtmlReport`].
//!
//! # Ejemplo
//!
//! ```
//! use cpeforge_core::model::company::Company;
//! use cpeforge_core::model::sale::{BaseSale, Invoice};
//! use cpeforge_core::model::client::Client;
//! use cpeforge_core::Decimal;
//! use cpeforge_report::{HtmlReport, ReportParameters, TrustedHtml};
//!
//! let invoice = Invoice {
//!     base: BaseSale {
//!         tipo_doc: Some("01".into()),
//!         serie: Some("F001".into()),
//!         correlativo: Some("1".into()),
//!         fecha_emision: Some("2024-03-15T10:00:00-05:00[America/Lima]".parse().unwrap()),
//!         company: Some(Company { ruc: Some("20000000001".into()), ..Company::default() }),
//!         client: Some(Client { tipo_doc: Some("6".into()), num_doc: Some("20000000002".into()), ..Client::default() }),
//!         mto_igv: Some(Decimal::new(1800, 2)),
//!         mto_imp_venta: Some(Decimal::new(11800, 2)),
//!         ..BaseSale::default()
//!     },
//!     ..Invoice::default()
//! };
//! let mut params = ReportParameters::default();
//! params.system.logo = Some(b"\x89PNG\r\n\x1a\n".to_vec());
//! params.user.header = Some(TrustedHtml::new("Telf: <b>01 0000000</b>"));
//! let html = HtmlReport::default().render_html(&invoice, &params).unwrap();
//! assert!(html.contains("FACTURA"));
//! assert!(html.contains("data:image/svg+xml;base64,"));
//! ```
//!
//! Sin `mtoIGV` la plantilla de factura falla con
//! [`RenderCause::TypeError`]: `n_format` no acepta un importe nulo.

mod catalog;
mod context;
mod error;
mod filters;
pub mod format;
mod html;
mod params;
pub mod qr;
mod resolver;
mod templates;
mod validate;
pub mod xml_utils;

pub use error::{RenderCause, ReportError};

pub use filters::{image_data_uri, mime_type};
pub use html::{DEFAULT_TEMPLATE, HtmlReport, ReportDocument, ReportOptions};
pub use params::{
    Extra, ParamValue, ReportParameters, SystemParameters, TrustedHtml, UserParameters,
};
pub use resolver::{DefaultTemplateResolver, TemplateResolver};
pub use templates::BUILTIN_TEMPLATES;
pub use xml_utils::{XmlUtils, XmlUtilsError};

/// Valor del filtro `catalog`: texto de `value` en el catálogo fijo
/// `catalog` (`01`, `02`, `021`, `06`, `18`, `19`, `20`, `54`, `59`), o cadena
/// vacía si el catálogo o el código no existen.
#[must_use]
pub fn catalog_value(value: &str, catalog: &str) -> &'static str {
    catalog::lookup(value, catalog)
}
