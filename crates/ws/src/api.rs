//! Caché de token y adaptación GRE. El HTTP lo implementa `cpeforge-gre-api`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use base64::Engine as _;
use cpeforge_core::model::response::{ResponseError, StatusResult, SummaryResult};
use cpeforge_gre_api::{
    ApiError, AuthApi, Configuration, CpeApi, CpeDocument, CpeDocumentArchivo, ErrorObject,
    HttpTransport, Secret, TokenRequest,
};
use sha2::{Digest, Sha256};

use crate::WsError;
use crate::cdr::read_cdr_zip;
use crate::zip::{ZipTime, compress_at};

/// Un token se reutiliza solo si le quedan **más** de estos segundos.
const RENEW_MARGIN: Duration = Duration::from_secs(600);

/// Token guardado sin exposición en `Debug`.
#[derive(Clone)]
pub struct CachedToken {
    /// Valor protegido.
    pub value: Secret,
    /// Instante de vencimiento.
    pub expires_at: SystemTime,
}

impl std::fmt::Debug for CachedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CachedToken(<oculto>)")
    }
}

/// Puerto para guardar tokens por clave (véase [`CacheKey`]).
///
/// Implementarlo sobre un almacén externo (Redis, base de datos) permite
/// compartir tokens entre varias instancias. Las claves no contienen
/// secretos, pero el valor sí: el almacén debe protegerlo.
pub trait TokenStore: Send + Sync {
    /// Busca token; ningún registro debe exponerlo.
    ///
    /// # Errors
    ///
    /// Fallo de la tienda.
    fn get(&self, id: &str) -> Result<Option<CachedToken>, WsError>;
    /// Guarda token.
    ///
    /// # Errors
    ///
    /// Fallo de la tienda.
    fn set(&self, id: &str, token: CachedToken) -> Result<(), WsError>;
    /// Elimina el token de `id`. La implementación por defecto no hace nada,
    /// para tiendas que solo expiran por tiempo.
    ///
    /// # Errors
    ///
    /// Fallo de la tienda.
    fn remove(&self, id: &str) -> Result<(), WsError> {
        let _ = id;
        Ok(())
    }
}

impl<T: TokenStore + ?Sized> TokenStore for Arc<T> {
    fn get(&self, id: &str) -> Result<Option<CachedToken>, WsError> {
        (**self).get(id)
    }
    fn set(&self, id: &str, token: CachedToken) -> Result<(), WsError> {
        (**self).set(id, token)
    }
    fn remove(&self, id: &str) -> Result<(), WsError> {
        (**self).remove(id)
    }
}

/// Tienda en memoria de un proceso, segura entre hilos.
///
/// Guarda un token por clave y sustituye el anterior al renovarlo, así que
/// crece con el número de claves distintas, no con el de solicitudes. Los
/// tokens vencidos permanecen hasta que se renuevan o se llama a
/// [`InMemoryStore::remove_expired`]. Las secciones críticas solo copian o
/// insertan un valor y no pueden dejar el mapa a medias, por lo que un mutex
/// envenenado se recupera en lugar de inutilizar la tienda.
#[derive(Default)]
pub struct InMemoryStore(Mutex<HashMap<String, CachedToken>>);

impl std::fmt::Debug for InMemoryStore {
    /// Solo el número de tokens guardados, nunca sus valores.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryStore")
            .field("tokens", &self.len())
            .finish()
    }
}

impl InMemoryStore {
    fn map(&self) -> MutexGuard<'_, HashMap<String, CachedToken>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Elimina los tokens que vencen en `now` o antes y devuelve cuántos.
    pub fn remove_expired(&self, now: SystemTime) -> usize {
        let mut map = self.map();
        let before = map.len();
        map.retain(|_, token| token.expires_at > now);
        before - map.len()
    }

    /// Número de claves guardadas.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map().len()
    }

    /// Indica si la tienda está vacía.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map().is_empty()
    }
}

impl TokenStore for InMemoryStore {
    fn get(&self, id: &str) -> Result<Option<CachedToken>, WsError> {
        Ok(self.map().get(id).cloned())
    }

    fn set(&self, id: &str, token: CachedToken) -> Result<(), WsError> {
        self.map().insert(id.to_owned(), token);
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), WsError> {
        self.map().remove(id);
        Ok(())
    }
}

