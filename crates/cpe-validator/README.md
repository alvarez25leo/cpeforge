# cpeforge-cpe-validator

Validación de comprobantes SUNAT con hojas XSL: `XmlTypeResolver` detecta el
tipo por la raíz del XML, `XslPathResolver` ubica la hoja en un **directorio
local de reglas** (`2.X/ValidaExprRegFactura-2.0.1.xsl`, …), `XslValidator`
la ejecuta con libxslt (XSLT 1.0 + EXSLT) y `ErrorResultParser` convierte cada
`xsl:message` `nivel|código|mensaje[|ruta|valor]` en un `CpeError`.

El crate **no distribuye las reglas SUNAT**: el resultado depende de la
versión de las hojas que provea el consumidor. Las hojas tal como las publica
SUNAT («Archivo XSL – 2.1») no se ejecutan directamente: incluyen
`local:///commons/…` de IBM DataPower (no publicados) y usan EXSLT
`regexp:match`; se rechazan al compilar sin leer nada fuera del directorio de
reglas. Deben convertirse como en la referencia (`regexp:match` →
`MATCHES_HANDLER`, `commons` incluidos en el directorio de reglas). Los
nombres que busca `XslPathResolver` son los de la referencia
(`ValidaExprRegPercepcion-1.0.1.xsl`, `ValidaExprRegGreRemitente-2.0.1.xsl`);
el ZIP de SUNAT trae `Percepcion-1.0.2` y `GuiaRemitente-2.0.1`, que se
renombran al convertirlos o se ubican con un `RuleResolver` propio
(`CpeValidator::with_parts`).

```rust,no_run
use cpeforge_cpe_validator::CpeValidator;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let validator = CpeValidator::new("/ruta/a/reglas");
let xml = std::fs::read("20000000001-01-F001-1.xml")?;
for error in validator.validate_xml("20000000001-01-F001-1.xml", &xml)? {
    println!("{:?} {:?} {:?}", error.level, error.code, error.message);
}
# Ok(()) }
```

Las hojas pueden llamar a la función de extensión de expresiones regulares
registrada como `MATCHES_HANDLER`
(`CPEForge\Validator\Xml\XsltFunctions::matches`) con semántica PCRE en modo
de bytes y límites de retroceso y recursión. Las reglas que usen otro nombre
de función deben adaptarse a ese.

**Perfil seguro:** se rechaza todo `DOCTYPE`; no hay red ni escritura;
`xsl:include`, `xsl:import` y `document()` solo leen dentro del directorio de
reglas autorizado (enlaces simbólicos resueltos); hay límites de tamaño, de
operaciones XPath (si la libxml2 lo aplica, ver `engine_info()`) y de PCRE.
Los fallos de infraestructura son `CpeValidatorError` (con `error_class()`),
nunca una lista vacía.

Requisitos: libxml2 ≥ 2.9.11, libxslt ≥ 1.1.34 y libexslt del sistema
(`libxslt1-dev` en Debian/Ubuntu; el SDK en macOS), comprobadas al compilar y
al iniciar. Windows no está soportado. Antes de libxml2 el XML pasa por
`cpeforge_xml_limits::check_markup` (sin `DOCTYPE`, como máximo 256 atributos
por elemento).

**Limitación conocida, sin soporte:** la libxml2 de Apple en macOS < 13 no
ofrece el límite de operaciones XPath (`engine_info().xpath_operation_limit`
es `false`); con ella una hoja de reglas costosa no se corta por
`Limits::xpath_operations`. Con reglas que no sean de confianza usar macOS 13
o posterior, o Linux con libxml2 ≥ 2.9.11.

## Estado

Versión estable. La API pública sigue SemVer: los errores, resultados y
opciones pueden crecer sin romper el código que la usa (ver el
[README del repositorio](https://github.com/alvarez25leo/cpeforge#estado)).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
