//! Clientes `AuthApi` y `CpeApi`.
//!
//! Cada operación se ofrece en varias formas (ejemplo con el token):
//!
//! | Forma | Resultado |
//! | --- | --- |
//! | [`AuthApi::get_token`] | el modelo |
//! | [`AuthApi::get_token_with_http_info`] | el modelo, el estado y las cabeceras |
//! | [`AuthApi::get_token_async`] | el modelo, con errores genéricos |
//! | [`AuthApi::get_token_async_with_http_info`] | el modelo, el estado y las cabeceras, con errores genéricos |
//! | [`AuthApi::get_token_request`] | la solicitud, sin enviarla |
//!
//! Todas las funciones de envío son asíncronas; el sufijo `async` no indica
//! eso, sino el flujo de errores. Sin sufijo, un 4xx/5xx lleva en el mensaje
//! el método, la URI, el estado y un resumen del cuerpo, y los 422/500 de
//! `CpeApi` traen el cuerpo tipado; un fallo de red es
//! [`ApiError::Transport`]. Con sufijo `async`, todo estado fuera de 2xx usa
//! el mensaje genérico `Error connecting to the API`, el cuerpo de error no
//! se lee y un fallo de red se informa como error de clase `Error`.

use std::fmt::Write as _;

use crate::codec::{self, DecodeError};
use crate::config::{Configuration, DEFAULT_GRANT_TYPE, DEFAULT_SCOPE};
use crate::error::{ApiError, ErrorObject, Headers};
use crate::model::{
    ApiToken, CpeDocument, CpeError, CpeErrorValidation, CpeResponse, Model, StatusResponse,
};
use crate::secret::Secret;
use crate::transport::{HttpRequest, HttpResponse, HttpTransport, Method};

const FORM: &str = "application/x-www-form-urlencoded";
const JSON: &str = "application/json";

/// Respuesta con metadatos HTTP (formas `*_with_http_info`).
#[derive(Clone, PartialEq, Eq)]
pub struct ApiResponse<T> {
    /// Cuerpo deserializado; `None` si el cuerpo no es JSON o es `null`.
    pub data: Option<T>,
    /// Código de estado.
    pub status: u16,
    /// Cabeceras.
    pub headers: Headers,
}

impl<T> std::fmt::Debug for ApiResponse<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiResponse")
            .field("status", &self.status)
            .field("data", &"<oculto>")
            .field("headers", &"<ocultas>")
            .finish()
    }
}

/// Parámetros de la solicitud de token (campos del formulario y
/// `client_id` de la ruta).
#[derive(Clone, PartialEq, Eq)]
pub struct TokenRequest {
    /// `grant_type`.
    pub grant_type: String,
    /// `scope`.
    pub scope: String,
    /// `client_id` (también en la ruta).
    pub client_id: String,
    /// `client_secret`.
    pub client_secret: Secret,
    /// `username`: RUC + usuario SOL.
    pub username: String,
    /// `password`: clave SOL.
    pub password: Secret,
}

impl std::fmt::Debug for TokenRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenRequest")
            .field("grant_type", &self.grant_type)
            .field("scope", &self.scope)
            .field("client_id", &"<oculto>")
            .field("client_secret", &self.client_secret)
            .field("username", &"<oculto>")
            .field("password", &self.password)
            .finish()
    }
}

impl TokenRequest {
    /// Solicitud con `grant_type=password` y el `scope` del OpenAPI
    /// (`https://api-cpe.sunat.gob.pe`), los valores que usa `cpeforge-ws`.
    #[must_use]
    pub fn password_grant(
        client_id: impl Into<String>,
        client_secret: impl Into<Secret>,
        username: impl Into<String>,
        password: impl Into<Secret>,
    ) -> Self {
        Self {
            grant_type: DEFAULT_GRANT_TYPE.to_owned(),
            scope: DEFAULT_SCOPE.to_owned(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            username: username.into(),
            password: password.into(),
        }
    }
}

/// Cabeceras de una solicitud, siempre en este orden: `User-Agent` (si no
/// está vacío), `Accept`, `Content-Type` (JSON si llega vacío) y, si
/// `bearer`, `Authorization` cuando hay token.
fn base_headers(config: &Configuration, content_type: &str, bearer: bool) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if !config.user_agent().is_empty() {
        headers.push(("User-Agent".to_owned(), config.user_agent().to_owned()));
    }
    headers.push(("Accept".to_owned(), JSON.to_owned()));
    let ct = if content_type.is_empty() {
        JSON
    } else {
        content_type
    };
    headers.push(("Content-Type".to_owned(), ct.to_owned()));
    if bearer {
        if let Some(token) = config.bearer() {
            headers.push(("Authorization".to_owned(), format!("Bearer {token}")));
        }
    }
    headers
}

