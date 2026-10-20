# cpeforge-gre-api

Cliente HTTP de bajo nivel para la API GRE de SUNAT (guía de remisión
electrónica): token, envío de CPE y consulta de envío.

```rust,no_run
use cpeforge_gre_api::{AuthApi, Configuration, CpeApi, ReqwestTransport, TokenRequest};

# async fn ejemplo() -> Result<(), Box<dyn std::error::Error>> {
let transport = ReqwestTransport::new()?;
let auth = AuthApi::new(transport.clone(), Configuration::auth());
let credentials = TokenRequest::password_grant("cliente", "secreto", "rucusuario", "clave");
let token = auth.get_token(&credentials).await?;
let cpe = CpeApi::new(transport, Configuration::cpe(token.access_token.unwrap_or_default()));
let _status = cpe.consultar_envio("ticket").await?;
# Ok(())
# }
```

El ejemplo muestra la API; no debe ejecutarse con credenciales reales durante
las pruebas. `examples/flujo_gre.rs` usa un transporte simulado y no abre
conexiones.

- `get_token`, `enviar_cpe` y `consultar_envio` son `async`. Las formas
  `_with_http_info` devuelven estado y cabeceras además del modelo; las formas
  `_request` construyen la solicitud sin enviarla. El transporte implementa
  `HttpTransport`, de modo que se puede inyectar uno de prueba.
- `Configuration::default()` apunta al host de autenticación. Para enviar o
  consultar use `Configuration::cpe(token)`.
- `ReqwestTransport` verifica TLS, fija timeouts, limita el cuerpo, no usa
  proxy, no reintenta y solo sigue redirecciones dentro del mismo origen. Solo
  acepta HTTP sin TLS para localhost/loopback salvo habilitación explícita.
- Un `client_id`, `filename` o `numTicket` igual a `.` o `..` se rechaza
  antes de enviar (`ApiError::InvalidRequest`): la URL se normalizaría y el
  token o las credenciales irían a otra ruta del host. `ReqwestTransport`
  también rechaza esos segmentos en solicitudes construidas con `_request`.
- El User-Agent por defecto es `CPEForge/0.1.0`.
- El crate no construye ZIP, no calcula hashes ni cachea tokens (eso lo hace
  `cpeforge-ws`). `Debug` y `Display` de solicitudes, respuestas y errores
  ocultan contenido potencialmente sensible; `ApiError::message()` expone el
  mensaje completo de forma explícita.

## Estado

Versión estable. La API pública sigue SemVer: los errores, resultados y
opciones pueden crecer sin romper el código que la usa (ver el
[README del repositorio](https://github.com/alvarez25leo/cpeforge#estado)).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
