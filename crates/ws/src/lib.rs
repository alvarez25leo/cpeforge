//! Transporte SOAP, ZIP, lectura de CDR y adaptador GRE para los servicios
//! de comprobantes electrónicos de SUNAT.
//!
//! - [`services`]: envío (`sendBill`, `sendSummary`) y consultas SOAP.
//! - [`api`]: caché de tokens OAuth2 y envío/consulta de guías por la API GRE.
//! - [`zip`] y [`cdr`]: empaquetado del XML y lectura del CDR, con límites de
//!   tamaño, profundidad y nombres seguros.
//! - [`outcome`]: estado tributario de una respuesta, separado del éxito de
//!   transporte.
//!
//! Las diferencias deliberadas frente a la implementación de referencia
//! (TLS siempre verificado, DTD rechazado, rutas de ZIP validadas, límites de
//! anidamiento) están registradas en `tests/reference/ws/differences.json`.

pub mod api;
pub mod cdr;
pub mod error;
pub mod outcome;
mod packet;
pub mod services;
pub mod soap;
pub mod zip;

pub use error::WsError;
