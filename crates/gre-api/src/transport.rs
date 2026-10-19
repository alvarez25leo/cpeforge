//! Transporte HTTP inyectable.
//!
//! Los clientes son genéricos sobre [`HttpTransport`]: una prueba o un
//! integrador puede responder sin red. Con la característica `reqwest`
//! (activa por defecto) se incluye [`ReqwestTransport`].

use std::fmt;
use std::future::Future;

/// Método HTTP de las tres operaciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
}

impl Method {
    /// Nombre del método.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// Solicitud construida por el cliente (formas `*_request`).
#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Método.
    pub method: Method,
    /// URL completa: host configurado + ruta codificada.
    pub url: String,
    /// Cabeceras en orden fijo: `User-Agent`, `Accept`, `Content-Type` y
    /// `Authorization`; el orden forma parte de los bytes enviados.
    pub headers: Vec<(String, String)>,
    /// Cuerpo (vacío si no hay).
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Valor de una cabecera (sin distinguir mayúsculas).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

impl fmt::Debug for HttpRequest {
    /// No muestra `Authorization` ni el cuerpo, que puede llevar
    /// `client_secret` y la contraseña SOL.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &"<oculta: puede incluir identificadores>")
            .field("headers", &"<ocultas>")
            .field("body", &format_args!("<{} bytes>", self.body.len()))
            .finish()
    }
}

/// Respuesta recibida.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// Código de estado.
    pub status: u16,
    /// Frase de estado (la del servidor, o la canónica).
    pub reason: String,
    /// Cabeceras en el orden recibido.
    pub headers: Vec<(String, String)>,
    /// Cuerpo.
    pub body: Vec<u8>,
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("reason", &self.reason)
            .field("headers", &"<ocultas>")
            .field("body", &format_args!("<{} bytes>", self.body.len()))
            .finish()
    }
}

/// Tipo de fallo del transporte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportErrorKind {
    /// No se pudo conectar (DNS, conexión rechazada, TLS).
    Connect,
    /// Se superó el tiempo límite.
    Timeout,
    /// La respuesta superó el tamaño máximo.
    TooLarge,
    /// URL `http://` a un host que no es local, rechazada.
    InsecureUrl,
    /// URL inválida.
    InvalidUrl,
    /// Otro error de E/S o de protocolo (conexión cerrada sin respuesta).
    Other,
}

/// Fallo sin respuesta HTTP (`ConnectException`/`RequestException` sin respuesta).
#[derive(Clone, PartialEq, Eq)]
pub struct TransportError {
    /// Tipo.
    pub kind: TransportErrorKind,
    /// Descripción (no incluye credenciales).
    pub message: String,
}

impl fmt::Debug for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportError")
            .field("kind", &self.kind)
            .field("message", &"<oculto>")
            .finish()
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error de transporte ({:?}); consulte message en un contexto seguro",
            self.kind
        )
    }
}

impl std::error::Error for TransportError {}

/// Envío de una solicitud HTTP.
pub trait HttpTransport: Send + Sync {
    /// Envía la solicitud y devuelve la respuesta, sea cual sea su estado.
    /// El transporte decide su política de redirección y reintentos. El
    /// transporte incluido sigue redirecciones dentro del mismo origen,
    /// hasta cinco saltos, y nunca reintenta.
    fn send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send;
}

impl<T: HttpTransport + ?Sized> HttpTransport for &T {
    fn send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send {
        (**self).send(request)
    }
}

impl<T: HttpTransport + ?Sized> HttpTransport for std::sync::Arc<T> {
    fn send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send {
        (**self).send(request)
    }
}

#[cfg(feature = "reqwest")]
pub use reqwest_impl::{ReqwestTransport, ReqwestTransportBuilder};

#[cfg(feature = "reqwest")]
mod reqwest_impl {
    use std::time::Duration;

    use super::{
        HttpRequest, HttpResponse, HttpTransport, Method, TransportError, TransportErrorKind,
    };

