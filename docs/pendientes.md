# Pendientes por módulo

Estado al 20 de octubre de 2026. Los 13 crates compilan y sus pruebas pasan
(`cargo test --workspace --all-features`), pero ninguno se considera estable
todavía.

Los 13 crates `cpeforge` y `cpeforge-*` están publicados en crates.io: 0.1.0,
salvo `cpeforge-core` y `cpeforge-xml`, en 0.1.1 (19 de octubre de 2026). Los
antiguos `greenter-rs-*` ya no existen en crates.io.

## Resuelto

| Fecha | Crate | Qué se resolvió |
| --- | --- | --- |
| 2026-10-19 | `cpeforge-xml` 0.1.1 | Los cuatro casos de XML que no validaban contra XSD: cantidades y porcentajes con exponente (`1.0E-6`), `MultiplierFactorNumeric` vacío en cargos sin factor, importes desde `1e14` con ceros de relleno, varios `cac:Payment` por comprobante (ahora `XmlError::MultiplePayments`). Registrados en `tests/reference/xml/differences.json` con motivo `conformidad-xsd` o `rechazo-explicito`. |
| 2026-10-19 | `cpeforge-xml` 0.1.1 | Pánico de `n_format` con importes enormes y decimales (por ejemplo `1e23` con 10 decimales). |
| 2026-10-20 | `cpeforge-core` | Pruebas de `NaN`/`Inf`: los importes son `Decimal` y se rechazan los textos `NaN`, `inf`, `Infinity`, `1e400` y todo `f64`; `int_cast` devuelve `0` con textos no finitos. Solo pruebas, sin cambio publicable. |
| 2026-10-20 | `cpeforge-gre-api` | Prueba de integración local: `offline_parity` corre en `cargo test` contra un servidor HTTP en proceso (159 casos de `tests/reference/gre-api`); no hay prueba manual contra SUNAT. |
| 2026-10-20 | `cpeforge-cpe-validator` | Documentado en su README que las hojas deben llamar a `CPEForge\Validator\Xml\XsltFunctions::matches`. |
| 2026-10-20 | CI | El job `security` (`scripts/release_readiness.py --ci`) fallaba desde el commit `646e24b` porque el motivo `conformidad-xsd` no estaba en la lista del script; añadido y `oracle/compatibilidad.md` regenerado. |

## Pendiente

### Código y pruebas

- **cpeforge-ws.** En compilación debug, `roxmltree` usa unos 17 KB de pila
  por nivel: un CDR o una respuesta SOAP de más de ~120 niveles desborda la
  pila de 2 MiB de un hilo de Tokio y aborta el proceso, aunque el límite
  configurado (`MAX_XML_DEPTH`, `MAX_SOAP_DEPTH`) es 256. En release aguanta
  ~3 100 niveles. Opciones: bajar el límite (los CDR reales no pasan de unos
  15 niveles), un límite menor solo con `debug_assertions`, o analizar en un
  hilo con pila propia. La prueba actual usa un hilo de 64 MiB
  (`tests/hardening.rs`).
- **cpeforge-ws.** Ampliar las pruebas de integración locales.
- **cpeforge.** Cubrir todas las familias de documentos, errores y CDR contra
  un servidor local: `facade.rs` y `robustness.rs` cubren sobre todo facturas;
  `local_contract.rs` está ignorada (requiere `scripts/cpeforge_local_contract.sh`).
- **cpeforge-xml-parser.** Validar contra XSD los XML de prueba (no hay prueba
  de esquema como la de `cpeforge-xml`).
- **cpeforge-cpe-validator.** Probar con las hojas de reglas SUNAT reales.
- **cpeforge-xmldsig.** Ampliar la verificación externa (contra otra
  implementación) de referencias por `Id`, transformación XPath, SHA-384/512,
  RIPEMD-160 y HMAC; el código los admite, falta el contraste.
- **PDF.** No existe generador de PDF: la representación impresa
  (`cpeforge-report`) termina en HTML.

### Decisiones

- **cpeforge-xml.** ¿Más estrategias de escape (`js`, `css`, `url`) o solo
  `html`/desactivado?
