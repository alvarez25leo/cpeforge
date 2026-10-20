# cpeforge-xmldsig

Firma y verificación XML (XMLDSig), canonicalización C14N 1.0 inclusiva y
exclusiva, claves y certificados X.509, y cifrado XML (XMLEnc) con el perfil
de firma de comprobantes SUNAT.

## Módulos

| Módulo | Contenido |
| --- | --- |
| `sunat` | `SignedXml`: firma envuelta en `ext:ExtensionContent` (`Id="CPEForgeSign"`) |
| `dsig` | Firma y verificación XMLDSig genérica |
| `key` | Claves RSA/HMAC/AES y utilidades Base64 |
| `enc` | Cifrado y descifrado XMLEnc |
| `x509` | Lectura de certificados X.509 y `X509ContentType` |
| `xpath` | Consultas XPath 1.0 acotadas (la transformación está en `c14n::canonicalize_xpath`) |
| `c14n`, `dom` | Canonicalización y DOM propio |
| `trust` | Política de confianza en el certificado |

## Seguridad

- **Perfil por defecto RSA-SHA1 / SHA-1**, el que históricamente acepta
  SUNAT. SHA-1 ya no se considera resistente a colisiones: preferir
  `RSA_SHA256` y `SHA256` con `set_signature_algorithm` y
  `set_digest_algorithm` cuando el receptor lo admita. Fuera del perfil
  SUNAT, `XmlSecurityDsig::reject_sha1` rechaza al verificar un digest SHA-1
  y una firma RSA-SHA1 o HMAC-SHA1.
- **Validez matemática ≠ confianza.** `SignedXml::verify_xml` comprueba firma
  y digest con el certificado incluido en el documento y acepta certificados
  vencidos o autofirmados. Para decidir si el firmante es confiable usar
  `trust::TrustPolicy` con raíces explícitas (valida cadena, vigencia,
  `keyUsage` de firma y, si se indican, los `extendedKeyUsage` aceptados; no
  consulta CRL/OCSP).
- **Cifrado XML.** `XmlSecEnc` solo acepta AES-GCM (XML Encryption 1.1) y
  RSA-OAEP, y todo fallo de descifrado es `Error::DecryptionFailed`, sin
  detalle que sirva de oráculo. AES-CBC, 3DES y RSA PKCS#1 v1.5 (los de la
  referencia) requieren `allow_legacy_algorithms`.
- **XPath y recursión acotados.** La transformación XPath corta tras recorrer
  10 millones de nodos y rechaza expresiones de más de 16 384 caracteres, con
  más de 32 niveles de anidamiento o 128 operadores. La resolución de
  `KeyInfo` limita a 8 el anidamiento de `EncryptedKey`.
- **Entradas hostiles.** Se rechazan DTD y entidades no predefinidas (sin XXE
  ni expansión de entidades), solo se acepta UTF-8 y se limita tamaño
  (10 MiB), profundidad (256), nodos (1.000.000) y el trabajo de namespaces.
  Las referencias externas no se descargan nunca.
- **Wrapping.** Por defecto una referencia `#id` usa el primer elemento con
  ese `Id`. `XmlSecurityDsig::reject_duplicate_ids` y
  `SignedXml::set_reject_duplicate_ids(true)` rechazan la referencia si el
  `Id` es ambiguo. Después de verificar, comprobar con `validated_nodes()`
  que el nodo firmado es el que se va a usar.
- **Material sensible.** `Debug` de `SignedXml` y `XmlSecurityKey` no muestra
  PEM ni contraseñas; PEM, contraseñas, PKCS#12 y claves simétricas se borran
  de memoria al soltarse (`zeroize`).
- **Criptografía:** OpenSSL mediante el crate `openssl` (sin criptografía
  propia).
- **Contraste externo.** `tests/external.rs` verifica las firmas fuera del
  crate: `xmllint` (C14N de libxml2) y `openssl` recalculan el digest y la
  firma de referencias `URI=""` enveloped (RSA-SHA1/256/384/512, SHA-1/256/
  384/512, RIPEMD-160) y por `Id` con C14N exclusiva (RSA-SHA512, HMAC-SHA1).
  La transformación XPath solo se prueba internamente.
- **Sin red.** `XmlSecurityDsig::add_509_cert` recibe el certificado como
  texto (PEM o base64); la carga por URL de la referencia no existe y no se
  añadirá: el crate nunca descarga nada.

## Rendimiento

`cargo run --release -p cpeforge-xmldsig --example bench` (RSA-2048, un hilo,
Apple Silicon, mediana de 3 ejecuciones):

| Operación | Tiempo |
| --- | --- |
| `verify_xml`, factura de 5 líneas (2 KiB) | 64 µs |
| `sign_xml`, factura de 5 líneas | 359 µs |
| `verify_xml`, 2000 líneas (691 KiB) | 6,5 ms |
| `sign_xml`, 2000 líneas | 8,3 ms |
| C14N inclusiva, 2000 líneas | 2,3 ms |

## Ejemplo

```bash
cargo run -p cpeforge-xmldsig --example sign_verify_invoice
```

Requiere OpenSSL 3 (`OPENSSL_DIR` si no lo encuentra `pkg-config`).

## Estado

Versión 0.1.1 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
