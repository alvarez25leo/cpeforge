//! Configuración del cliente.
//!
//! **Hosts.** El `openapi.yaml` fijado declara los servidores
//! [`OPENAPI_AUTH_SERVER`] y [`OPENAPI_CPE_SERVER`] (`https://api.sunat.gob.pe/v1`),
//! pero el envío y la consulta usan [`CPE_HOST`]
//! (`https://api-cpe.sunat.gob.pe/v1`), el segundo host de
//! [`HOST_SETTINGS`] y el que usa el cliente de referencia. El crate
//! conserva ambas constantes.
//!
//! **Host por defecto.** [`Configuration::default`] apunta a [`AUTH_HOST`]
//! incluso para [`crate::CpeApi`]: el host no se elige por operación, cada
//! cliente usa siempre el de su configuración. Para el envío use
//! [`Configuration::cpe`] o [`Configuration::with_host`].

use std::fmt;

use crate::secret::Secret;

/// Host de autenticación (`HOST_SETTINGS[0]` y primer `server` del OpenAPI).
pub const AUTH_HOST: &str = "https://api-seguridad.sunat.gob.pe/v1";
/// Host de envío y consulta (`HOST_SETTINGS[1]`).
pub const CPE_HOST: &str = "https://api-cpe.sunat.gob.pe/v1";
/// Primer `server` del `openapi.yaml` fijado (igual a [`AUTH_HOST`]).
pub const OPENAPI_AUTH_SERVER: &str = "https://api-seguridad.sunat.gob.pe/v1";
/// Segundo `server` del `openapi.yaml` fijado; **distinto** de [`CPE_HOST`].
pub const OPENAPI_CPE_SERVER: &str = "https://api.sunat.gob.pe/v1";
/// `User-Agent` por defecto. Es un valor del contrato de interoperabilidad:
/// identifica las solicitudes igual que el cliente de referencia.
pub const DEFAULT_USER_AGENT: &str = "CPEForge/0.1.0";
/// Versión del **documento** OpenAPI (`info.version`).
pub const OPENAPI_DOCUMENT_VERSION: &str = "1.0.0";
/// Versión del cliente de referencia (la referencia) cuyo
/// comportamiento sigue este crate.
pub const REFERENCE_PACKAGE_VERSION: &str = "v1.0.2";
/// `scope` por defecto del OpenAPI (el que usa `cpeforge-ws`).
pub const DEFAULT_SCOPE: &str = "https://api-cpe.sunat.gob.pe";
/// `grant_type` por defecto del OpenAPI.
pub const DEFAULT_GRANT_TYPE: &str = "password";

/// Host conocido y su descripción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct HostSetting {
    /// URL base.
    pub url: &'static str,
    /// Descripción.
    pub description: &'static str,
}

/// Hosts conocidos, por índice: autenticación y envío/consulta.
pub const HOST_SETTINGS: [HostSetting; 2] = [
    HostSetting {
        url: AUTH_HOST,
        description: "Url para obtener el token de autenticacion",
    },
    HostSetting {
        url: CPE_HOST,
        description: "Url para enviar comprobantes y consultar su estado",
    },
];

/// Índice fuera de rango en [`host_from_settings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidHostIndex(pub i64);

impl fmt::Display for InvalidHostIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid index {} when selecting the host. Must be less than {}",
            self.0,
            HOST_SETTINGS.len()
        )
    }
}

impl std::error::Error for InvalidHostIndex {}

/// URL del host `index` de [`HOST_SETTINGS`].
///
/// # Errors
///
/// [`InvalidHostIndex`] si el índice es negativo o no existe.
pub fn host_from_settings(index: i64) -> Result<&'static str, InvalidHostIndex> {
    usize::try_from(index)
        .ok()
        .and_then(|i| HOST_SETTINGS.get(i))
        .map(|h| h.url)
        .ok_or(InvalidHostIndex(index))
}

/// Configuración de un cliente.
#[derive(Clone, PartialEq, Eq)]
pub struct Configuration {
    host: String,
    access_token: Option<Secret>,
    user_agent: String,
}

impl Default for Configuration {
    /// Host de autenticación, sin token y `User-Agent`
    /// [`DEFAULT_USER_AGENT`].
    fn default() -> Self {
        Self {
            host: AUTH_HOST.to_owned(),
            access_token: None,
            user_agent: DEFAULT_USER_AGENT.to_owned(),
        }
    }
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Configuration")
            .field("host", &"<oculto: puede incluir credenciales>")
            .field("access_token", &self.access_token)
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl Configuration {
    /// Configuración para [`crate::AuthApi`] ([`AUTH_HOST`]).
    #[must_use]
    pub fn auth() -> Self {
        Self::default()
    }

    /// Configuración para [`crate::CpeApi`] ([`CPE_HOST`]) con el token.
    #[must_use]
    pub fn cpe(access_token: impl Into<String>) -> Self {
        Self::default()
            .with_host(CPE_HOST)
            .with_access_token(access_token)
    }

    /// Sustituye la URL base (sin `/` final; se concatena con la ruta).
    #[must_use]
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Fija el token de acceso que se envía como `Bearer`.
    #[must_use]
    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(Secret::new(token));
        self
    }

    /// Sustituye el `User-Agent`; una cadena vacía omite la cabecera.
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// URL base.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Token de acceso, si se fijó.
    #[must_use]
    pub fn access_token(&self) -> Option<&Secret> {
        self.access_token.as_ref()
    }

    /// `User-Agent` configurado.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Token que se envía: se omite si es `""` o `"0"`.
    pub(crate) fn bearer(&self) -> Option<&str> {
        self.access_token
            .as_ref()
            .map(Secret::expose)
            .filter(|t| !t.is_empty() && *t != "0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts() {
        assert_eq!(Configuration::default().host(), AUTH_HOST);
        assert_eq!(host_from_settings(1), Ok(CPE_HOST));
        assert_ne!(CPE_HOST, OPENAPI_CPE_SERVER);
        assert_eq!(
            host_from_settings(2).unwrap_err().to_string(),
            "Invalid index 2 when selecting the host. Must be less than 2"
        );
        let c = Configuration::cpe("valor-secreto-123");
        assert!(!format!("{c:?}").contains("valor-secreto-123"));
    }
}
