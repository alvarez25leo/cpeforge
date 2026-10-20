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
  llamadas concurrentes. La clave por defecto (`CacheKey::Scoped`) aísla
  ambientes y usuarios en una misma tienda; `CacheKey::ClientId` usa solo el
  `client_id`, el formato que comparten otras implementaciones, y exige una
  tienda por ambiente y usuario. Hasta la 0.1 la clave por defecto era
  `ClientId`: al actualizar, los tokens guardados no se reutilizan y se pide
  uno nuevo una vez (ver el
  [CHANGELOG](https://github.com/alvarez25leo/cpeforge/blob/main/CHANGELOG.md)).
- Las respuestas SOAP y los CDR se rechazan antes de construir el árbol si
  superan 64 niveles, 256 atributos por elemento o el coste de ámbitos de
  espacios de nombres (`cpeforge_xml_limits::check_tree_markup`). `GreSender`
  y `ApiFactory` rechazan `.` y `..` como nombre, ticket o `client_id`
  (irían como segmento de ruta de la URL).
- Un ZIP recibido se rechaza antes de abrirlo si algún registro de fin de
  directorio central declara más de 64 entradas o marca ZIP64.
- `cargo bench -p cpeforge-ws --bench pipeline` mide cada etapa local (ZIP,
  SHA-256, Base64, SOAP, CDR, OAuth2, GRE) con transportes simulados.

## Estado

Versión 0.2.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
