# Inventario de `greenter/ws` v5.3.0 antes del port

Fuente inmutable: `thegreenter/greenter` tag `v5.3.0`, commit
`f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`, checkout aislado
`target/php-reference/v5.3.0/`. Son **30 archivos PHP** en `packages/ws/src/`
más seis WSDL/XSD locales. `lite/See.php`, `lite/Api.php` y
`lite/Factory/WsSenderResolver.php` son consumidores, no parte del port.

Cada fila describe entrada, efecto/salida, error relevante, consumidor y
comprobación planeada. «Pendiente» significa que todavía no hay fixture PHP y
prueba Rust completos; no implica que la rama sea irrelevante.

| Archivo PHP | Contrato público; entrada → salida/error | Consumidor; caso y segundo verificador | Estado |
| --- | --- | --- | --- |
| `Zip/CompressInterface.php` | `compress(?filename, ?content): ?bytes` | `BaseSunat`; ZIP de XML, lector ZIP independiente | Pendiente |
| `Zip/DecompressInterface.php` | `decompress(?bytes, ?filter): ?array` de nombre/contenido | `BaseSunat`; entradas múltiples/corruptas, `unzip` | Pendiente |
| `Zip/ZipFly.php` | DEFLATE de una entrada; descompresión en orden de entradas con filtro, excepciones de `nelexa/zip` | Envío/CDR; vacío, corrupción, extensiones, ZIP malicioso | Pendiente |
| `Zip/ZipDecompressDecorator.php` | Vacío → `[]`; captura `Exception` del descompresor → `[]` | `BaseSunat`; ZIP corrupto y vacío | Pendiente |
| `Ws/Reader/CdrReaderInterface.php` | `getCdrResponse(?xml): ?CdrResponse` | `BaseSunat`; CDR sintético | Pendiente |
| `Ws/Reader/DomCdrReader.php` | XML vacío → `XmlReaderException`; extrae `ReferenceID`, `ResponseCode`, `Description`, notas y `DocumentDescription` por namespace | `BaseSunat`, `GreSender`; CDR con/sin notas/referencia, parser independiente | Pendiente |
| `Ws/Reader/FilenameExtractorInterface.php` | `getFilename(DOMDocument|string): ?string` | `lite/See`; UBL sintético | Pendiente |
| `Ws/Reader/UblNs.php` | Constantes CAC/CBC UBL 2 | Lectores; XPath y namespace verificados | Pendiente |
| `Ws/Reader/XmlFilenameExtractor.php` | RUC por familia/UBL 2.0/2.1, tipo 01/07/08/09/20/40, ID; versión desconocida → excepción | `lite/See`; XML por familia y validador XML | Pendiente |
| `Ws/Reader/XmlReader.php` | DOM y XPath; `getDocument`, `parseToDocument`, `loadXpath`, `loadXpathFromDoc`, `getRoot`, `getXpath`, `getValue` | Lectores/resolver; XML vacío, malformado, prefijos | Pendiente |
| `Ws/Reader/XmlReaderException.php` | `LogicException` para CDR vacío/UBL inválido | Lectores; errores tipados | Pendiente |
| `Ws/Resolver/TypeResolverInterface.php` | `getType(DOMDocument|string): ?class` | `lite`; raíces UBL | Pendiente |
| `Ws/Resolver/XmlTypeResolver.php` | Invoice, Note, Despatch, Perception, Retention, Summary, Voided RA/Reversion; raíz desconocida → `""` | `lite/See`; XML sintético por raíz | Pendiente |
| `Ws/Services/BaseSunat.php` | `set/getClient`, `setCodeProvider`, `compress`, `extractResponse`, fault/código, error 100..1999; selecciona primer XML del ZIP | Bill/Summary/Ext/Consult/GreSender; mocks y ZIP/CDR | Pendiente |
| `Ws/Services/BillSender.php` | `send` → ZIP `<nombre>.xml`, SOAP `sendBill`, CDR ZIP → `BillResult`; sin CDR → `CDR`; `SoapFault` → error | `lite/WsSenderResolver`; servidor SOAP local | Pendiente |
| `Ws/Services/SummarySender.php` | `send` → ZIP, SOAP `sendSummary`, ticket → `SummaryResult`; fault → error | `lite/WsSenderResolver`; servidor SOAP local | Pendiente |
| `Ws/Services/ExtService.php` | `getStatus(ticket)` → 98 pendiente, 0/99 con CDR, 100..1999 error; respuesta sin `status` → excepción | `lite/See`; mock SOAP, CDR | Pendiente |
| `Ws/Services/ConsultCdrService.php` | `getStatus`/`getStatusCdr` con RUC/tipo/serie/número; contenido opcional, mensaje, código y fault | `lite/See`; servidor SOAP local | Pendiente |
| `Ws/Services/CustomErrorCodes.php` | Código `CDR` y dos textos para CDR ausente | Servicios SOAP; casos sin CDR | Pendiente |
| `Ws/Services/SoapClient.php` | WSDL local, `setCredentials`, `setService`, `call`; TLS PHP deshabilitado por defecto | `SoapBuilder`; capturar WSSE/envelope/HTTP con loopback. Rust conservará TLS verificado: diferencia deliberada bloqueante | Pendiente |
| `Ws/Services/WsClientInterface.php` | `call(function, arguments)` puede devolver objeto o lanzar `SoapFault` | Servicios; transporte inyectable | Pendiente |
| `Ws/Services/SunatEndpoints.php` | Constantes beta/homologación/producción/consulta/otros; guía SOAP obsoleta | `lite`, builders; `endpoints` compara literales sin red | Comparado |
| `Ws/Services/WsdlProvider.php` | Rutas locales de WSDL de envío/consulta | `SoapBuilder`; WSDL/XSD local | Inventariado |
| `Ws/Builder/DocumentNoSupportException.php` | Rechaza clase ajena a `BaseSunat` | `ServiceBuilder`; caso inválido | Pendiente |
| `Ws/Builder/ServiceBuilder.php` | `setClient`, `build(type)` dinámico de `BaseSunat` | `lite`; selección tipada Rust | Pendiente |
| `Ws/Builder/SoapBuilder.php` | `setUrl/Wsdl/WsdlParams/User/Password`, `build` configura cliente | `lite`; configuración/WSSE sin red | Pendiente |
| `Ws/Header/WSSESecurityHeader.php` | UsernameToken PasswordText; usuario como texto, contraseña incrustada en `XSD_ANYXML` | `SoapClient`; estructura SOAP y protección de secretos | Pendiente |
| `Api/ApiFactory.php` | `create` consulta token por `client_id`; reutiliza solo expiración `> ahora + 600s`; pide `password`/scope, rechaza token vacío y crea `CpeApi` con host CPE | `lite/Api`; reloj/store inyectables, cliente `gre-api` | Pendiente |
| `Api/InMemoryStore.php` | `get/set` por ID nullable en array PHP | `ApiFactory`; caché por ID y frontera de 600s | Pendiente |
| `Api/GreSender.php` | `send` hace ZIP→Base64/SHA-256→`enviarCpe`; `status` interpreta 98/99/0, CDR y errores HTTP 422/500 | `lite/Api`; API GRE simulada y hash ZIP independiente | Pendiente |