    /// Transporte con `reqwest`:
    ///
    /// - TLS nativo del sistema con verificación de certificados y nombres;
    /// - redirecciones limitadas al mismo origen (evita reenviar el token o
    ///   las credenciales a otro host), sin reintentos ni proxy del sistema;
    /// - HTTP/1.1, cabeceras en `Title-Case` como cURL;
    /// - tiempo límite total y de conexión, y tamaño máximo de respuesta;
    /// - rechaza `http://` salvo hacia `localhost`/loopback, a menos que se
    ///   permita explícitamente.
    #[derive(Debug, Clone)]
    pub struct ReqwestTransport {
        client: reqwest::Client,
        max_response_bytes: usize,
        allow_insecure_http: bool,
    }

    /// Opciones de [`ReqwestTransport`].
    #[derive(Debug, Clone)]
    pub struct ReqwestTransportBuilder {
        timeout: Duration,
        connect_timeout: Duration,
        max_response_bytes: usize,
        allow_insecure_http: bool,
    }

    impl Default for ReqwestTransportBuilder {
        fn default() -> Self {
            Self {
                timeout: Duration::from_secs(60),
                connect_timeout: Duration::from_secs(10),
                max_response_bytes: 32 * 1024 * 1024,
                allow_insecure_http: false,
            }
        }
    }

    impl ReqwestTransportBuilder {
        /// Tiempo límite total por solicitud (por defecto 60 s).
        #[must_use]
        pub fn timeout(mut self, timeout: Duration) -> Self {
            self.timeout = timeout;
            self
        }

        /// Tiempo límite de conexión (por defecto 10 s).
        #[must_use]
        pub fn connect_timeout(mut self, timeout: Duration) -> Self {
            self.connect_timeout = timeout;
            self
        }

        /// Tamaño máximo del cuerpo de respuesta (por defecto 32 MiB).
        #[must_use]
        pub fn max_response_bytes(mut self, bytes: usize) -> Self {
            self.max_response_bytes = bytes;
            self
        }

        /// Permite `http://` hacia hosts que no son locales. **Expone el token
        /// y las credenciales en claro**; solo para entornos controlados.
        #[must_use]
        pub fn allow_insecure_http(mut self, allow: bool) -> Self {
            self.allow_insecure_http = allow;
            self
        }

        /// Construye el transporte.
        ///
        /// # Errors
        ///
        /// [`TransportError`] si no se puede inicializar TLS.
        pub fn build(self) -> Result<ReqwestTransport, TransportError> {
            let client = reqwest::Client::builder()
                .tls_backend_native()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .no_proxy()
                .http1_only()
                .http1_title_case_headers()
                .timeout(self.timeout)
                .connect_timeout(self.connect_timeout)
                .build()
                .map_err(|e| TransportError {
                    kind: TransportErrorKind::Other,
                    message: e.to_string(),
                })?;
            Ok(ReqwestTransport {
                client,
                max_response_bytes: self.max_response_bytes,
                allow_insecure_http: self.allow_insecure_http,
            })
        }
    }

    impl ReqwestTransport {
        /// Opciones con los valores por defecto.
        #[must_use]
        pub fn builder() -> ReqwestTransportBuilder {
            ReqwestTransportBuilder::default()
        }

        /// Transporte con los valores por defecto.
        ///
        /// # Errors
        ///
        /// Como [`ReqwestTransportBuilder::build`].
        pub fn new() -> Result<Self, TransportError> {
            Self::builder().build()
        }
    }

    fn is_loopback(url: &reqwest::Url) -> bool {
        match url.host() {
            Some(url::Host::Domain(d)) => {
                d.eq_ignore_ascii_case("localhost")
                    || d.to_ascii_lowercase().ends_with(".localhost")
            }
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        }
    }

    fn classify(e: &reqwest::Error) -> TransportErrorKind {
        if e.is_timeout() {
            TransportErrorKind::Timeout
        } else if e.is_connect() {
            TransportErrorKind::Connect
        } else {
            TransportErrorKind::Other
        }
    }