fn is_json(content_type: &str) -> bool {
    let ct = if content_type.is_empty() {
        JSON
    } else {
        content_type
    };
    ct.to_ascii_lowercase().contains(JSON)
}

/// Cabeceras agrupadas por nombre (sin distinguir mayúsculas), en orden.
fn group_headers(raw: &[(String, String)]) -> Headers {
    let mut out: Headers = Vec::new();
    for (k, v) in raw {
        match out
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(k))
        {
            Some((_, values)) => values.push(v.clone()),
            None => out.push((k.clone(), vec![v.clone()])),
        }
    }
    out
}

/// Caracteres no imprimibles según la clase `[^\pL\pM\pN\pP\pS\pZ\n\r\t]`:
/// controles (Cc), formato (Cf, los más comunes), sustitutos y de uso
/// privado (Co).
fn non_printable(c: char) -> bool {
    if matches!(c, '\n' | '\r' | '\t') {
        return false;
    }
    let u = c as u32;
    c.is_control()
        || matches!(u, 0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x180E | 0x200B..=0x200F | 0x202A..=0x202E
            | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F)
        || matches!(u, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x10_0000..=0x10_FFFD)
}

/// Resumen del cuerpo que acompaña al mensaje de un 4xx/5xx: primeros 120
/// bytes; si se trunca, quita un carácter UTF-8 final
/// incompleto y añade ` (truncated...)`; sin resumen si el cuerpo está
/// vacío, no es UTF-8 válido o tiene caracteres no imprimibles.
fn body_summary(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    let mut cut = body[..body.len().min(120)].to_vec();
    let truncated = body.len() > 120;
    if truncated && std::str::from_utf8(&cut).is_err() {
        let lookahead = &body[120..body.len().min(123)];
        cut = trim_incomplete_utf8(cut, lookahead);
    }
    let mut summary = String::from_utf8(cut).ok()?;
    if truncated {
        summary.push_str(" (truncated...)");
    }
    if summary.chars().any(non_printable) {
        return None;
    }
    Some(summary)
}

/// Quita del final de `summary` un carácter UTF-8 cortado, solo si los bytes
/// siguientes del cuerpo (`lookahead`) lo completan de forma válida.
fn trim_incomplete_utf8(summary: Vec<u8>, lookahead: &[u8]) -> Vec<u8> {
    let Some(start) = summary.iter().rposition(|b| !(0x80..=0xBF).contains(b)) else {
        return summary;
    };
    let expected = match summary[start] {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return summary,
    };
    let available = summary.len() - start;
    if available >= expected {
        return summary;
    }
    let mut sequence = summary[start..].to_vec();
    sequence.extend(lookahead.iter().take(expected - available));
    if sequence.len() != expected || std::str::from_utf8(&sequence).is_err() {
        return summary;
    }
    summary[..start].to_vec()
}

/// URI tal como aparece en los mensajes de error: esquema y host en
/// minúsculas, sin puerto por defecto, contraseña de `userinfo` redactada y caracteres
/// no permitidos de la ruta codificados.
pub(crate) fn display_uri(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    let scheme = scheme.to_ascii_lowercase();
    let split = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(split);
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, authority),
    };
    let mut host = hostport.to_ascii_lowercase();
    let default_port = match scheme.as_str() {
        "http" => Some(":80"),
        "https" => Some(":443"),
        _ => None,
    };
    if let Some(p) = default_port {
        if let Some(h) = host.strip_suffix(p) {
            host = h.to_owned();
        }
    }
    let user = match userinfo {
        Some(u) => match u.split_once(':') {
            Some((name, _)) => format!("{name}:***@"),
            None => format!("{u}@"),
        },
        None => String::new(),
    };
    let mut encoded = String::new();
    for b in tail.bytes() {
        if b.is_ascii_alphanumeric() || b"_-.~!$&'()*+,;=%:@/?#".contains(&b) {
            encoded.push(char::from(b));
        } else {
            let _ = write!(encoded, "%{b:02X}");
        }
    }
    format!("{scheme}://{user}{host}{encoded}")
}

