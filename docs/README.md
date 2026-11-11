# Equivalencias con Greenter

CPEForge es un proyecto independiente que tomó como referencia funcional el
SDK PHP [Greenter](https://github.com/thegreenter/greenter) v5.4.0 (MIT). Esta
página sirve solo para orientarse si se viene de ese proyecto: indica qué
crate de CPEForge cubre cada paquete y qué identificadores cambiaron. CPEForge
no está afiliado a Greenter ni a SUNAT.

## Paquetes

| Paquete Greenter (PHP) | Crate CPEForge (Rust) | Directorio |
| --- | --- | --- |
| `greenter/core` | `cpeforge-core` | `crates/core` |
| `greenter/xml` | `cpeforge-xml` | `crates/xml` |
| `thegreenter/xmldsig` | `cpeforge-xmldsig` | `crates/xmldsig` |
| `greenter/gre-api` | `cpeforge-gre-api` | `crates/gre-api` |
| `greenter/ws` | `cpeforge-ws` | `crates/ws` |
| `greenter/lite` | `cpeforge` (paquete principal) | `crates/cpeforge` |
| `greenter/validator` | `cpeforge-validator` | `crates/validator` |
| `greenter/xcodes` | `cpeforge-xcodes` | `crates/xcodes` |
| `greenter/xml-parser` | `cpeforge-xml-parser` | `crates/xml-parser` |
| `greenter/cpe-validator` | `cpeforge-cpe-validator` | `crates/cpe-validator` |
| `greenter/ubl-validator` | `cpeforge-ubl-validator` | `crates/ubl-validator` |
| `greenter/report` | `cpeforge-report` | `crates/report` |
| — | `cpeforge-xml-limits` (apoyo, sin equivalente) | `crates/xml-limits` |
| `greenter/htmltopdf` | sin implementar | — |
| `greenter/data` | sin equivalente (datos de ejemplo) | — |

Los crates `greenter-rs-*` publicados antes del cambio de nombre ya no
existen en crates.io; sus sucesores son los `cpeforge-*` de la tabla.

## Identificadores que cambiaron

| Antes | CPEForge |
| --- | --- |
| Id de la firma `GreenterSign` | `CPEForgeSign` |
| Referencia UBL `#GREENTER-SIGN` | `#CPEFORGE-SIGN` |
| User-Agent `Greenter-Client/1.0.0/PHP` (API GRE) | `CPEForge/0.1.0` |
| Nombres de clase `Greenter\Model\…`, `Greenter\Sunat\GRE\…` en errores y en `DocumentKind::from_class_name` | `CPEForge\Model\…`, `CPEForge\Sunat\GRE\…` |
| Función XSLT `Greenter\Validator\Xml\XsltFunctions::matches` | `CPEForge\Validator\Xml\XsltFunctions::matches` |
| `php_class()` (`cpe-validator`, `gre-api`, `xml-parser`) | `error_class()` |
| `XmlParserError::php_message()` | `error_message()` |
| `ApiError::Php` (`gre-api`) / `XmlParserError::Php` | `ApiError::Decode` / `XmlParserError::Runtime` |
| `PhpTypeError` (`validator`) | `SetterTypeError` |
| `response_code::php_int_cast` (`core`) | `response_code::int_cast` |
| `format::php_float_string` (`xml`) | `format::float_string` |
| `X509ContentType::from_php` (`xmldsig`) | `X509ContentType::from_code` |
| `key::php_base64_decode` (`xmldsig`) | `key::lenient_base64_decode` |
| módulo `report::php` | `report::format` |

Las reglas XSL que llamen a la función `matches` con el nombre anterior deben
actualizarse al nuevo nombre.

## Cambios incorporados de la versión 5.4.0

| Cambio | Crates | Detalle |
| --- | --- | --- |
| Indicador de emisión excepcional en percepción (campo 7 de «Percepciones1_0») | `core`, `xml`, `xml-parser`, `validator` | `Perception::ind_excepcional` (`01`, reglas 3322-3324). Se escribe como `sac:ExceptionalIndicator` tras `cbc:IssueTime` solo si tiene valor; el validador exige 2 caracteres. |
| Número de pago o cobro | `core`, `xml`, `xml-parser`, `validator` | `Payment::numero` va en `cac:Payment/cbc:ID`; sin valor se usa la posición (1). El validador exige `> 0`; el parser lo lee del propio `cac:Payment`, igual que `cbc:PaidDate`. |
| Un pago o cobro por comprobante relacionado | `validator` | `Count` máximo 1 en `RetentionDetail::pagos` y `PerceptionDetail::cobros` (el XSD admite un `cac:Payment`). `cpeforge-xml` ya rechazaba más de uno con `XmlError::MultiplePayments`. |
| `TaxAmount` de EXO, INA y EXP con formato n(12,2) | `xml` | `0.00` en lugar de `0` en factura, notas UBL 2.1 y liquidación de compra. |
| Dirección de entrega con partes opcionales | `xml` | En la factura UBL 2.1 cada parte de `cac:Delivery/…/cac:Address` es opcional: la exportación de servicios puede declarar solo el país. |
| Dirección del agente de retención y percepción | `xml` | `cbc:CityName` es la provincia y `cbc:CountrySubentity` el departamento. |

**Compatibilidad:** quien haya invertido a propósito `departamento` y
`provincia` en la dirección del emisor de retenciones o percepciones para
compensar el orden anterior debe volver a asignarlos a su campo real. Los
parsers de retención y percepción siguen leyendo `cbc:CityName` como
departamento, igual que la referencia.

## Herramienta de contraste

`oracle/`, `example/php/` y los `scripts/*_parity.sh` ejecutan la referencia
PHP sobre los mismos casos que Rust. Se conservan como herramienta de
contraste; no forman parte de los crates publicados. La salida de la
referencia se traduce a la identidad CPEForge antes de compararla
(`oracle/identidad.py`), y en los oráculos que firman de extremo a extremo la
referencia firma ya con `CPEForgeSign`/`#CPEFORGE-SIGN`
(`oracle/php/identidad.php`), así que firmas, digest y ZIP se siguen
comparando byte a byte.
