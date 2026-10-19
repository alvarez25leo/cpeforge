# Inventario previo de `greenter/lite` v5.3.0

Referencia inmutable: `target/php-reference/v5.3.0`, commit `f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`. Cinco archivos fuente, seis clases `*Test.php`, dos clases base. Consumidores: la aplicación invoca `See` o `Api`; `FeFactory` satisface `core/FactoryInterface`; los resolvers consumen modelos `core`, builders `xml` y servicios `ws`. No se edita `greenter-php/`.

| Archivo y operación pública | Entrada | Salida y efecto; error PHP | Composición Rust y verificación prevista |
| --- | --- | --- | --- |
| `FeFactory` getters/setters | `BuilderInterface?`, `SignedXml?`, `SenderInterface?` | Estado mutable; getters pueden devolver `null` | `FeFactory`, dobles de builder/firma/sender; comprobar sustitución y estado |
| `FeFactory::getXmlSigned` | Documento | `builder.build` → `signer.signXml`; `lastXml` cambia solo tras firma exitosa; errores se propagan | `xml` + `xmldsig`, comparador de XML y firma independiente |
| `FeFactory::send` | Documento | `getXmlSigned` → `getName` → `sender.send`; fallo de sender deja `lastXml` nuevo | `FeFactory::send`; dobles y servidor local |
| `FeFactory::sendXml` | Nombre y XML anulables | Llama al sender sin construir ni firmar; `lastXml` intacto | `FeFactory::send_xml`, comparar bytes recibidos |
| `XmlBuilderResolver::find` | Nombre completo de clase | `Reversion` usa `VoidedBuilder`; otros usan nombre corto + `Builder`; clase inexistente produce error de clase | Enum tipado; `xml` builders; caso desconocido explícito |
| `WsSenderResolver::find` | Nombre completo de clase | `Summary`, `Voided`, `Reversion` → `SummarySender`; todo otro tipo → `BillSender`; inyecta cliente y proveedor de códigos | Selector tipado + servicios `ws`; desconocido cae en rama bill solo al seleccionar sender |
| `See::new`/`getFactory` | Ninguna | Crea SOAP, signer y factory; expone factory mutable | Inyección de transporte y composición; la mutación posterior se observa |
| `See::setBuilderOptions`/`setCachePath` | Opciones; ruta anulable | Mezcla sobre `autoescape=false`; ruta vacía o `null` desactiva caché | `RenderOptions::lite`; opciones soportadas y diferencia de caché documentada |
| `See::setCertificate` | PEM | Configura signer; error de PEM aparece al firmar | `xmldsig`; PEM efímero y Debug redactado |
| `See::setCredentials`/`setClaveSOL` | Usuario/clave o RUC+usuario/clave | WSSE username concatena RUC y usuario | `SoapClient`; servidor local verifica cabecera sin guardar secretos |
| `See::setService`/`setCodeProvider` | URL opcional/proveedor | Reconfigura destino; proveedor modifica descripciones de error | Cliente `ws` y adaptador `ErrorCodeProvider` en `lite`; fault SOAP sintético con código `0100`. `setService(null)` PHP aún no representado. |
| `See::getXmlSigned` | Documento | Resolver builder, construir y firmar, sin enviar | `FeFactory`; XML previo/posterior, XSD y verificador firma |
| `See::send` | Documento | Resolver builder+sender, construir, firmar, enviar; `lastXml` como factory | `FeFactory` + `ws`; ZIP/CDR/ticket SOAP local |
| `See::sendXml` | Tipo de clase, nombre, XML | Resuelve builder incluso sin usarlo; envía bytes tal cual; `lastXml` intacto | Selector + `ws`; comparar bytes y error de tipo desconocido |
| `See::sendXmlFile` | XML como cadena | DOM → tipo/nombre desde `ws` → `sendXml`; XML malformado genera fallo DOM | `ws::cdr::{resolve_type,extract_filename}`; DTD rechazado deliberadamente |
| `See::getStatus` | Ticket anulable | `ExtService` SOAP; resultado o error | `ws::ExtService`; códigos 98/99/0 y fault local |
| `Api::new` | Endpoints opcionales, factory/signer opcionales | Hosts auth/cpe predeterminados; crea Guzzle/store; sin red hasta operación | `ws::ApiFactory` inyectado, `gre-api` transporte; hosts fijados |
| `Api::setBuilderOptions` | Opciones | Mezcla opciones, devuelve `Api` | `RenderOptions`; opciones fuera del modelo Rust se registran |
| `Api::setApiCredentials`/`setClaveSOL` | Client ID/secreto, RUC+usuario/clave | Guarda cuatro campos, sin autenticar aún | `TokenRequest` y `Secret`; no exponer en Debug |
| `Api::setCertificate`/`getLastXml` | PEM/ninguna | Firma posterior; `lastXml` nullable | `xmldsig`; estado tras fallo de build/firma/envío |
| `Api::send` | Documento | Resolver/build/sign → asignar `lastXml` → `sendXml`; fallo remoto conserva XML firmado | Builders + signer + `ws::GreSender`; XML, ZIP/HTTP, resultado |
| `Api::sendXml` | Nombre, XML | Crea sender/token y envía XML **sin firmar**; no cambia `lastXml` | `ws::ApiFactory` + `GreSender`; comparar bytes, hash y token |
| `Api::getStatus` | Ticket nullable | Crea sender/token y consulta | `GreSender::status`; ticket, CDR, errores locales |

