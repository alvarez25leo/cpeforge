# cpeforge-ws

Transporte SOAP, ZIP, lectura de CDR y adaptador GRE para los servicios de
comprobantes electrónicos de SUNAT. Las pruebas no envían datos a SUNAT/OSE.

`cargo run -p cpeforge-ws --example local_cdr --locked` muestra ZIP/CDR y la
construcción de una solicitud SOAP local sin enviarla.

- `outcome::TaxStatus` clasifica un resultado como pendiente, aceptado (con o
  sin observaciones), rechazado, excepción o indeterminado. `success` solo
  indica que el servicio respondió lo esperado.
- `cdr::read_cdr_checked` exige raíz `ApplicationResponse`, `ResponseCode` y
  `ReferenceID`; `cdr::read_cdr` es la variante tolerante.
- `api::ApiFactory` emite una sola solicitud de token por clave aunque haya
  llamadas concurrentes; `CacheKey::Scoped` aísla ambientes y usuarios. Con
  la clave por defecto (`client_id`), varias empresas o ambientes en una
  misma tienda comparten token si comparten `client_id`: en un servicio
  multiempresa usar `CacheKey::Scoped` o una tienda por empresa.
- Las respuestas SOAP y los CDR se rechazan antes de construir el árbol si
  superan 64 niveles, 256 atributos por elemento o el coste de ámbitos de
  espacios de nombres (`cpeforge_xml_limits::check_tree_markup`). `GreSender`
  y `ApiFactory` rechazan `.` y `..` como nombre, ticket o `client_id`
  (irían como segmento de ruta de la URL).
- `cargo bench -p cpeforge-ws --bench pipeline` mide cada etapa local (ZIP,
  SHA-256, Base64, SOAP, CDR, OAuth2, GRE) con transportes simulados.

## Estado

Versión 0.1.1 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
