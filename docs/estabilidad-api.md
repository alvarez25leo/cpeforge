# Estabilidad de la API

Reglas que siguen los 13 crates para que una versión menor no rompa el
código de quien los usa. Las comprueban el compilador, las pruebas
`tests/api_traits.rs` de cada crate y los lints del workspace.

## Tipos que pueden crecer (`#[non_exhaustive]`)

Añadir una variante o un campo a estos tipos es compatible:

- **Errores y clasificaciones**: los enums de error de cada crate
  (`CoreError`, `XmlError`, `ApiError`, `WsError`, `CpeForgeError`,
  `XmlParserError`, `ReportError`, `ValidatorError`, `CatalogError`,
  `CpeValidatorError`, `InfrastructureError`, `MarkupRejection`…) y sus
  enums de detalle (`TransportErrorKind`, `LimitKind`, `AccessKind`,
  `RenderCause`…).
- **Resultados y variantes**: `SendResult`, `DocumentKind`, `SenderKind`,
  `DocumentRef`, `VoidedDocument`, `TrustResult`, `SignatureCheck`,
  `Encrypted`, `Decrypted`, `KeyType`, `NodeKind`, `Transform`,
  `ObjectData`, `ZipTime`, `Method` (HTTP), `Number`…
- **Opciones con `Default`**: `xml_parser::Limits`, `cpe_validator::Limits`,
  `RenderOptions`, `ReportOptions`, `XmlUtils`, `BuilderOptionsPatch`,
  `xmldsig::dom::ParseOptions`, `ReferenceOptions`, `CertOptions`,
  `c14n::Method`. Se crean con `Default` (o un constructor) y se ajustan
  campo a campo o con los métodos `with_*`:

  ```rust
  use cpeforge_xml_parser::Limits;

  let mut limits = Limits::default();
  limits.max_bytes = 2 * 1024 * 1024;
  ```

- **Salidas de la biblioteca**: `Violation`, `CpeError` (validador XSL),
  `EngineInfo`, `Validation`, `ubl_validator::XmlError`, `ZipEntry`,
  `QrSymbol`, `ParsedDateTime`, `ApiResponse`, `CertInfo`, `Purpose`,
  `ValidatedNode` y los nodos del DOM de `xmldsig` (`Element`, `Attribute`,
  `NsDecl`).

Un `match` sobre estos enums necesita una rama `_`.

## Modelos de datos (exhaustivos)

Los construye quien usa la biblioteca y reflejan un esquema fijado (UBL y
SUNAT, el OpenAPI de GRE):

- los modelos de `cpeforge_core::model` (`Invoice`, `Despatch`, `Company`…)
  y los resultados `BillResult`, `SummaryResult`, `StatusResult`,
  `CdrResponse`, `ResponseError`;
- los DTO de `cpeforge_gre_api::model` y `TokenRequest`;
- los tipos que crea quien implementa un puerto: `HttpRequest`,
  `HttpResponse`, `TransportError` (`HttpTransport`), `CachedToken`
  (`TokenStore`), `SoapStatus` (`WsClient`);
- los parámetros de `cpeforge-report` (`ReportParameters` y sus partes) y
  `SalePayloadParts`.

Se construyen con `..Default::default()`. Añadirles un campo es un cambio
incompatible y solo se hace en una versión mayor.

## Dependencias públicas

Aparecen en la API y su versión mayor forma parte de la de los crates:
`jiff` (fechas, reexportado como `cpeforge_core::Zoned`), `rust_decimal`
(importes, `cpeforge_core::Decimal`), `serde_json::Value` (entradas y
valores de `cpeforge-validator`) y `serde` con la característica `serde`.
Ningún tipo de `roxmltree`, `openssl`, `reqwest`, `minijinja` ni `zip`
aparece en la API pública.

## Hilos y errores

- Los clientes, validadores, firmadores, claves, documentos y el transporte
  son `Send + Sync`. La excepción es `cpeforge_cpe_validator::XmlDocument`
  (un documento de libxml2): es `Send` pero no `Sync`.
- Todos los errores implementan `std::error::Error + Send + Sync + 'static`,
  y su `Display`/`Debug` no muestra secretos ni datos de la respuesta.
- Todo tipo público implementa `Debug` (lint `missing_debug_implementations`).

## Versión mínima de Rust

`rust-version` del workspace (1.85). Subirla es un cambio de versión menor.
