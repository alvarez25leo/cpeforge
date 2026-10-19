//! Contratos de envío y autenticación.

use std::error::Error as StdError;
use std::fmt;

use crate::Zoned;
use crate::model::response::ServiceResult;

/// Envía un archivo a un servicio.
pub trait Sender {
    /// Resultado del envío.
    type Output: ServiceResult;
    /// Error del transporte.
    type Error;

    /// Envía `content` con el nombre `filename` (`send()`). La referencia admite `null`
    /// en ambos parámetros. El contenido son bytes porque una cadena puede
    /// contener datos binarios.
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn send(
        &self,
        filename: Option<&str>,
        content: Option<&[u8]>,
    ) -> Result<Option<Self::Output>, Self::Error>;
}

/// Token de acceso con su expiración.
#[derive(Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct BasicToken {
    /// Valor del token.
    pub value: Option<String>,
    /// Fecha de expiración.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::zoned_option"))]
    pub expire: Option<Zoned>,
}

impl fmt::Debug for BasicToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BasicToken(<valor y vencimiento ocultos>)")
    }
}

impl BasicToken {
    /// Token con valor y expiración (`new BasicToken($value, $expire)`).
    #[must_use]
    pub fn new(value: Option<String>, expire: Option<Zoned>) -> Self {
        Self { value, expire }
    }
}

/// Almacén de tokens por identificador.
///
/// Los métodos reciben `&self` para permitir almacenes compartidos; una
/// implementación en memoria usa mutabilidad interior.
pub trait TokenStore {
    /// Error del almacén.
    type Error;

    /// Token guardado para `id` (`get()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn get(&self, id: Option<&str>) -> Result<Option<BasicToken>, Self::Error>;

    /// Guarda el token de `id` (`set()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn set(&self, id: Option<&str>, token: BasicToken) -> Result<(), Self::Error>;
}

/// Respuesta de servicio con formato inesperado.
///
/// Conserva mensaje, código y causa. Por defecto el
/// mensaje es vacío y el código `0`.
#[derive(Debug, Default)]
pub struct InvalidServiceResponse {
    message: String,
    code: i64,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl InvalidServiceResponse {
    /// Error con mensaje y código (`new InvalidServiceResponseException($message, $code)`).
    #[must_use]
    pub fn new(message: impl Into<String>, code: i64) -> Self {
        Self {
            message: message.into(),
            code,
            source: None,
        }
    }

    /// Añade la causa (`$previous`).
    #[must_use]
    pub fn with_source(
        mut self,
        source: impl Into<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Mensaje (`getMessage()`).
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Código (`getCode()`).
    #[must_use]
    pub fn code(&self) -> i64 {
        self.code
    }
}

impl fmt::Display for InvalidServiceResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for InvalidServiceResponse {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}
