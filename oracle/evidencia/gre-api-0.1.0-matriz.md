# Inventario público y matriz de `greenter/gre-api` v1.0.2

Fuente: checkout aislado `target/php-reference/gre-api-v1.0.2` en
`81627a3511b98cba7243ee3a60cc66ff4a688f51`; `src/` contiene **17 archivos
PHP**. El `openapi.yaml` de ese commit declara la versión documental `1.0.0`,
tres operaciones y seis schemas; las dos clases PHP para objetos anidados
completan los ocho modelos. El inventario se hizo antes de cerrar el diseño
del cliente; las filas «pendiente» impiden declarar paridad total.

| Fuente PHP / OpenAPI | API pública y comportamiento | Rust / casos | Estado |
| --- | --- | --- | --- |
| `Api/AuthApi.php`, `AuthApiInterface.php`; `getToken` | `getToken`, `getTokenWithHttpInfo`, `getTokenAsync`, `getTokenAsyncWithHttpInfo`, `getTokenRequest`, `contentTypes`, constructor, `getConfig`, `setHostIndex`, `getHostIndex`. `POST /clientessol/{client_id}/oauth2/token/`; seis campos form-urlencoded, `Accept`, `Content-Type`, `User-Agent`; cuerpo `ApiToken`. El índice se guarda pero el código usa `getHost()`. | `AuthApi` y `TokenRequest`; `token-*` (78). Todas las variantes operativas, incluidas las diferencias de error async, tienen método Rust. Sin equivalente público para `setHostIndex`/`getHostIndex`, porque no alteran las tres operaciones. | 75 comparables; 3 argumentos nulos fuera del tipo Rust; errores de transporte distintos. |
| `Api/CpeApi.php`, `CpeApiInterface.php`; `enviarCpe` | `enviarCpe`, `enviarCpeWithHttpInfo`, `enviarCpeAsync`, `enviarCpeAsyncWithHttpInfo`, `enviarCpeRequest`, constructor, `getConfig`, `setHostIndex`, `getHostIndex`. `POST /contribuyente/gem/comprobantes/{filename}`, JSON `CpeDocument`, Bearer; 200 `CpeResponse`, 422 `CpeErrorValidation`, 500 `CpeError`. | `CpeApi::enviar_cpe*`; `envio-*` (55), incluidos path hostil, cuerpo grande, JSON, error, fecha y async. | Implementado con diferencias de fecha documentadas. |
| `Api/CpeApi.php`, `CpeApiInterface.php`; `consultarEnvio` | `consultarEnvio`, `consultarEnvioWithHttpInfo`, `consultarEnvioAsync`, `consultarEnvioAsyncWithHttpInfo`, `consultarEnvioRequest`. `GET /contribuyente/gem/comprobantes/envios/{numTicket}`, Bearer; 200 `StatusResponse`, 422/500 cuerpos de error. | `CpeApi::consultar_envio*`; `consulta-*` (25), estados `98`/`99`/`0`, error anidado, JSON y redirección. | Implementado; redirección entre orígenes deliberadamente bloqueada. |
| `Model/ApiToken.php` | `access_token`, `token_type`, `expires_in` (PHP `int`; OpenAPI `int32`). | `ApiToken`; `token-values-*`, `token-body-*`; ausente/null/vacío, tipos PHP, límites. | Comparado por operaciones. |
| `Model/CpeDocument.php`, `Model/CpeDocumentArchivo.php` | `archivo` anidado; `nomArchivo`, `arcGreZip`, `hashZip`; el paquete PHP no construye ZIP ni comprueba hash. | `CpeDocument`, `CpeDocumentArchivo`; `envio-document-*`, `envio-json-*`, `envio-large`. | Comparado por operaciones. |
| `Model/CpeResponse.php` | `numTicket`, `fecRecepcion` (`DateTime`); PHP acepta fechas relativas/implícitas y normaliza fechas imposibles. | `CpeResponse`, `DateTimeValue`; `envio-fecha-*`. | Seis diferencias registradas. |
| `Model/StatusResponse.php`, `Model/StatusResponseError.php` | `codRespuesta`, `error` (`numError`, `desError`), `arcCdr`, `indCdrGenerado`; los códigos se manejan como cadenas. | `StatusResponse`, `StatusResponseError`; `consulta-*`. | Comparado por operaciones. |
| `Model/CpeError.php`, `Model/CpeErrorValidation.php` | `cod`, `msg`; `errors` lista de `CpeError` con elementos `null` posibles. | `CpeError`, `CpeErrorValidation`; `envio-error-*`, `consulta-error-*`. | Comparado por operaciones. |
| `Model/ModelInterface.php` y ocho modelos | `openAPITypes`, `openAPIFormats`, `attributeMap`, `setters`, `getters`, `getModelName`, `listInvalidProperties`, `valid`, `isNullable`, `isNullableSetToNull`, constructor, getters/setters de cada campo, `ArrayAccess` (`offsetExists/Get/Set/Unset`), `jsonSerialize`, `__toString`, `toHeaderValue`. Los modelos PHP son mutables y admiten campos adicionales por `ArrayAccess`. | Campos públicos tipados, `Model::model_name/to_json/to_json_pretty`; se comparan las conversiones observables de las tres operaciones. | **Pendiente** paridad de la superficie de introspección y mutación PHP que no forma parte del cliente Rust idiomático. |
| `Configuration.php` | `get/setApiKey`, `get/setApiKeyPrefix`, `get/setAccessToken`, `get/setBooleanFormatForQueryString`, `get/setUsername`, `get/setPassword`, `get/setHost`, `get/setUserAgent`, `get/setDebug`, `get/setDebugFile`, `get/setTempFolderPath`, `getDefaultConfiguration`, `setDefaultConfiguration`, `toDebugReport`, `getApiKeyWithPrefix`, `getHostSettings`, `getHostString`, `getHostFromSettings`, constantes de boolean. | `Configuration` inmutable (`with_*`), `host_from_settings`, `AUTH_HOST`, `CPE_HOST`, `HOST_SETTINGS`; `config-defaults`. | Configuración usada por las tres operaciones comparada; opciones genéricas no usadas (API key, Basic, debug, ruta temporal, estado global) **sin equivalente público**. |
| `HeaderSelector.php` | `selectHeaders` y `getNextWeight`; selección por `Accept`/`Content-Type`, pesos y multipart. | Cabeceras de las tres operaciones en `api.rs`; `token-content-type-*`, `envio-content-type-*`, `consulta-content-type-*`. | Ramas usadas comparadas; selector genérico y multipart no portados. |
| `ObjectSerializer.php` | `setDateTimeFormat`, `sanitizeForSerialization`, `sanitizeFilename`, `sanitizeTimestamp`, `toPathValue`, `toQueryValue`, `convertBoolToQueryStringFormat`, `toHeaderValue`, `toFormValue`, `toString`, `serializeCollection`, `deserialize`, `buildQuery`. | `php.rs`, `model.rs`, `datetime.rs`; los tres request builders y la deserialización de respuestas/errores prueban las ramas efectivamente usadas. | Helpers genéricos de query/colecciones fuera de las tres operaciones **pendientes**; fechas difieren. |
| `ApiException.php` | Constructor; `getResponseHeaders`, `getResponseBody`, `setResponseObject`, `getResponseObject`, código/mensaje heredados. | `ApiError::{Http,Transport,Php}`, `response_*`, `code`, `message`; `token-error-*`, `envio-error-*`, `consulta-error-*`. | Metadatos comparados; textos cURL difieren. |

