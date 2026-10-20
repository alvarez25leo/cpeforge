//! Fachadas de envío a SUNAT: `See` usa SOAP y `Api` la API GRE.
//! El XML, firma, red, ZIP, CDR y tokens se
//! implementan en los crates `cpeforge-*`.

pub mod api;
/// Errores de la orquestación.
pub mod error;
pub mod factory;
pub mod options;
pub mod resolver;
pub mod see;

pub use api::Api;
/// Importe en letras para la leyenda 1000 (ver [`cpeforge_core::amount_words`]).
pub use cpeforge_core::amount_words;
pub use error::CpeForgeError;
pub use factory::{FeFactory, SendResult, Sender, XmlSigner};
pub use options::BuilderOptionsPatch;
pub use resolver::{DocumentKind, DocumentRef, SenderKind, XmlBuilderResolver, sender_kind};
pub use see::ErrorCodeProvider;
pub use see::See;