/// Clave con que [`ApiFactory`] guarda los tokens en su [`TokenStore`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheKey {
    /// Solo `client_id`: el formato de clave compatible con otras
    /// implementaciones que comparten la tienda. Dos ambientes (beta y
    /// producción) o dos usuarios SOL con el mismo `client_id` comparten
    /// token: usarla solo con una tienda por ambiente y usuario.
    ClientId,
    /// Host de autenticación, `scope`, `client_id` y usuario SOL, separados
    /// por U+001F. Aísla ambientes y usuarios dentro de una misma tienda. Es
    /// la clave por defecto.
    #[default]
    Scoped,
}

/// Registro de renovaciones en curso: un candado asíncrono por clave.
type Flights = Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>;

/// Turno exclusivo para renovar el token de una clave. Al soltarse, libera el
/// candado y retira la entrada del registro si nadie más la espera, también
/// si la tarea se cancela mientras espera.
struct Flight<'a> {
    flights: &'a Flights,
    key: String,
    lock: Arc<tokio::sync::Mutex<()>>,
    guard: Option<tokio::sync::OwnedMutexGuard<()>>,
}

impl<'a> Flight<'a> {
    async fn acquire(flights: &'a Flights, key: &str) -> Self {
        let lock = flights
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(key.to_owned())
            .or_default()
            .clone();
        let mut flight = Self {
            flights,
            key: key.to_owned(),
            lock,
            guard: None,
        };
        flight.guard = Some(flight.lock.clone().lock_owned().await);
        flight
    }
}

impl Drop for Flight<'_> {
    fn drop(&mut self) {
        self.guard.take();
        let mut flights = self.flights.lock().unwrap_or_else(PoisonError::into_inner);
        // Las copias solo se crean con el registro bloqueado: con dos (la del
        // registro y la propia) nadie más espera esta clave.
        if Arc::strong_count(&self.lock) == 2 {
            flights.remove(&self.key);
        }
    }
}

/// Reloj inyectable para el margen estricto de renovación de 600 segundos.
pub trait Clock: Send + Sync {
    /// Instante actual.
    fn now(&self) -> SystemTime;
}

/// Reloj del sistema.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Crea `CpeApi` usando token cacheado o recién obtenido con `AuthApi`.
///
/// Las llamadas concurrentes a [`ApiFactory::create`] con la misma clave y la
/// caché vacía o vencida emiten **una sola** solicitud de token: la primera
/// renueva y las demás esperan y reutilizan el resultado. Si la renovación
/// falla, la siguiente en espera lo intenta de nuevo; los errores no se
/// guardan. La coordinación es por proceso: con varias instancias, cada una
/// renueva por su cuenta aunque compartan un [`TokenStore`] externo.
///
/// # Clave de la caché
///
/// Por defecto los tokens se guardan con [`CacheKey::Scoped`] (host de
/// autenticación, `scope`, `client_id` y usuario SOL): una misma tienda sirve
/// a varias empresas y ambientes sin mezclar sus tokens. Hasta la versión 0.1
/// la clave era solo el `client_id`; para compartir la tienda con otra
/// implementación que use ese formato, usar
/// `.with_cache_key(CacheKey::ClientId)` y una tienda por ambiente y usuario.
pub struct ApiFactory<T, S, C> {
    transport: T,
    auth_config: Configuration,
    cpe_host: String,
    store: S,
    clock: C,
    cache_key: CacheKey,
    flights: Flights,
}

impl<T, S, C> std::fmt::Debug for ApiFactory<T, S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiFactory(<configuración y credenciales ocultas>)")
    }
}

impl<T: HttpTransport + Clone, S: TokenStore, C: Clock> ApiFactory<T, S, C> {
    /// Configura el cliente. `auth_config` permite URL loopback en pruebas.
    #[must_use]
    pub fn new(
        transport: T,
        auth_config: Configuration,
        cpe_host: impl Into<String>,
        store: S,
        clock: C,
    ) -> Self {
        Self {
            transport,
            auth_config,
            cpe_host: cpe_host.into(),
            store,
            clock,
            cache_key: CacheKey::default(),
            flights: Flights::default(),
        }
    }

    /// Cambia la clave de caché (por defecto, [`CacheKey::Scoped`]).
    #[must_use]
    pub fn with_cache_key(mut self, cache_key: CacheKey) -> Self {
        self.cache_key = cache_key;
        self
    }

