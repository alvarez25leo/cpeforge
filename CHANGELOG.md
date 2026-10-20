# Cambios

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
Los crates siguen SemVer: en la serie 0.x, subir la versión menor (0.1 → 0.2)
indica cambios incompatibles. Las reglas de compatibilidad están en
[docs/estabilidad-api.md](docs/estabilidad-api.md).

## 0.2.1 — 2026-10-20

`cpeforge-xmldsig`, `cpeforge-ws` y `cpeforge` 0.2.1, compatibles con 0.2.0.

### Corregido

- **`cpeforge-xmldsig`: compila con la versión mínima declarada (Rust
  1.85).** Las versiones 0.1.1 y 0.2.0 usaban `if let … && let …` (estable
  desde Rust 1.88) y fallaban al compilar con Rust 1.85–1.87 pese a declarar
  `rust-version = "1.85"`. Con esas versiones, usar Rust 1.88 o posterior.
- La documentación de `ApiFactory`, `cpeforge::Api` y los README de
  `cpeforge` y `cpeforge-ws` explican la clave de caché `CacheKey::Scoped`
  y enlazan la guía de migración.

## 0.2 — 2026-10-20

| Crate | Versión |
| --- | --- |
| `cpeforge`, `cpeforge-xml`, `cpeforge-xmldsig`, `cpeforge-gre-api`, `cpeforge-ws`, `cpeforge-validator`, `cpeforge-xml-parser`, `cpeforge-cpe-validator`, `cpeforge-ubl-validator`, `cpeforge-report` | 0.2.0 |
| `cpeforge-xcodes` | 0.3.0 |
| `cpeforge-xml-limits` | 0.1.2 (compatible) |
| `cpeforge-core` | 0.1.1 (sin cambios) |

### Migración desde 0.1

1. **`match` con rama `_`.** Los enums de error, resultado y clasificación
   son `#[non_exhaustive]` (`SendResult`, `DocumentKind`, `VoidedDocument`,
   `SignatureCheck`, `TrustResult`, `Encrypted`, `Decrypted`, `NodeKind`,
   `ErrorObject`…):

   ```rust,ignore
   match result {
       SendResult::Bill(bill) => { /* … */ }
       SendResult::Summary(summary) => { /* … */ }
       _ => { /* variante de una versión futura */ }
   }
   ```

2. **Opciones desde `Default`.** `xml_parser::Limits`,
   `cpe_validator::Limits`, `RenderOptions`, `XmlUtils`,
   `BuilderOptionsPatch`, `xmldsig::dom::ParseOptions`, `ReferenceOptions`,
   `CertOptions` y `c14n::Method` ya no se construyen con un literal
   (`Limits { max_bytes: 1, ..Limits::default() }` no compila). Se parte de
   `Default` y se cambian los campos, o se usan los constructores nuevos:

   ```rust,ignore
   let mut options = ReferenceOptions::default();
   options.force_uri = true;

   let render = RenderOptions::default().with_now(timestamp);
   let method = Method::new(true, false).with_inclusive_prefixes(vec!["ds".into()]);
   ```

3. **Funciones retiradas de la API pública.**
   `cpeforge_ubl_validator::resolve_version` (no la usaba nadie; la versión
   UBL se resuelve en el validador), `XmlUtils::hash_sign_from_document`
   (exponía `roxmltree`; usar `XmlUtils::hash_sign`) y
   `CertInfo::from_x509` (exponía `openssl`; usar
   `CertInfo::from_base64_body` o `X509Certificate`).

4. **Revisar la clave de la caché de tokens GRE** (sin error de compilación;
   ver «Cambios de comportamiento»).

### Cambios de comportamiento

Compilan igual que en 0.1 pero cambian el resultado:

- **`cpeforge-ws`: la clave de caché por defecto de `ApiFactory` es
  `CacheKey::Scoped`** (host de autenticación, `scope`, `client_id` y usuario
  SOL), antes `CacheKey::ClientId`. Una misma tienda (`InMemoryStore` o una
  propia) ya no comparte el token entre ambientes ni entre usuarios SOL con el
  mismo `client_id`, que es lo correcto para un servicio con varias empresas.
  Tras actualizar, los tokens guardados con la clave anterior no se encuentran
  y se pide uno nuevo una vez. Si la tienda se comparte con otra
  implementación que usa el `client_id` como clave (por ejemplo, un Redis
  común con Greenter en PHP), conservar el formato anterior con
  `.with_cache_key(CacheKey::ClientId)` en `ApiFactory` y usar una tienda por
  ambiente y usuario.
- **`cpeforge-report`: `XmlUtils` admite 64 niveles por defecto** (antes 256)
  y como máximo 1 000 000 de nodos (`xml_utils::MAX_NODES`). `max_depth` se
  puede subir si el hilo tiene pila suficiente.
- **`cpeforge-ws`: un ZIP recibido se rechaza antes de abrirlo** si algún
  registro de fin de directorio central declara más de 64 entradas o marca
  ZIP64 (los CDR de SUNAT tienen dos).
- **`cpeforge-gre-api` y `cpeforge-ws`: `client_id`, nombre de archivo o
  ticket iguales a `.` o `..` se rechazan** antes de enviar
  (`ApiError::InvalidRequest`, `WsError::Security`,
  `TransportErrorKind::InvalidUrl` en `ReqwestTransport`). Antes la URL se
  normalizaba y la solicitud, con el token, iba a otra ruta del host.
- **XML de entrada (`cpeforge-ws`, `cpeforge-xml-parser`,
  `cpeforge-report`)**: se rechazan, antes de analizar, los elementos con más
  de 256 atributos y los documentos con ámbitos de espacios de nombres
  desmedidos (`XmlParserError::TooManyAttributes` / `TooManyNamespaces`).
- **`cpeforge-report`**: `n_format` admite hasta 100 decimales, el HTML
  generado tiene un máximo de 32 MiB (`ReportOptions::max_output_bytes`), el
  QR rechaza contenidos mayores que la versión 40 antes de codificarlos e
  `image_data_uri` solo acepta subtipos MIME simples.

### Añadido

- `cpeforge-xml-limits`: `check_tree_markup` y
  `DEFAULT_MAX_NAMESPACE_SCOPE` (límites antes de `roxmltree`).
- `cpeforge-gre-api`: `ApiError::InvalidRequest`.
- `cpeforge-xml`: `RenderOptions::with_now`, `with_autoescape` y
  `with_strict_variables`.
- `cpeforge-xmldsig`: `c14n::Method::new` y `with_inclusive_prefixes`,
  `dom::XmlDeclaration::new`.
- `cpeforge-report`: `xml_utils::MAX_NODES`.
- Todos los tipos públicos implementan `Debug`; los tipos principales son
  `Send + Sync` y los errores `Error + Send + Sync + 'static`, comprobado en
  `tests/api_traits.rs` de cada crate.

### Seguridad

- Análisis cuadrático de atributos y amplificación de memoria por espacios de
  nombres en `roxmltree` (CDR, respuestas SOAP, `xml-parser`, `XmlUtils`).
- Segmentos de ruta `.`/`..` en la API GRE.
- ZIP con miles de entradas declaradas.
- La transformación XPath de `cpeforge-xmldsig` se contrasta con libxml2
  (`tests/external_xpath.rs`).
