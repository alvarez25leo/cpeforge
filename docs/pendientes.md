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

- **Revisión independiente** de la API de todos los crates, y de seguridad de
  los que no entraron en la primera revisión (`ws`, `gre-api`, `xml-parser`,
  `report`), antes de declarar una versión estable.
- **Revisión de seguridad 2026-10-19** (`cpeforge-xmldsig` y el FFI de los dos
  validadores), sin publicar. Corregido, con regresiones en
  `xmldsig/tests/review_hardening.rs` y `*-validator/tests/review_prescan.rs`:
  - `xmldsig`: un certificado de `KeyInfo` ya no se acepta como secreto HMAC
    (confusión de algoritmo); `SignedXml` exige el perfil SUNAT (una sola
    `Reference` `URI=""` con la transformación enveloped), no usa claves de
    `KeyInfo` sin certificado y no cambia el firmante al verificar; una
    referencia sin destino nunca valida; tope de 32 referencias; una clave
    simétrica ya no firma en vacío; las variantes `&str` exigen UTF-8.
  - `xmldsig`, DoS: atributos duplicados, herencia de `xml:*` en C14N y
    subárbol del apex en tiempo lineal; el evaluador XPath cobra
    comparaciones de conjuntos, `string()` e `id()`; prefijos sin declarar y
    referencias de carácter con signo se rechazan.
  - Validadores: preescaneo lineal en Rust antes de libxml2 (tope de 256
    atributos por elemento, que libxml2 < 2.12 compara en tiempo cuadrático;
    cualquier `<!…>` que no sea comentario o CDATA se rechaza, también en
    UTF-16; UTF-32/UCS-4/EBCDIC se rechazan). `schema.c` usa la firma `const`
    del callback de errores de libxml2 ≥ 2.12; `ubl-validator` inicializa
    libxml2 una sola vez; cada transformación desactiva XInclude.
  - Cuatro diferencias deliberadas nuevas (motivo `seguridad`) en
    `tests/reference/xmldsig/differences.json`.
- **Riesgos aceptados o pendientes de esa revisión:**
  - XML Encryption (`xmldsig::enc`): AES-CBC sin autenticación, RSA-1.5 y
    3DES admitidos, y los errores de descifrado distinguen relleno de clave
    (oráculo). No está en el camino SUNAT; antes de exponerlo, solo AES-GCM y
    RSA-OAEP y un error único.
  - Sin piso de algoritmos: SUNAT exige RSA-SHA1/SHA-1. Evaluar una opción
    para rechazar SHA-1 fuera del perfil SUNAT.
  - `XmlSecurityDsig::reject_duplicate_ids` es `false` por defecto en la API
    de bajo nivel (paridad); `SignedXml` ya no depende de él.
  - `TrustPolicy` no comprueba `keyUsage`/`extendedKeyUsage` del certificado.
  - Las claves de sesión descifradas no se borran de memoria (`zeroize`).
  - En macOS la libxml2 del sistema no aplica el límite de operaciones XPath
    (`Limits::xpath_operations`); en Linux sí (sonda en tiempo de ejecución).
  - No se comprueba una versión mínima de libxml2/libxslt al compilar ni al
    iniciar; < 2.11 tiene la carrera de `xmlInitParser` si los dos validadores
    se inicializan a la vez en el mismo proceso (cada uno usa su `Once`).
  - El preescaneo está duplicado en los dos validadores (`src/prescan.rs`);
    moverlo a `cpeforge-xml-limits` en la próxima publicación de ese crate.
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
- **GitHub.** Añadir los topics del repositorio (hoy vacíos): `rust`, `sunat`,
  `electronic-invoicing`, `peru`, `ubl`, `xml`, `xml-signature`, `cpe`, `sdk`.