- **cpeforge-xml-parser.** Ampliar `XmlReader` a XPath 1.0 completo
  (funciones, predicados, ejes) o fijar el subconjunto actual como contrato
  (fila `CORREGIR` en `oracle/compatibilidad.md`).
- **cpeforge-report.** Alcance de las plantillas propias: solo el dialecto
  MiniJinja o añadir la sintaxis de Twig que falta; validar al cargarlas las
  funciones y filtros desconocidos (dos filas `CORREGIR`); aprobar la
  conversión de importes a `f64` dentro de la plantilla.
- **cpeforge-ubl-validator.** Derechos de redistribución de seis XSD
  (empaquetarlos o mantenerlos externos). Ante XML vacío hoy devuelve
  `InfrastructureError::EmptyInput`, como la referencia: confirmar que es el
  contrato.
- **cpeforge-xmldsig.** Aprobar la API `add_509_cert` sin URL remota.
- **cpeforge-xcodes.** Verificar la vigencia del catálogo de 1 710 códigos
  frente a SUNAT y definir cómo se mantendrá actualizado.
- **cpeforge-cpe-validator.** Límite de coste XPath con la libxml2 de Apple en
  macOS < 13.

### Revisión y repositorio

- **Revisión independiente** de seguridad y de API de todos los crates antes de
  declarar una versión estable. Prioridad: `cpeforge-xmldsig` (DOM, C14N,
  evaluador XPath, verificación de referencias) y el FFI de
  `cpeforge-cpe-validator` (`csrc/glue.c`, `src/ffi.rs`) y
  `cpeforge-ubl-validator` (`src/ffi.rs` y límites).
- **cpeforge** se declara estable después de `core`, `xml`, `xmldsig`,
  `gre-api` y `ws`.
- **GitHub.** Añadir los topics del repositorio (hoy vacíos): `rust`, `sunat`,
  `electronic-invoicing`, `peru`, `ubl`, `xml`, `xml-signature`, `cpe`, `sdk`.

## Qué hace cada crate

| Crate | Qué hace |
| --- | --- |
| `cpeforge-core` | Modelos de comprobantes (factura, boleta, notas, guía, resumen, baja, reversión, retención, percepción), emisor/receptor, respuestas de SUNAT, importe en letras y contratos para builders, envío, parseo, reportes y validación. |
| `cpeforge-xml` | XML UBL sin firmar de los 13 tipos de documento (UBL 2.0/2.1, guía 2022, resúmenes, bajas, retención y percepción), válido contra el XSD. |
| `cpeforge-xmldsig` | Firma y verificación XMLDSig con el perfil SUNAT, C14N, X.509, XMLEnc y política de confianza. |
| `cpeforge-gre-api` | Cliente HTTP de la API GRE de SUNAT: token OAuth2, envío y consulta. |
| `cpeforge-ws` | SOAP con WS-Security, ZIP, lectura de CDR, clasificación del resultado tributario y adaptador GRE con caché de token. |
| `cpeforge` | Fachadas `See` (SOAP) y `Api` (GRE) que construyen, firman, envían y leen el CDR. |
| `cpeforge-validator` | Reglas de validación de los modelos (33 loaders base y 7 variantes UBL 2.1), con mensajes traducibles y reglas extensibles. |
| `cpeforge-xcodes` | Catálogo fijo de 1 710 códigos de error de SUNAT. |
| `cpeforge-xml-parser` | Lectura de XML de comprobantes hacia los modelos de `core`, con perfil XML seguro y `XmlReader`. |
| `cpeforge-xml-limits` | Límite de anidamiento XML comprobado sobre el texto antes de analizarlo. Sin pendientes más allá de la revisión. |
| `cpeforge-cpe-validator` | Validación con las hojas XSL de reglas SUNAT (libxslt) y función de expresiones regulares PCRE. |
| `cpeforge-ubl-validator` | Validación XSD de documentos UBL 2.0/2.1 contra 45 esquemas fijados por SHA-256. |
| `cpeforge-report` | HTML de la representación impresa (siete plantillas), QR SVG y `DigestValue`. |
