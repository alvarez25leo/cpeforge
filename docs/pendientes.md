# Pendientes

## Código y pruebas

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

## Decisiones

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
