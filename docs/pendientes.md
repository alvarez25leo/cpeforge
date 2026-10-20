# Pendientes

## Código y pruebas

- **cpeforge-cpe-validator: ejecutar las hojas SUNAT tal como se publican.**
  Las hojas de «Archivo XSL – 2.1» incluyen `local:///commons/…` de IBM
  DataPower (SUNAT no los publica) y usan `dp:` y EXSLT `regexp:match`, que
  libexslt no implementa. Falta un adaptador: `local:///` → directorio de
  `commons` del consumidor, `regexp:match`/`regexp:test` sobre PCRE2 y `dp:`
  sin efecto. Además, los nombres del resolver (`Percepcion-1.0.1`,
  `GreRemitente-2.0.1`) no coinciden con los del ZIP (`Percepcion-1.0.2`,
  `GuiaRemitente-2.0.1`).
- **cpeforge-xmldsig.** Contrastar con otra implementación la transformación
  XPath.
- **PDF.** Nuevo crate para generar el PDF de la representación impresa
  (`cpeforge-report` termina en HTML).

## Seguridad

- **cpeforge-ws: `CacheKey` por defecto.** `CacheKey::ClientId` comparte el
  token entre ambientes o usuarios con el mismo `client_id` en una tienda
  común; pasar a `CacheKey::Scoped` por defecto cambia el formato de clave
  compartido con otras implementaciones (decidir antes de la versión estable).
- **cpeforge-report: `XmlUtils` sin límite de nodos ni de pila.** Analiza con
  `nodes_limit` ilimitado y admite 256 niveles por defecto (`ws` y
  `xml-parser` usan 64: en debug `roxmltree` desborda la pila de un hilo de
  Tokio hacia los 120). Añadir `max_nodes` y bajar `max_depth` cambia campos
  públicos de un struct no `non_exhaustive`.
- **cpeforge-ws: ZIP con miles de entradas.** `ZipArchive::new` lee todo el
  directorio central antes de comprobar `MAX_ENTRIES` (64). Contar las
  entradas en el registro de fin de directorio antes de abrirlo.
- **cpeforge-gre-api: segmentos `.`/`..`.** Los `*_request` de `AuthApi` y
  `CpeApi` construyen la URL con ellos (solo `ws` los rechaza); validarlos en
  `gre-api` cambia su API pública (devuelven `HttpRequest`, no `Result`).

## Revisión y repositorio

- **Linux.** Confirmar en la CI (ubuntu-24.04) las comprobaciones de versión
  mínima de libxml2/libxslt de `build.rs` (pkg-config); solo se han visto
  pasar en macOS.
- **Revisión independiente** de la API de todos los crates y de seguridad de
  `ws`, `gre-api`, `xml-parser` y `report` antes de declarar una versión
  estable.
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