## Contrato OpenAPI y host

| Operación | Servidor OpenAPI | Host del código PHP | Cuerpo y respuesta declarados |
| --- | --- | --- | --- |
| `getToken` | `https://api-seguridad.sunat.gob.pe/v1` | mismo por defecto | Formulario de seis campos; `200 ApiToken`. |
| `enviarCpe` | `https://api.sunat.gob.pe/v1` | `Configuration::getHostSettings()[1]` = `https://api-cpe.sunat.gob.pe/v1`; `CpeApi` sin configuración conserva el **host de seguridad** porque usa `getHost()` | JSON `CpeDocument`; `200 CpeResponse`, `422 CpeErrorValidation`, `500 CpeError`. |
| `consultarEnvio` | `https://api.sunat.gob.pe/v1` | igual que `enviarCpe` | Bearer; `200 StatusResponse`, `422 CpeErrorValidation`, `500 CpeError`. |

`greenter/ws` v5.3.0 (`ApiFactory.php`) elige explícitamente el segundo host
de `Configuration`; `GreSender.php` llama a las dos operaciones CPE.
`greenter/lite` configura el host de autenticación. No se portaron su caché de
token, ZIP, CDR ni flujos de facturación a este crate.

Los fixtures por fila están en `tests/reference/gre-api/cases.json` (159 casos),
el resultado ejecutado del PHP en `expected.json`, y las observaciones
independientes de OpenAPI en `contract.json`. El comparador `parity.rs` exige
que no aparezcan diferencias nuevas y **falla** mientras existan las once
diferencias conocidas o los tres casos nulos no aplicables.
