# Equivalencias con Greenter

CPEForge es un proyecto independiente que tomó como referencia funcional el
SDK PHP [Greenter](https://github.com/thegreenter/greenter) v5.3.0 (MIT). Esta
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

## Herramienta de contraste

`oracle/`, `example/php/` y los `scripts/*_parity.sh` ejecutan la referencia
PHP sobre los mismos casos que Rust. Se conservan como herramienta de
contraste; no forman parte de los crates publicados. La salida de la
referencia se traduce a la identidad CPEForge antes de compararla
(`oracle/identidad.py`), y en los oráculos que firman de extremo a extremo la
referencia firma ya con `CPEForgeSign`/`#CPEFORGE-SIGN`
(`oracle/php/identidad.php`), así que firmas, digest y ZIP se siguen
comparando byte a byte.