## Auditoría de pruebas PHP

| Clase | Riesgo y decisión |
| --- | --- |
| `ApiTest` | `@group integration`; GRE externo y PEM upstream. Excluir. |
| `FeFactoryTest` | `@group integration`; helpers con WSDL/endpoints remotos. Excluir. |
| `CeFactoryTest` | `@group integration`; SOAP externo y PEM upstream. Excluir. |
| `SeeFeTest` | `@group integration`; FE beta y PEM upstream. Excluir. |
| `SeeCeTest` | `@group integration`; retención beta y PEM upstream. Excluir. |
| `FeFactoryXmlTest` | Sin marca; sender simulado, pero `FeFactoryBase` lee `tests/Resources/SFSCert.pem`. Allowlist explícita de sus cinco métodos mediante `scripts/lite_upstream_safe.sh`: copia temporal de dos PHP tests, sustituye solo esa ruta por un PEM efímero y ejecuta PHPUnit sin red. **5/5, 13 assertions**. |
| `FeFactoryBase`, `CeFactoryBase` | No son tests autónomos; métodos auxiliares crean SOAP a WSDL remoto. Nunca invocarlos sin inspección individual. |

Allowlist de la suite upstream: `FeFactoryXmlTest::{testInvoiceXml,testCreditNoteXml,testDebitNoteXml,testResumenXml,testBajaXml}`. Se ejecutó con `sandbox-exec` denegando red y PEM efímero. Las otras cinco clases permanecen excluidas hasta contar con sustitución local segura de sus servicios; la suite completa continúa como puerta pendiente. Ningún PEM upstream se copia a fixtures.

### Inventario de los 33 métodos `test*` upstream

| Clase | Métodos inspeccionados | Consumo/riesgo y estado |
| --- | --- | --- |
| `FeFactoryXmlTest` (5) | `testInvoiceXml`, `testCreditNoteXml`, `testDebitNoteXml`, `testResumenXml`, `testBajaXml` | XML firmado + sender falso; los cinco ejecutados con PEM efímero y red denegada. |
| `FeFactoryTest` (11) | `testInvoice`, `testGetXmlSigned`, `testInvalidInvoice`, `testInvoiceNotValidZipFileName`, `testInvoiceRechazado`, `testNotaCredito`, `testNotaDebito`, `testResumen`, `testBaja`, `testStatusResumenV2`, `testStatus` | `FeFactoryBase` puede crear SOAP con WSDL/endpoint externo; clase `integration`. No ejecutados; los casos XML/envío/estado tienen contrapartes sintéticas parciales. |
| `CeFactoryTest` (7) | `testRetention`, `testGetXmlSigned`, `testPerception`, `testPerceptionNotValidRuc`, `testReversion`, `testStatus`, `testStatusInvalidTicket` | Helpers de retención/percepción con endpoint externo; clase `integration`. No ejecutados; faltan rutas lite de estas familias. |
| `SeeFeTest` (6) | `testSendInvoiceV21`, `testSendSummary`, `testGetStatus`, `testGetXmlSigned`, `testSendXml`, `testSendXmlFile` | SOAP FE beta y PEM upstream; clase `integration`. No ejecutados; oráculo local cubre las seis operaciones funcionales, no todas sus variantes. |
| `SeeCeTest` (3) | `testSendBill`, `testSendReversion`, `testGetXmlSigned` | SOAP retención beta; clase `integration`. No ejecutados. |
| `ApiTest` (1) | `testSend` | API GRE remota y PEM upstream; clase `integration`. No ejecutado; `Api::send` se ejercita con `CpeApiInterface` simulado. |