/// Mensaje de un estado 4xx/5xx: `Client error` o `Server error`, método,
/// URI, estado y razón, y el resumen del cuerpo en una línea aparte.
fn http_error_message(request: &HttpRequest, response: &HttpResponse) -> String {
    let label = match response.status / 100 {
        4 => "Client error",
        5 => "Server error",
        _ => "Unsuccessful request",
    };
    let mut message = format!(
        "{label}: `{} {}` resulted in a `{} {}` response",
        request.method.as_str(),
        display_uri(&request.url),
        response.status,
        response.reason
    );
    if let Some(summary) = body_summary(&response.body) {
        message.push_str(":\n");
        message.push_str(&summary);
        message.push('\n');
    }
    message
}

fn decode_failure(e: DecodeError) -> ApiError {
    ApiError::Decode {
        class: e.class,
        message: e.message,
    }
}

/// Tipo de cuerpo de error que `CpeApi` lee para 422 y 500.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ErrorBodies {
    None,
    Cpe,
}

/// Flujo de errores de una forma (véase el módulo).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ErrorFlow {
    /// Mensajes HTTP detallados, cuerpo de error tipado y error de transporte.
    Detailed,
    /// Mensaje genérico fuera de 2xx; un fallo de red es un error `Error`.
    Generic,
}

type ModelReader<M> =
    fn(Option<&serde_json::Map<String, serde_json::Value>>) -> Result<M, DecodeError>;

/// Envía la solicitud, convierte un estado fuera de 2xx en [`ApiError`] según
/// `flow` y lee el cuerpo de una respuesta 2xx con `read`.
async fn execute<T, M>(
    transport: &T,
    request: HttpRequest,
    error_bodies: ErrorBodies,
    flow: ErrorFlow,
    read: ModelReader<M>,
) -> Result<ApiResponse<M>, ApiError>
where
    T: HttpTransport,
{
    // `client_id`, `filename` o `numTicket` iguales a `.` o `..`.
    if crate::transport::has_dot_segment(&request.url) {
        return Err(ApiError::InvalidRequest {
            message: "parámetro de ruta «.» o «..» no admitido".to_owned(),
        });
    }
    let response = match transport.send(request.clone()).await {
        Ok(r) => r,
        Err(e) => {
            if flow == ErrorFlow::Generic {
                return Err(ApiError::Decode {
                    class: "Error",
                    message: "Call to undefined method GuzzleHttp\\Exception\\ConnectException::getResponse()".to_owned(),
                });
            }
            return Err(ApiError::Transport {
                kind: e.kind,
                message: format!("[0] {}", e.message),
            });
        }
    };
    let status = response.status;
    if !(200..=299).contains(&status) {
        let message = if flow == ErrorFlow::Generic {
            format!(
                "[{status}] Error connecting to the API ({})",
                display_uri(&request.url)
            )
        } else if status >= 400 {
            format!("[{status}] {}", http_error_message(&request, &response))
        } else {
            format!(
                "[{status}] Error connecting to the API ({})",
                display_uri(&request.url)
            )
        };
        // El cuerpo de error se decodifica una sola vez: a diferencia de una
        // respuesta 2xx, una cadena JSON no se vuelve a decodificar.
        let error_source = || codec::object_source(&codec::decode_json_or_null(&response.body));
        let object = match (error_bodies, status, flow) {
            (ErrorBodies::Cpe, 500, ErrorFlow::Detailed) => {
                let src = error_source();
                Some(ErrorObject::Server(
                    CpeError::from_source(src.as_ref()).map_err(decode_failure)?,
                ))
            }
            (ErrorBodies::Cpe, 422, ErrorFlow::Detailed) => {
                let src = error_source();
                Some(ErrorObject::Validation(
                    CpeErrorValidation::from_source(src.as_ref()).map_err(decode_failure)?,
                ))
            }
            _ => None,
        };
        return Err(ApiError::Http {
            code: status,
            message,
            headers: group_headers(&response.headers),
            body: response.body,
            object: object.map(Box::new),
        });
    }
    let content = codec::decode_json_or_null(&response.body);
    let data = if content.is_null() {
        None
    } else {
        Some(read(codec::model_source(&content).as_ref()).map_err(decode_failure)?)
    };
    Ok(ApiResponse {
        data,
        status,
        headers: group_headers(&response.headers),
    })
}

