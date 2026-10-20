# Pendientes

## Código y pruebas

- **PDF.** Nuevo crate para generar el PDF de la representación impresa
  (`cpeforge-report` termina en HTML).

## Revisión y repositorio

- **Linux.** Confirmar en la CI (ubuntu-24.04) las comprobaciones de versión
  mínima de libxml2/libxslt de `build.rs` (pkg-config); solo se han visto
  pasar en macOS.
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
