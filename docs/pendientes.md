# Pendientes

## Código y pruebas

- **cpeforge-cpe-validator.** Probar con las hojas de reglas SUNAT reales
  («Archivo XSL – 2.1», en <https://cpe.sunat.gob.pe/guias-y-manuales>).
- **cpeforge-xmldsig.** Contrastar con otra implementación la transformación
  XPath (las referencias enveloped y por `Id`, los digests y las firmas RSA y
  HMAC ya se verifican con `xmllint` y `openssl` en `tests/external.rs`).
- **PDF.** Nuevo crate para generar el PDF de la representación impresa
  (`cpeforge-report` termina en HTML).

## Seguridad

- **XML Encryption** (`xmldsig::enc`): admite AES-CBC sin autenticación,
  RSA-1.5 y 3DES, y los errores de descifrado distinguen relleno de clave
  (oráculo). No está en el camino SUNAT; antes de exponerlo, limitarlo a
  AES-GCM y RSA-OAEP con un error único, y borrar de memoria las claves de
  sesión (`zeroize`).
- **Piso de algoritmos.** SUNAT exige RSA-SHA1/SHA-1; falta una opción para
  rechazar SHA-1 fuera del perfil SUNAT.
- **`TrustPolicy`** no comprueba `keyUsage`/`extendedKeyUsage` del certificado.
- **Versión mínima de libxml2/libxslt.** No se comprueba al compilar ni al
  iniciar; antes de 2.11 `xmlInitParser` no es seguro si los dos validadores
  se inicializan a la vez en el mismo proceso.
- **Preescaneo duplicado** en los dos validadores (`src/prescan.rs`): moverlo a
  `cpeforge-xml-limits` en la próxima publicación de ese crate.

## Revisión y repositorio

- **Publicar** `cpeforge-xmldsig`, `cpeforge-cpe-validator` y
  `cpeforge-ubl-validator` con la revisión de seguridad del 2026-10-19 (las
  versiones de crates.io no la tienen).
- **Revisión independiente** de la API de todos los crates, y de seguridad de
  `ws`, `gre-api`, `xml-parser` y `report`, antes de declarar una versión
  estable.
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
- **GitHub.** Añadir los topics del repositorio (hoy vacíos): `rust`, `sunat`,
  `electronic-invoicing`, `peru`, `ubl`, `xml`, `xml-signature`, `cpe`, `sdk`.
