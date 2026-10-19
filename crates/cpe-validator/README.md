# cpeforge-cpe-validator

Validación de comprobantes SUNAT con hojas XSL: `XmlTypeResolver` detecta el
tipo por la raíz del XML, `XslPathResolver` ubica la hoja en un **directorio
local de reglas** (`2.X/ValidaExprRegFactura-2.0.1.xsl`, …), `XslValidator`
la ejecuta con libxslt (XSLT 1.0 + EXSLT) y `ErrorResultParser` convierte cada
`xsl:message` `nivel|código|mensaje[|ruta|valor]` en un `CpeError`.

El crate **no distribuye las reglas SUNAT**: el resultado depende de la
versión de las hojas que provea el consumidor.

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

Requisitos: libxml2, libxslt y libexslt del sistema (`libxslt1-dev` en
Debian/Ubuntu; el SDK en macOS). Windows no está soportado.

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
