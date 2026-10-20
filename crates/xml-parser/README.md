# cpeforge-xml-parser

Lectura de comprobantes electrónicos SUNAT en XML a los modelos de
`cpeforge-core`. Un parser por familia: `InvoiceParser`, `NoteParser`,
`DespatchParser`, `ReceiptParser`, `SummaryParser`, `VoidedParser` (devuelve
`Voided` o `Reversion` según el prefijo `RA`), `RetentionParser` y
`PerceptionParser`. Todos implementan `cpeforge_core::ports::DocumentParser`
con entrada `[u8]` y aceptan además un `XmlDocument` ya analizado.
`XmlReader` ofrece, como contrato, el subconjunto de XPath que usan los
parsers: rutas absolutas (`/a:b/c:d`) y relativas (`a:b/c:d`), descendientes
(`//a:b`, `a//b`), comodín `*` y un `@atributo` final. Funciones (`count()`,
`string()`…), predicados (`[1]`), ejes explícitos y uniones se rechazan con
`XmlParserError::Query`; no está previsto implementar XPath 1.0 completo.

```rust
use cpeforge_core::ports::DocumentParser;
use cpeforge_xml_parser::{ParseOptions, SummaryParser};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let xml = br#"<SummaryDocuments
  xmlns="urn:sunat:names:specification:ubl:peru:schema:xsd:SummaryDocuments-1"
  xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
  <cbc:ID>RC-20240601-1</cbc:ID><cbc:IssueDate>2024-06-01</cbc:IssueDate>
</SummaryDocuments>"#;
let parser = SummaryParser::with_options(ParseOptions::new());
let summary = parser.parse(xml)?.ok_or("sin documento")?;
assert_eq!(summary.correlativo.as_deref(), Some("1"));
# Ok(()) }
```

Conversiones: importes como `Decimal` exacto, enteros con saturación,
comparación numérica tolerante de códigos, fechas en `America/Lima` o la zona
configurada. Los errores de lectura exponen `error_class()` y
`error_message()`. El reloj de las fechas «ahora» es inyectable con
`ParseOptions::with_fixed_now`.

**Perfil seguro:** se rechaza el XML mal formado, cualquier DTD con
declaraciones (sin expansión de entidades ni acceso externo), las
codificaciones distintas de UTF-8/UTF-16/ISO-8859-1/US-ASCII/windows-1252/
ISO-8859-15, las entradas que superan `Limits` (10 MiB, 64 niveles,
1 000 000 de nodos por defecto) y, antes de construir el árbol, los elementos
con más de 256 atributos y los ámbitos de espacios de nombres desmedidos
(`cpeforge_xml_limits::check_tree_markup`). Las pruebas usan solo XML
sintéticos.

`cargo run -p cpeforge-xml-parser --example parse_invoice`,
`cargo run -p cpeforge-xml-parser --example reader` y
`cargo bench -p cpeforge-xml-parser --bench parse` muestran el uso y el
rendimiento sin red.

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