## Seis recursos WSDL/XSD

| Recurso | Importaciones y operaciones | SHA-256 |
| --- | --- | --- |
| `billService.wsdl` | Importa `billService.ns1.wsdl`; binding SOAP 1.1 `getStatus`, `sendBill`, `sendPack`, `sendSummary` con acciones `urn:*` | `c805b5efa14e870205bcc6e424eb940997992534c5b029603e7a0b20ebf33215` |
| `billService.ns1.wsdl` | Importa `billService.xsd2.xsd`; `portType` de cuatro métodos | `b4910f1021b1ad97b1e9c9619b2bb34096160b7ceac47e19f165e91122f1304d` |
| `billService.xsd2.xsd` | Tipos/elementos de envío y estado | `4743806437a72b4e1a8466d4da4cb8eefe7db1b648dbcf6fcfcadd60764bd46e` |
| `billConsultService.wsdl` | Importa `billConsultService.ns1.wsdl`; binding SOAP 1.1 `getStatus`, `getStatusCdr` | `a9ad0ddaff509a9bf430e202e097380eeabd09a580c2f854053d7dcc3b68379d` |
| `billConsultService.ns1.wsdl` | Importa `billConsultService.xsd`; `portType` de consulta | `87032b7468988500bd38a62c463f77850583c6ae6a023c52c15a78be855b21a0` |
| `billConsultService.xsd` | Tipos/elementos de consulta de comprobantes y CDR | `d47f0cfee67da52cb1aaf6236b3513f3137280088e99bb99b2711e7c4ffe22c5` |

## Auditoría de los once `*Test.php` antes de ejecutar

| Archivo | Riesgo visto en el código | Decisión inicial |
| --- | --- | --- |
| `Api/ApiFactoryTest.php` | `AuthApi` simulado; sin red | Permitir tras revisar autoload |
| `Api/GreSenderTest.php` | `CpeApi` simulado; lee recursos upstream locales | Permitir solo con datos sintéticos equivalentes; no copiar recursos al fixture público |
| `Builder/ServiceBuilderTest.php` | `WsClientInterface` simulado; lee XML/CDR local | Permitir tras revisión |
| `Builder/SoapBuilderTest.php` | `testCreateSoapClient` solo WSDL local; `testCreateConsultClient` llama `getStatusCdr` contra endpoint real | Permitir únicamente primer método; excluir segundo |
| `Reader/DomCdrReaderTest.php` | Solo XML local | Permitir tras revisión de fixtures |
| `Reader/XmlFilenameExtractorTest.php` | Solo XML local | Permitir tras revisión de fixtures |
| `Resolver/TypeResolverTest.php` | Solo XML local | Permitir tras revisión de fixtures |
| `Services/ConsultCdrServiceTest.php` | Dos `InvalidUser` construyen `SoapClient` real y llaman SUNAT; otros tres usan mock | Excluir dos primeros; permitir tres con mock |
| `Services/FeSunatTest.php` | Dos `@group manual` llaman SUNAT; `testSendVoidedSummarySender` **sin marca** también usa `SoapClient` real; otros métodos usan mocks | Excluir estos tres; permitir resto por lista exacta |
| `Zip/ZipFactoryTest.php` | Solo ZIP local | Permitir tras revisión |
| `Zip/ZipFileDecompressTest.php` | Solo ZIP local | Permitir tras revisión |

El grupo `manual` por sí solo **no** aísla las pruebas PHP. Ninguna prueba
upstream debe ejecutarse por exclusión de grupo genérica. Toda ejecución será
con lista permitida y red de salida bloqueada, usando credenciales ficticias.

En el corte 0.1.0 se ejecutaron únicamente `ZipFactoryTest.php` y
`ZipFileDecompressTest.php` con `sandbox-exec` denegando red: 5/5 pruebas
cada archivo. Los nueve restantes siguen pendientes de allowlist exacta y
revisión de todos sus fixtures. La cobertura por casos, diferencias y gates
está en [ws-0.1.0.md](ws-0.1.0.md); las filas «Pendiente» de esta matriz
siguen indicando falta de caracterización exhaustiva, aunque haya fixtures
parciales para su grupo.
