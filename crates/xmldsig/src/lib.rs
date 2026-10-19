//! Firma y verificación XML (XMLDSig), canonicalización C14N, claves, X.509 y
//! cifrado XML, con el perfil de firma SUNAT ([`sunat::SignedXml`]).
//!
//! Los antecedentes y las decisiones de compatibilidad están en el README.

pub mod c14n;
pub mod dom;
pub mod dsig;
pub mod enc;
mod encoding;
mod error;
pub mod key;
pub mod sunat;
pub mod trust;
pub mod x509;
mod x509_ext;
pub mod xpath;
mod xpath_eval;

pub use error::{Error, XmlError};