fn require<M>(response: ApiResponse<M>, method: &str, model: &str) -> Result<M, ApiError> {
    response.data.ok_or_else(|| ApiError::Decode {
        class: "TypeError",
        message: format!(
            "CPEForge\\Sunat\\GRE\\Api\\{method}(): Return value must be of type CPEForge\\Sunat\\GRE\\Model\\{model}, null returned"
        ),
    })
}

/// Cliente de autenticación (`AuthApi`).
#[derive(Debug, Clone)]
pub struct AuthApi<T> {
    transport: T,
    config: Configuration,
}

impl<T: HttpTransport> AuthApi<T> {
    /// Crea el cliente.
    pub fn new(transport: T, config: Configuration) -> Self {
        Self { transport, config }
    }

    /// Configuración del cliente.
    pub fn config(&self) -> &Configuration {
        &self.config
    }

    /// Construye sin enviarla la solicitud
    /// `POST {host}/clientessol/{client_id}/oauth2/token/` con los seis campos como formulario (o JSON si `content_type` contiene
    /// `application/json`). `None` usa `application/x-www-form-urlencoded`.
    pub fn get_token_request(
        &self,
        request: &TokenRequest,
        content_type: Option<&str>,
    ) -> HttpRequest {
        let content_type = content_type.unwrap_or(FORM);
        let path = format!(
            "/clientessol/{}/oauth2/token/",
            codec::percent_encode(&request.client_id)
        );
        let fields = [
            ("grant_type", request.grant_type.as_str()),
            ("scope", request.scope.as_str()),
            ("client_id", request.client_id.as_str()),
            ("client_secret", request.client_secret.expose()),
            ("username", request.username.as_str()),
            ("password", request.password.expose()),
        ];
        let body = if is_json(content_type) {
            codec::encode_compact_json(&codec::Json::Object(
                fields
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), codec::Json::Str((*v).to_owned())))
                    .collect(),
            ))
        } else {
            codec::build_query(&fields)
        };
        HttpRequest {
            method: Method::Post,
            url: format!("{}{path}", self.config.host()),
            headers: base_headers(&self.config, content_type, false),
            body: body.into_bytes(),
        }
    }

    /// Solicita un token y devuelve también el estado y las cabeceras.
    ///
    /// # Errors
    ///
    /// [`ApiError`] de transporte, de estado HTTP o de lectura del cuerpo.
    pub async fn get_token_with_http_info(
        &self,
        request: &TokenRequest,
        content_type: Option<&str>,
    ) -> Result<ApiResponse<ApiToken>, ApiError> {
        let req = self.get_token_request(request, content_type);
        execute(
            &self.transport,
            req,
            ErrorBodies::None,
            ErrorFlow::Detailed,
            ApiToken::from_source,
        )
        .await
    }

    /// Solicita un token.
    ///
    /// # Errors
    ///
    /// Como [`Self::get_token_with_http_info`]; además `TypeError` si la
    /// respuesta 2xx no trae un cuerpo JSON.
    pub async fn get_token(&self, request: &TokenRequest) -> Result<ApiToken, ApiError> {
        require(
            self.get_token_with_http_info(request, None).await?,
            "AuthApi::getToken",
            "ApiToken",
        )
    }

    /// Como [`Self::get_token_with_http_info`], con errores genéricos: fuera
    /// de 2xx devuelve el mensaje genérico y no lee el cuerpo de error.
    ///
    /// # Errors
    ///
    /// [`ApiError`] de transporte, estado HTTP o deserialización.
    pub async fn get_token_async_with_http_info(
        &self,
        request: &TokenRequest,
        content_type: Option<&str>,
    ) -> Result<ApiResponse<ApiToken>, ApiError> {
        let req = self.get_token_request(request, content_type);
        execute(
            &self.transport,
            req,
            ErrorBodies::None,
            ErrorFlow::Generic,
            ApiToken::from_source,
        )
        .await
    }

    /// Como [`Self::get_token`], con errores genéricos.
    ///
    /// # Errors
    ///
    /// Como [`Self::get_token_async_with_http_info`].
    pub async fn get_token_async(&self, request: &TokenRequest) -> Result<ApiToken, ApiError> {
        require(
            self.get_token_async_with_http_info(request, None).await?,
            "AuthApi::getToken",
            "ApiToken",
        )
    }
}