    fn describe(e: &reqwest::Error) -> String {
        // reqwest incluye la URL; se quita para no repetir el host y la ruta.
        let mut message = e.to_string();
        if let Some(url) = e.url() {
            message = message.replace(&format!(" for url ({url})"), "");
        }
        let mut source = std::error::Error::source(e);
        while let Some(s) = source {
            message.push_str(": ");
            message.push_str(&s.to_string());
            source = s.source();
        }
        message
    }

    impl HttpTransport for ReqwestTransport {
        async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            let original_url = reqwest::Url::parse(&request.url).map_err(|e| TransportError {
                kind: TransportErrorKind::InvalidUrl,
                message: format!("URL inválida: {e}"),
            })?;
            if original_url.scheme() != "https"
                && !(self.allow_insecure_http
                    || (original_url.scheme() == "http" && is_loopback(&original_url)))
            {
                return Err(TransportError {
                    kind: TransportErrorKind::InsecureUrl,
                    message: format!(
                        "se rechaza {}:// hacia un host no local",
                        original_url.scheme()
                    ),
                });
            }
            let mut current = request;
            let mut current_url = original_url.clone();
            let mut hops = 0;
            let mut response = loop {
                let method = match current.method {
                    Method::Get => reqwest::Method::GET,
                    Method::Post => reqwest::Method::POST,
                };
                let mut builder = self.client.request(method, current_url.clone());
                for (k, v) in &current.headers {
                    builder = builder.header(k.as_str(), v.as_str());
                }
                // Con el `User-Agent` vacío se envía el mismo valor que el
                // cliente de referencia en ese caso, no el de reqwest.
                if current.header("user-agent").is_none() {
                    builder = builder.header("User-Agent", "GuzzleHttp/7");
                }
                if current.method == Method::Post && current.body.is_empty() {
                    builder = builder.header("Content-Length", "0");
                }
                if current.method == Method::Post || !current.body.is_empty() {
                    builder = builder.body(current.body.clone());
                }
                let next_response = builder.send().await.map_err(|e| TransportError {
                    kind: classify(&e),
                    message: describe(&e),
                })?;
                let redirect =
                    matches!(next_response.status().as_u16(), 301 | 302 | 303 | 307 | 308);
                if !redirect || hops >= 5 {
                    break next_response;
                }
                let Some(location) = next_response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                else {
                    break next_response;
                };
                let Ok(target) = current_url.join(location) else {
                    break next_response;
                };
                if target.scheme() != original_url.scheme()
                    || target.host_str() != original_url.host_str()
                    || target.port_or_known_default() != original_url.port_or_known_default()
                {
                    break next_response;
                }
                if matches!(next_response.status().as_u16(), 301..=303)
                    && current.method == Method::Post
                {
                    current.method = Method::Get;
                    current.body.clear();
                }
                current_url = target;
                hops += 1;
            };
            let status = response.status();
            let reason = response
                .extensions()
                .get::<hyper::ext::ReasonPhrase>()
                .map(|r| String::from_utf8_lossy(r.as_bytes()).into_owned())
                .or_else(|| status.canonical_reason().map(str::to_owned))
                .unwrap_or_default();
            let headers = response
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_owned(),
                        String::from_utf8_lossy(v.as_bytes()).into_owned(),
                    )
                })
                .collect();
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| TransportError {
                kind: classify(&e),
                message: describe(&e),
            })? {
                if body.len() + chunk.len() > self.max_response_bytes {
                    return Err(TransportError {
                        kind: TransportErrorKind::TooLarge,
                        message: format!("respuesta mayor que {} bytes", self.max_response_bytes),
                    });
                }
                body.extend_from_slice(&chunk);
            }
            Ok(HttpResponse {
                status: status.as_u16(),
                reason,
                headers,
                body,
            })
        }
    }
}
