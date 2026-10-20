# Pendientes

## Código y pruebas

- **cpeforge-ws.** Ampliar las pruebas de integración locales.
- **cpeforge.** Cubrir todas las familias de documentos, errores y CDR contra
  un servidor local: `facade.rs` y `robustness.rs` cubren sobre todo facturas;
  `local_contract.rs` está ignorada (requiere `scripts/cpeforge_local_contract.sh`).
- **cpeforge-cpe-validator.** Probar con las hojas de reglas SUNAT reales.
- **cpeforge-xmldsig.** Ampliar la verificación externa (contra otra
  implementación) de referencias por `Id`, transformación XPath, SHA-384/512,
  RIPEMD-160 y HMAC; el código los admite, falta el contraste.
- **PDF.** No existe generador de PDF: la representación impresa
  (`cpeforge-report`) termina en HTML.

## Decisiones

- **cpeforge-ubl-validator.** Derechos de redistribución de seis XSD
  (empaquetarlos o mantenerlos externos).
- **cpeforge-xcodes.** Verificar la vigencia del catálogo de 1 710 códigos
  frente a SUNAT y definir cómo se mantendrá actualizado.

## Revisión y repositorio

- **Publicar** `cpeforge-ws` y `cpeforge-report` 0.1.1: el límite de
  profundidad de 64 niveles y la validación de plantillas al cargarlas están en
  el repositorio, no en crates.io.
- **Revisión independiente** de seguridad y de API de todos los crates antes de
  declarar una versión estable. Prioridad: `cpeforge-xmldsig` (DOM, C14N,
  evaluador XPath, verificación de referencias) y el FFI de
  `cpeforge-cpe-validator` (`csrc/glue.c`, `src/ffi.rs`) y
  `cpeforge-ubl-validator` (`src/ffi.rs` y límites).
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
- **GitHub.** Añadir los topics del repositorio (hoy vacíos): `rust`, `sunat`,
  `electronic-invoicing`, `peru`, `ubl`, `xml`, `xml-signature`, `cpe`, `sdk`.
