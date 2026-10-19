//! Cliente HTTP de la API de la nueva plataforma GRE de SUNAT (documento
//! OpenAPI `1.0.0`).
//!
//! Tres operaciones:
//!
//! - [`AuthApi::get_token`]: `POST /clientessol/{client_id}/oauth2/token/`;
//! - [`CpeApi::enviar_cpe`]: `POST /contribuyente/gem/comprobantes/{filename}`;
//! - [`CpeApi::consultar_envio`]: `GET /contribuyente/gem/comprobantes/envios/{numTicket}`.
//!
//! El transporte es inyectable ([`HttpTransport`]); con la característica
//! `reqwest` se incluye [`ReqwestTransport`] (TLS verificado, redirecciones
//! solo dentro del mismo origen y tiempos límite). Este crate **no** comprime ZIP, no
//! guarda ni renueva tokens y no interpreta el CDR: eso corresponde a
//! `cpeforge-ws` y `cpeforge`.
//!
//! ```no_run
//! # #[cfg(feature = "reqwest")]
//! # async fn ejemplo() -> Result<(), Box<dyn std::error::Error>> {
//! use cpeforge_gre_api::{AuthApi, Configuration, CpeApi, ReqwestTransport, TokenRequest};
//!
//! let transport = ReqwestTransport::new()?;
//! let auth = AuthApi::new(transport.clone(), Configuration::auth());
//! let token = auth
//!     .get_token(&TokenRequest::password_grant("client-id", "client-secret", "20000000001USUARIO", "clave"))
//!     .await?;
//! let cpe = CpeApi::new(transport, Configuration::cpe(token.access_token.unwrap_or_default()));
//! let estado = cpe.consultar_envio("ticket").await?;
//! println!("{:?}", estado.cod_respuesta);
//! # Ok(())
//! # }
//! ```

mod api;
mod codec;
pub mod config;
mod datetime;
mod error;
pub mod model;
mod secret;
mod transport;

pub use api::{ApiResponse, AuthApi, CpeApi, TokenRequest};
pub use config::Configuration;
pub use datetime::{DateTimeError, DateTimeValue, ParsedDateTime};
pub use error::{ApiError, ErrorObject, Headers};
pub use model::{
    ApiToken, CpeDocument, CpeDocumentArchivo, CpeError, CpeErrorValidation, CpeResponse, Model,
    StatusResponse, StatusResponseError,
};
pub use secret::Secret;
pub use transport::{
    HttpRequest, HttpResponse, HttpTransport, Method, TransportError, TransportErrorKind,
};
#[cfg(feature = "reqwest")]
pub use transport::{ReqwestTransport, ReqwestTransportBuilder};
