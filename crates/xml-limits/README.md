# cpeforge-xml-limits

Límite de anidamiento XML comprobado sobre el texto, antes de construir el
árbol. Crate de apoyo de CPEForge, sin dependencias.

`roxmltree` 0.21.1 analiza los elementos de forma recursiva y no limita la
profundidad. Con un hilo de 2 MiB (la pila de los workers de Tokio) aborta el
proceso a partir de 125 niveles en debug y 3 136 en release; el
desbordamiento de pila no es un pánico y no puede capturarse. `exceeds_depth`
recorre el texto una sola vez, sin recursión, y permite rechazar el documento
antes:

```rust
use cpeforge_xml_limits::exceeds_depth;

let deep = format!("{}{}", "<a>".repeat(5_000), "</a>".repeat(5_000));
assert!(exceeds_depth(&deep, 256));
// El raíz es el nivel 1; un elemento vacío también ocupa un nivel.
assert!(!exceeds_depth("<r><a/></r>", 2));
assert!(exceeds_depth("<r><a/></r>", 1));
```

Omite comentarios, CDATA, instrucciones de procesamiento y declaraciones
`<!…>`, y respeta las comillas de los atributos. Para todo documento que
`roxmltree` acepta, la profundidad contada es la real (`tests/roxmltree.rs`).
Solo es una cota válida si el parser no expande entidades declaradas
(`allow_dtd: false`).

Lo usan `cpeforge-xml-parser` (64 niveles por defecto), `cpeforge-ws` (256
para SOAP y CDR) y `cpeforge-report`.

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE`.