/// Cliente de envío y consulta (`CpeApi`). Requiere un token
/// ([`Configuration::cpe`]); sin él, o si es `""` o `"0"`, no envía
/// `Authorization`.
#[derive(Debug, Clone)]
pub struct CpeApi<T> {
    transport: T,
    config: Configuration,
}

impl<T: HttpTransport> CpeApi<T> {
    /// Crea el cliente.
    pub fn new(transport: T, config: Configuration) -> Self {
        Self { transport, config }
    }

    /// Configuración del cliente.
    pub fn config(&self) -> &Configuration {
        &self.config
    }

    /// Construye sin enviarla la solicitud
    /// `POST {host}/contribuyente/gem/comprobantes/{filename}` con el documento
    /// en JSON compacto ([`Model::to_json`]). Sin documento, el cuerpo va
    /// vacío. Con un `content_type` que no es JSON, el cuerpo es
    /// [`Model::to_json_pretty`].
    pub fn enviar_cpe_request(
        &self,
        filename: &str,
        document: Option<&CpeDocument>,
        content_type: Option<&str>,
    ) -> HttpRequest {
        let content_type = content_type.unwrap_or(JSON);
        let body = match document {
            Some(d) if is_json(content_type) => d.to_json(),
            Some(d) => d.to_json_pretty(),
            None => String::new(),
        };
        HttpRequest {
            method: Method::Post,
            url: format!(
                "{}/contribuyente/gem/comprobantes/{}",
                self.config.host(),
                codec::percent_encode(filename)
            ),
            headers: base_headers(&self.config, content_type, true),
            body: body.into_bytes(),
        }
    }

    /// Envía un comprobante y devuelve también el estado y las cabeceras.
    ///
    /// # Errors
    ///
    /// [`ApiError`]; los 422 y 500 llevan el cuerpo tipado en
    /// [`ApiError::response_object`].
    pub async fn enviar_cpe_with_http_info(
        &self,
        filename: &str,
        document: Option<&CpeDocument>,
        content_type: Option<&str>,
    ) -> Result<ApiResponse<CpeResponse>, ApiError> {
        let req = self.enviar_cpe_request(filename, document, content_type);
        execute(
            &self.transport,
            req,
            ErrorBodies::Cpe,
            ErrorFlow::Detailed,
            CpeResponse::from_source,
        )
        .await
    }

    /// Envía un comprobante.
    ///
    /// # Errors
    ///
    /// Como [`Self::enviar_cpe_with_http_info`]; además `TypeError` si la
    /// respuesta 2xx no trae un cuerpo JSON.
    pub async fn enviar_cpe(
        &self,
        filename: &str,
        document: Option<&CpeDocument>,
    ) -> Result<CpeResponse, ApiError> {
        require(
            self.enviar_cpe_with_http_info(filename, document, None)
                .await?,
            "CpeApi::enviarCpe",
            "CpeResponse",
        )
    }

    /// Como [`Self::enviar_cpe_with_http_info`], con errores genéricos: sin
    /// [`ApiError::response_object`] para 422/500.
    ///
    /// # Errors
    ///
    /// [`ApiError`] de transporte, estado HTTP o deserialización.
    pub async fn enviar_cpe_async_with_http_info(
        &self,
        filename: &str,
        document: Option<&CpeDocument>,
        content_type: Option<&str>,
    ) -> Result<ApiResponse<CpeResponse>, ApiError> {
        let req = self.enviar_cpe_request(filename, document, content_type);
        execute(
            &self.transport,
            req,
            ErrorBodies::Cpe,
            ErrorFlow::Generic,
            CpeResponse::from_source,
        )
        .await
    }

    /// Como [`Self::enviar_cpe`], con errores genéricos.
    ///
    /// # Errors
    ///
    /// Como [`Self::enviar_cpe_async_with_http_info`].
    pub async fn enviar_cpe_async(
        &self,
        filename: &str,
        document: Option<&CpeDocument>,
    ) -> Result<CpeResponse, ApiError> {
        require(
            self.enviar_cpe_async_with_http_info(filename, document, None)
                .await?,
            "CpeApi::enviarCpe",
            "CpeResponse",
        )
    }

