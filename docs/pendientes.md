# Pendientes

## Código y pruebas

- **cpeforge-cpe-validator.** Probar con las hojas de reglas SUNAT reales
  («Archivo XSL – 2.1», en <https://cpe.sunat.gob.pe/guias-y-manuales>).
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
  estable.
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
