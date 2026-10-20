//! Modelos y contratos de comprobantes electrónicos peruanos.
//!
//! Los antecedentes y la versión de referencia están en el README. Contiene:
//!
//! - modelos de venta, liquidación de compra, guías, resúmenes, bajas,
//!   retenciones, percepciones, identidades y respuestas ([`model`]);
//! - el nombre de archivo de cada documento ([`Document::name`]) y el
//!   identificador de resúmenes y bajas con la fecha de Perú
//!   ([`model::summary::Summary::xml_id`]);
//! - los contratos que implementarán otros crates ([`ports`], [`service`]);
//! - el importe en letras de la leyenda 1000 ([`amount_words`]).
//!
//! **No** genera XML, no firma, no comprime ni se comunica con SUNAT: esas
//! responsabilidades pertenecen a crates posteriores.
//!
//! # Contrato de los modelos
//!
//! - Cada propiedad de la referencia es un campo público en `snake_case`. Con
//!   la feature `serde`, el nombre serializado es exactamente el de esa
//!   propiedad (`mtoIGV`, `numDocfectado`, …), incluidas sus erratas.
//! - Los campos son `Option<T>`: «no establecido» y `null` son
//!   indistinguibles y ambos se representan con `None`. Una cadena vacía, un
//!   cero o una lista vacía son `Some(..)` y se conservan.
//! - Los valores por defecto (`ublVersion = "2.0"`, `codigoPais = "PE"`,
//!   `factorIcbper = 0.3`, …) los aplica [`Default`].
//! - Los importes y cantidades usan [`Decimal`] en lugar de `float`; se
//!   conservan signo y escala sin pérdida.
//! - Las fechas usan [`Zoned`] (instante + zona horaria IANA o desfase fijo).
//! - La herencia se expresa por composición: por ejemplo
//!   [`model::sale::Invoice`] contiene un [`model::sale::BaseSale`].
//!
//! # Feature `serde`
//!
//! Serializa los modelos con los nombres de propiedad de la referencia:
//!
//! - importes como texto exacto (`"18.50"`); al leer se acepta texto decimal o
//!   científico y enteros JSON, y se rechazan números JSON con decimales para
//!   no perder precisión;
//! - fechas como RFC 9557 (`2024-01-31T23:30:00-05:00[America/Lima]`); al leer
//!   se acepta también RFC 3339 con desfase (`2024-01-31T23:30:00-05:00`);
//! - contenido binario (`cdrZip`) como Base64;
//! - un campo ausente toma el valor por defecto del modelo y un `null`
//!   explícito lo deja en `None`.
//!
//! Las claves desconocidas se rechazan, salvo en los modelos compuestos
//! ([`model::sale::Invoice`], [`model::sale::Note`] y los resultados que
//! contienen [`model::response::BaseResult`]): `#[serde(flatten)]` no permite
//! rechazarlas.
//!
//! # Ejemplo
//!
//! ```
//! use cpeforge_core::model::company::Company;
//! use cpeforge_core::model::sale::{BaseSale, Invoice};
//! use cpeforge_core::Document;
//!
//! let invoice = Invoice {
//!     base: BaseSale {
//!         tipo_doc: Some("01".into()),
//!         serie: Some("F001".into()),
//!         correlativo: Some("123".into()),
//!         company: Some(Company { ruc: Some("20000000001".into()), ..Company::default() }),
//!         ..BaseSale::default()
//!     },
//!     ..Invoice::default()
//! };
//! assert_eq!(invoice.name().unwrap(), "20000000001-01-F001-123");
//! ```

pub mod amount_words;
mod document;
mod error;
pub mod model;
pub mod ports;
pub mod response_code;
#[cfg(feature = "serde")]
mod serde_util;
pub mod service;
pub mod time;

pub use document::Document;
pub use error::CoreError;
pub use jiff::Zoned;
pub use rust_decimal::Decimal;