    /// Clave con que se guarda el token de `request`.
    #[must_use]
    pub fn cache_key(&self, request: &TokenRequest) -> String {
        match self.cache_key {
            CacheKey::ClientId => request.client_id.clone(),
            CacheKey::Scoped => [
                self.auth_config.host(),
                request.scope.as_str(),
                request.client_id.as_str(),
                request.username.as_str(),
            ]
            .join("\u{1f}"),
        }
    }

    /// Obtiene token cuando faltan **600 segundos o menos** para vencer.
    ///
    /// # Errors
    ///
    /// Tienda, reloj, autorización, `client_id` igual a `.` o `..`, o
    /// respuesta inválida.
    pub async fn create(&self, request: &TokenRequest) -> Result<CpeApi<T>, WsError> {
        let key = self.cache_key(request);
        let token = match self.cached(&key)? {
            Some(token) => token,
            None => {
                let _turn = Flight::acquire(&self.flights, &key).await;
                // Otra tarea pudo renovar mientras se esperaba el turno.
                match self.cached(&key)? {
                    Some(token) => token,
                    None => self.renew(request, &key).await?,
                }
            }
        };
        Ok(CpeApi::new(
            self.transport.clone(),
            Configuration::cpe(token.expose()).with_host(&self.cpe_host),
        ))
    }

    /// Descarta el token guardado para `request`, por ejemplo tras un `401`
    /// de la API. La siguiente llamada a [`ApiFactory::create`] lo renueva.
    ///
    /// # Errors
    ///
    /// Fallo de la tienda.
    pub fn invalidate(&self, request: &TokenRequest) -> Result<(), WsError> {
        self.store.remove(&self.cache_key(request))
    }

    fn cached(&self, key: &str) -> Result<Option<Secret>, WsError> {
        let now = self.clock.now();
        let limit = now.checked_add(RENEW_MARGIN).unwrap_or(now);
        Ok(self
            .store
            .get(key)?
            .filter(|item| item.expires_at > limit)
            .map(|item| item.value))
    }

    async fn renew(&self, request: &TokenRequest, key: &str) -> Result<Secret, WsError> {
        path_segment(&request.client_id)?;
        let now = self.clock.now();
        let auth = AuthApi::new(self.transport.clone(), self.auth_config.clone());
        let response = auth.get_token(request).await.map_err(WsError::GreApi)?;
        let value = response
            .access_token
            .filter(|v| !v.is_empty())
            .ok_or(WsError::InvalidServiceResponse("Cliente No autorizado"))?;
        let seconds = response
            .expires_in
            .ok_or(WsError::InvalidResponse("expires_in ausente"))?;
        let expiry = if seconds >= 0 {
            now.checked_add(Duration::from_secs(seconds.unsigned_abs()))
        } else {
            now.checked_sub(Duration::from_secs(seconds.unsigned_abs()))
        }
        .ok_or(WsError::InvalidResponse("expiración fuera de rango"))?;
        let secret = Secret::new(value);
        self.store.set(
            key,
            CachedToken {
                value: secret.clone(),
                expires_at: expiry,
            },
        )?;
        Ok(secret)
    }
}

/// Envío y consulta GRE reutilizando exactamente `CpeApi` de `gre-api`.
pub struct GreSender<T> {
    api: CpeApi<T>,
    zip_time: ZipTime,
}

impl<T> std::fmt::Debug for GreSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GreSender(<cliente oculto>)")
    }
}

impl<T: HttpTransport> GreSender<T> {
    /// Construye con cliente GRE ya autenticado.
    #[must_use]
    pub fn new(api: CpeApi<T>) -> Self {
        Self {
            api,
            zip_time: ZipTime::default(),
        }
    }

    /// Hora que se escribe en el ZIP (por defecto, la actual en UTC).
    #[must_use]
    pub fn with_zip_time(mut self, zip_time: ZipTime) -> Self {
        self.zip_time = zip_time;
        self
    }

