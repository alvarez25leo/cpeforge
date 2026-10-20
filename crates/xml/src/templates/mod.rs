//! Una función de render por plantilla de referencia (`*.xml.twig`).
//!
//! El marcado literal se copia de las plantillas sin la indentación: el
//! `spaceless` final elimina todo espacio entre etiquetas. No se modifica la
//! estructura de ninguna plantilla. `settlement` (liquidación de compra) es
//! propia de CPEForge y no tiene plantilla de referencia.

pub(crate) mod despatch;
pub(crate) mod invoice20;
pub(crate) mod invoice21;
pub(crate) mod note;
pub(crate) mod retention;
pub(crate) mod settlement;
pub(crate) mod shared;
pub(crate) mod summary;
