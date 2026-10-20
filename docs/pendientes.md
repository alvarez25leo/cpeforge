# Pendientes

## Código y pruebas

- **cpeforge-cpe-validator: ejecutar las hojas SUNAT tal como se publican.**
  Las 15 hojas de «Archivo XSL – 2.1» (2022-09-06) se probaron: todas se
  analizan y el validador las detiene, sin E/S, en el `include` de
  `local:///commons/error/validate_utils.xsl` o `error_utils.xsl`. Esos
  `commons` (plantillas `isTrueExpresion`, `existElement`,
  `findElementInCatalog`… y los catálogos) son de IBM DataPower y SUNAT no
  los publica; las hojas usan además `dp:` y EXSLT `regexp:match`, que
  libexslt no implementa. Hoy, como la referencia, se usan hojas convertidas
  (`regexp:match` → `MATCHES_HANDLER`). Para ejecutar las originales haría
  falta un adaptador: `local:///` → directorio de `commons` del consumidor,
  `regexp:match`/`regexp:test` sobre PCRE2 y `dp:` sin efecto. Además, los
  nombres del resolver son los de la referencia (`Percepcion-1.0.1`,
  `GreRemitente-2.0.1`); el ZIP trae `Percepcion-1.0.2` y
  `GuiaRemitente-2.0.1`.
- **cpeforge-xmldsig.** Contrastar con otra implementación la transformación
  XPath (las referencias enveloped y por `Id`, los digests y las firmas RSA y
  HMAC ya se verifican con `xmllint` y `openssl` en `tests/external.rs`).
- **PDF.** Nuevo crate para generar el PDF de la representación impresa
  (`cpeforge-report` termina en HTML).

## Revisión y repositorio

- **Linux.** Las comprobaciones de versión mínima de libxml2/libxslt en
  `build.rs` (pkg-config) solo se probaron en macOS; confirmarlas en la CI.
- **Revisión independiente** de la API de todos los crates, y de seguridad de
  `ws`, `gre-api`, `xml-parser` y `report`, antes de declarar una versión
  estable. Una primera revisión interna (2026-10-20) corrigió el análisis
  cuadrático de atributos y la copia de ámbitos de espacios de nombres antes
  de `roxmltree` (`check_tree_markup`), los límites de `report` (decimales,
  HTML, QR, MIME) y los segmentos `.`/`..` en la API GRE.
- **cpeforge-ws: `CacheKey` por defecto.** `CacheKey::ClientId` comparte el
  token entre ambientes o usuarios con el mismo `client_id` en una tienda
  común; pasar a `CacheKey::Scoped` por defecto cambia el formato de clave
  compartido con otras implementaciones (decidir antes de la versión estable).
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