    /// Comprime XML, calcula Base64 y SHA-256 de los **mismos bytes ZIP** y envía.
    ///
    /// `success == Some(true)` indica que la API aceptó la solicitud, no el
    /// comprobante: el resultado se conoce con [`GreSender::status`] y se
    /// clasifica con [`crate::outcome::TaxStatus`].
    ///
    /// # Errors
    ///
    /// ZIP inválido, `filename` igual a `.` o `..`, o fallo de transporte no
    /// representable como resultado.
    pub async fn send(&self, filename: &str, xml: &[u8]) -> Result<SummaryResult, WsError> {
        path_segment(filename)?;
        let zip = compress_at(&format!("{filename}.xml"), xml, self.zip_time)?;
        let digest = Sha256::digest(&zip);
        let mut hash = String::with_capacity(64);
        for byte in digest {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            hash.push(char::from(HEX[usize::from(byte >> 4)]));
            hash.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        let doc = CpeDocument {
            archivo: Some(CpeDocumentArchivo {
                nom_archivo: Some(format!("{filename}.zip")),
                arc_gre_zip: Some(base64::engine::general_purpose::STANDARD.encode(&zip)),
                hash_zip: Some(hash),
            }),
        };
        let mut result = SummaryResult::default();
        match self.api.enviar_cpe(filename, Some(&doc)).await {
            Ok(response) => {
                result.ticket = response.num_ticket;
                result.base.success = Some(true);
            }
            Err(error) => result.base.error = Some(api_error(error)?),
        }
        Ok(result)
    }

    /// Consulta ticket GRE y extrae CDR cuando fue generado.
    ///
    /// Con código `0` o `99` y CDR generado, `success` es `Some(true)` aunque
    /// el CDR sea de rechazo (`99`); [`crate::outcome::TaxStatus::of_status`]
    /// distingue aceptación, observaciones y rechazo.
    ///
    /// # Errors
    ///
    /// `ticket` igual a `.` o `..`, Base64/CDR inválido o fallo de transporte
    /// no representable.
    pub async fn status(&self, ticket: &str) -> Result<StatusResult, WsError> {
        path_segment(ticket)?;
        let mut result = StatusResult::default();
        let status = match self.api.consultar_envio(ticket).await {
            Ok(value) => value,
            Err(error) => {
                result.bill.base.error = Some(api_error(error)?);
                return Ok(result);
            }
        };
        result.code = status.cod_respuesta.clone();
        match status.cod_respuesta.as_deref() {
            Some("98") => {
                result.bill.base.error = Some(ResponseError::new(
                    Some("98".into()),
                    Some("En proceso".into()),
                ))
            }
            Some("0" | "99") => {
                if let Some(error) = status.error {
                    result.bill.base.error =
                        Some(ResponseError::new(error.num_error, error.des_error));
                }
                if status.ind_cdr_generado.as_deref() == Some("1") {
                    let raw = status
                        .arc_cdr
                        .ok_or(WsError::InvalidResponse("arcCdr ausente"))?;
                    let zip = base64::engine::general_purpose::STANDARD
                        .decode(raw)
                        .map_err(|_| WsError::InvalidResponse("arcCdr Base64 inválido"))?;
                    result.bill.cdr_response = Some(read_cdr_zip(&zip)?);
                    result.bill.cdr_zip = Some(zip);
                    result.bill.base.success = Some(true);
                }
            }
            _ => {}
        }
        Ok(result)
    }
}

/// El valor va como segmento de ruta de la URL: `.` y `..` se normalizarían
/// y la solicitud, con el token o las credenciales, iría a otra ruta del host.
fn path_segment(value: &str) -> Result<(), WsError> {
    if matches!(value, "." | "..") {
        return Err(WsError::Security("segmento de ruta «.» o «..»"));
    }
    Ok(())
}

fn api_error(error: ApiError) -> Result<ResponseError, WsError> {
    match &error {
        ApiError::Http {
            code: 422,
            object: Some(object),
            ..
        } => match object.as_ref() {
            ErrorObject::Validation(body) => body
                .errors_iter()
                .next()
                .map(|first| ResponseError::new(first.cod.clone(), first.msg.clone()))
                .ok_or(WsError::InvalidResponse("422 sin errors")),
            _ => Err(WsError::InvalidResponse("422 sin CpeErrorValidation")),
        },
        ApiError::Http {
            code: 500,
            object: Some(object),
            ..
        } => match object.as_ref() {
            ErrorObject::Server(body) => Ok(ResponseError::new(body.cod.clone(), body.msg.clone())),
            _ => Err(WsError::InvalidResponse("500 sin CpeError")),
        },
        ApiError::Transport { .. } => Err(WsError::GreApi(error)),
        _ => Ok(ResponseError::new(
            Some("API".into()),
            Some(error.message().to_owned()),
        )),
    }
}
