//! Resultados de servicios y CDR.

use crate::response_code::is_accepted_code;

/// Resultado común de un envío o consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct BaseResult {
    /// Éxito de la operación; por defecto `Some(false)` (getter `isSuccess`).
    pub success: Option<bool>,
    /// Error devuelto por el servicio.
    pub error: Option<ResponseError>,
}

impl Default for BaseResult {
    fn default() -> Self {
        Self {
            success: Some(false),
            error: None,
        }
    }
}

/// Resultado de un envío síncrono con CDR.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
pub struct BillResult {
    /// Campos heredados de `BaseResult`.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub base: BaseResult,
    /// ZIP del CDR tal como lo devolvió el servicio (binario).
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_util::base64_option"))]
    pub cdr_zip: Option<Vec<u8>>,
    /// CDR interpretado.
    pub cdr_response: Option<CdrResponse>,
}

/// Resultado de un envío asíncrono (ticket).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
pub struct SummaryResult {
    /// Campos heredados de `BaseResult`.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub base: BaseResult,
    /// Ticket de seguimiento.
    pub ticket: Option<String>,
}

/// Estado de un ticket.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
pub struct StatusResult {
    /// Campos heredados de `BillResult`.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub bill: BillResult,
    /// StatusCode de SUNAT: `0` procesó, `98` en proceso, `99` con errores.
    pub code: Option<String>,
}

/// Resultado de la consulta de estado o CDR de un comprobante.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "camelCase"))]
pub struct StatusCdrResult {
    /// Campos heredados de `BillResult`.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub bill: BillResult,
    /// Código de estado.
    pub code: Option<String>,
    /// Mensaje de estado.
    pub message: Option<String>,
}

/// Constancia de recepción (CDR) interpretada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct CdrResponse {
    /// Identificador del comprobante.
    pub id: Option<String>,
    /// Código de respuesta.
    pub code: Option<String>,
    /// Descripción de la respuesta.
    pub description: Option<String>,
    /// Observaciones.
    pub notes: Option<Vec<String>>,
    /// Referencia del documento.
    pub reference: Option<String>,
}

/// Error de un servicio (clase `Error`). Por defecto código y mensaje son cadenas vacías, como el constructor de la referencia.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ResponseError {
    /// Código de error.
    pub code: Option<String>,
    /// Mensaje de error.
    pub message: Option<String>,
}

impl std::fmt::Debug for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponseError")
            .field("code", &self.code)
            .field("message", &"<oculto>")
            .finish()
    }
}

impl Default for ResponseError {
    fn default() -> Self {
        Self {
            code: Some("".to_owned()),
            message: Some("".to_owned()),
        }
    }
}

impl ResponseError {
    /// Error con código y mensaje (`new Error($code, $message)`).
    #[must_use]
    pub fn new(code: Option<String>, message: Option<String>) -> Self {
        Self { code, message }
    }

    /// Error solo con código; el mensaje queda vacío (`new Error($code)`).
    #[must_use]
    pub fn with_code(code: Option<String>) -> Self {
        Self {
            code,
            message: Some(String::new()),
        }
    }
}

impl CdrResponse {
    /// `true` si el código es `0` o `>= 4000` (aceptado con observaciones).
    ///
    /// Reproduce la conversión `(int)` de la referencia: un código ausente o no
    /// numérico vale `0` y se considera aceptado. Ver
    /// [`crate::response_code::int_cast`].
    ///
    /// **No usar como decisión tributaria**: un CDR sin código o con un
    /// código ilegible devuelve `true`. Para decidir si un comprobante quedó
    /// aceptado, rechazado o sin conclusión, usar la clasificación estricta
    /// `TaxStatus` de `cpeforge-ws`, que trata esos casos como
    /// indeterminados.
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        is_accepted_code(self.code.as_deref())
    }
}

/// Acceso común a los resultados de servicios, equivalente al tipo base de la referencia
/// `BaseResult` que devuelven `SenderInterface` y `FactoryInterface`.
pub trait ServiceResult {
    /// Parte común del resultado.
    fn base_result(&self) -> &BaseResult;

    /// `isSuccess()`.
    fn success(&self) -> Option<bool> {
        self.base_result().success
    }

    /// `getError()`.
    fn error(&self) -> Option<&ResponseError> {
        self.base_result().error.as_ref()
    }
}

impl ServiceResult for BaseResult {
    fn base_result(&self) -> &BaseResult {
        self
    }
}

impl ServiceResult for BillResult {
    fn base_result(&self) -> &BaseResult {
        &self.base
    }
}

impl ServiceResult for SummaryResult {
    fn base_result(&self) -> &BaseResult {
        &self.base
    }
}

impl ServiceResult for StatusResult {
    fn base_result(&self) -> &BaseResult {
        &self.bill.base
    }
}

impl ServiceResult for StatusCdrResult {
    fn base_result(&self) -> &BaseResult {
        &self.bill.base
    }
}
