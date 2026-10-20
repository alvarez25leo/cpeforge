# Pendientes por módulo

Estado al 19 de octubre de 2026. Los 13 crates compilan y sus pruebas pasan
(`cargo test --workspace --all-features`), pero ninguno se considera estable
todavía. Cada módulo indica qué hace y qué le falta.

Los 13 crates `cpeforge` y `cpeforge-*` 0.1.0 están publicados en crates.io.
El CI del commit `9720dae` terminó correctamente en
[GitHub Actions](https://github.com/alvarez25leo/cpeforge/actions/runs/39195271285).

## Pendientes del proyecto

- **Repositorio.** Añadir los topics de GitHub: `rust`, `sunat`,
  `electronic-invoicing`, `peru`,
  `ubl`, `xml`, `xml-signature`, `cpe`, `sdk`.
- **Revisión independiente** de seguridad y de API de todos los crates antes
  de declarar una versión estable.
- **PDF.** No existe generador de PDF (la representación impresa termina en
  HTML).

## cpeforge-core

Modelos de comprobantes (factura, boleta, notas, guía, resumen, baja,
reversión, retención, percepción), emisor/receptor, respuestas de SUNAT y
contratos para builders, envío, parseo, reportes y validación.

Falta: casos de prueba para `NaN`/`Inf` en importes y revisión independiente.

## cpeforge-xml

Genera el XML UBL sin firmar de los 13 tipos de documento (UBL 2.0/2.1, guía
2022, resúmenes, bajas, retención y percepción).

Corregido (sin publicar, versión del workspace): los cuatro casos que no
validaban contra XSD (cantidades con exponente, `MultiplierFactorNumeric`
vacío, importes con relleno desde `1e14`, varios `cac:Payment` por
comprobante, que ahora devuelve `XmlError::MultiplePayments`) y el pánico de
`n_format` con importes enormes y decimales (p. ej. `1e23` con 10 decimales). Las diferencias
con la referencia están en `tests/reference/xml/differences.json` (motivo
`conformidad-xsd` y `rechazo-explicito`).

Falta: publicar la versión corregida; decidir si se soportan más estrategias
de escape (`js`, `css`, `url`) o solo `html`/desactivado; revisión
independiente.

## cpeforge-xmldsig

Firma y verificación XMLDSig con el perfil SUNAT, C14N, X.509, XMLEnc y
política de confianza.

Falta: revisión de seguridad humana del DOM, la C14N, el evaluador XPath y la
verificación de referencias; ampliar la verificación externa (referencias por
`Id`, transformación XPath, SHA-384/512, RIPEMD-160, HMAC); aprobar la API
`add_509_cert` sin URL remota.

## cpeforge-gre-api

Cliente HTTP de la API GRE de SUNAT: token OAuth2, envío y consulta.

Falta: una prueba de integración local que sustituya la prueba manual contra
SUNAT; revisión independiente.

## cpeforge-ws

SOAP con WS-Security, ZIP, lectura de CDR, clasificación del resultado
tributario y adaptador GRE con caché de token.

Falta: evitar el desbordamiento de pila en compilación debug a partir de
~125 niveles de anidamiento (el límite configurado es 256); ampliar las pruebas
de integración locales; revisión independiente.

## cpeforge

Paquete principal: fachadas `See` (SOAP) y `Api` (GRE) que construyen,
firman, envían y leen el CDR.

Falta: cubrir todas las familias de documentos, errores y CDR contra un
servidor local; revisión independiente. Depende de que `core`, `xml`,
`xmldsig`, `gre-api` y `ws` se declaren estables.

## cpeforge-validator

Reglas de validación de los modelos (33 loaders base y 7 variantes UBL 2.1),
con mensajes traducibles y reglas extensibles.

Falta: revisión independiente.

## cpeforge-xcodes

Catálogo fijo de 1 710 códigos de error de SUNAT.

Falta: verificar la vigencia del catálogo frente a SUNAT y definir cómo se
mantendrá actualizado; revisión independiente.

## cpeforge-xml-parser

Lectura de XML de comprobantes SUNAT hacia los modelos de `core`, con perfil
XML seguro y `XmlReader`.

Falta: ampliar `XmlReader` a XPath 1.0 completo (funciones, predicados, ejes)
o fijar el subconjunto actual como contrato; validar contra XSD los XML de
prueba; revisión independiente.

## cpeforge-xml-limits

Límite de anidamiento XML comprobado sobre el texto antes de analizarlo.

Falta: nada específico más allá de la revisión independiente.

## cpeforge-cpe-validator

Validación con las hojas XSL de reglas SUNAT (libxslt) y función de
expresiones regulares PCRE.

Falta: probar con las reglas SUNAT reales; documentar que las hojas deben
llamar a `CPEForge\Validator\Xml\XsltFunctions::matches`; límite de coste XPath
con la libxml2 de Apple en macOS < 13; revisión de `csrc/glue.c` y
`src/ffi.rs`.

## cpeforge-ubl-validator

Validación XSD de documentos UBL 2.0/2.1 contra 45 esquemas fijados por
SHA-256.

Falta: decidir los derechos de redistribución de seis XSD (para poder
empaquetarlos) o mantenerlos externos; definir el comportamiento ante XML
vacío; revisión del FFI y de los límites.

## cpeforge-report

HTML de la representación impresa (siete plantillas), QR SVG y `DigestValue`.

Falta: decidir el alcance de las plantillas propias (solo dialecto MiniJinja o
añadir la sintaxis que falta) y validar al cargarlas las funciones y filtros
desconocidos; aprobar la conversión de importes a `f64` en la plantilla;
generación de PDF; revisión independiente.
