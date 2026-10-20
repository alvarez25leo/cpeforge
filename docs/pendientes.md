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

- **Revisión independiente** de seguridad y de API de todos los crates antes de
  declarar una versión estable. Prioridad: `cpeforge-xmldsig` (DOM, C14N,
  evaluador XPath, verificación de referencias) y el FFI de
  `cpeforge-cpe-validator` (`csrc/glue.c`, `src/ffi.rs`) y
  `cpeforge-ubl-validator` (`src/ffi.rs` y límites).
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
- **GitHub.** Añadir los topics del repositorio (hoy vacíos): `rust`, `sunat`,
  `electronic-invoicing`, `peru`, `ubl`, `xml`, `xml-signature`, `cpe`, `sdk`.