    /// Construye sin enviarla la solicitud
    /// `GET {host}/contribuyente/gem/comprobantes/envios/{numTicket}`.
    pub fn consultar_envio_request(&self, ticket: &str, content_type: Option<&str>) -> HttpRequest {
        HttpRequest {
            method: Method::Get,
            url: format!(
                "{}/contribuyente/gem/comprobantes/envios/{}",
                self.config.host(),
                codec::percent_encode(ticket)
            ),
            headers: base_headers(&self.config, content_type.unwrap_or(JSON), true),
            body: Vec::new(),
        }
    }

    /// Consulta un envío y devuelve también el estado y las cabeceras.
    ///
    /// # Errors
    ///
    /// Como [`Self::enviar_cpe_with_http_info`].
    pub async fn consultar_envio_with_http_info(
        &self,
        ticket: &str,
        content_type: Option<&str>,
    ) -> Result<ApiResponse<StatusResponse>, ApiError> {
        let req = self.consultar_envio_request(ticket, content_type);
        execute(
            &self.transport,
            req,
            ErrorBodies::Cpe,
            ErrorFlow::Detailed,
            StatusResponse::from_source,
        )
        .await
    }

    /// Consulta el estado de un envío por su ticket.
    ///
    /// # Errors
    ///
    /// Como [`Self::consultar_envio_with_http_info`]; además `TypeError` si
    /// la respuesta 2xx no trae un cuerpo JSON.
    pub async fn consultar_envio(&self, ticket: &str) -> Result<StatusResponse, ApiError> {
        require(
            self.consultar_envio_with_http_info(ticket, None).await?,
            "CpeApi::consultarEnvio",
            "StatusResponse",
        )
    }

    /// Como [`Self::consultar_envio_with_http_info`], con errores genéricos.
    ///
    /// # Errors
    ///
    /// [`ApiError`] de transporte, estado HTTP o deserialización.
    pub async fn consultar_envio_async_with_http_info(
        &self,
        ticket: &str,
        content_type: Option<&str>,
    ) -> Result<ApiResponse<StatusResponse>, ApiError> {
        let req = self.consultar_envio_request(ticket, content_type);
        execute(
            &self.transport,
            req,
            ErrorBodies::Cpe,
            ErrorFlow::Generic,
            StatusResponse::from_source,
        )
        .await
    }

    /// Como [`Self::consultar_envio`], con errores genéricos.
    ///
    /// # Errors
    ///
    /// Como [`Self::consultar_envio_async_with_http_info`].
    pub async fn consultar_envio_async(&self, ticket: &str) -> Result<StatusResponse, ApiError> {
        require(
            self.consultar_envio_async_with_http_info(ticket, None)
                .await?,
            "CpeApi::consultarEnvio",
            "StatusResponse",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_formats() {
        assert_eq!(
            display_uri("HTTP://User:Pass@LocalHost:80/a b/x?y=1"),
            "http://User:***@localhost/a%20b/x?y=1"
        );
        assert_eq!(
            body_summary(&[b'x'; 130]).unwrap().len(),
            120 + " (truncated...)".len()
        );
        assert_eq!(body_summary(b"bad\x01ctl"), None);
        assert_eq!(body_summary(b""), None);
        // El corte deja una «ñ» incompleta: se quita antes de «(truncated...)».
        assert_eq!(
            body_summary(format!("a{}", "ñ".repeat(61)).as_bytes()).unwrap(),
            format!("a{} (truncated...)", "ñ".repeat(59))
        );
        assert_eq!(body_summary(b"\xff"), None);
    }

    #[test]
    fn request_encoding() {
        struct Never;
        impl HttpTransport for Never {
            async fn send(&self, _: HttpRequest) -> Result<HttpResponse, crate::TransportError> {
                unreachable!("sin red")
            }
        }
        let api = AuthApi::new(Never, Configuration::default());
        let r = api.get_token_request(
            &TokenRequest::password_grant("id/1", "s&e", "20123456789MODDATOS", "p w"),
            None,
        );
        assert_eq!(
            r.url,
            "https://api-seguridad.sunat.gob.pe/v1/clientessol/id%2F1/oauth2/token/"
        );
        assert_eq!(
            String::from_utf8(r.body).unwrap(),
            "grant_type=password&scope=https%3A%2F%2Fapi-cpe.sunat.gob.pe&client_id=id%2F1&client_secret=s%26e&username=20123456789MODDATOS&password=p%20w"
        );
        let cpe = CpeApi::new(Never, Configuration::default());
        assert!(
            cpe.consultar_envio_request("t", None)
                .url
                .starts_with(crate::config::AUTH_HOST)
        );
    }
}
