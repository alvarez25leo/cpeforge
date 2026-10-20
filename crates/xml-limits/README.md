# cpeforge-xml-limits

Límites XML comprobados sobre el texto, antes de construir el árbol. Crate de
apoyo de CPEForge, sin dependencias.

## Anidamiento

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

Lo usan `cpeforge-xml-parser`, `cpeforge-ws` (SOAP y CDR) y
`cpeforge-report` (`XmlUtils`), con 64 niveles por defecto.

## Marcado antes de libxml2

libxml2 anterior a 2.12 compara cada atributo con los anteriores del mismo
elemento (tiempo cuadrático) y analiza un `DOCTYPE` entero antes de que se
pueda rechazar. `check_markup` recorre el documento una vez y rechaza
cualquier declaración `<!…>` que no sea comentario o CDATA, los elementos con
demasiados atributos y las codificaciones no compatibles con ASCII (UTF-16 se
decodifica antes de recorrerlo):

```rust
use cpeforge_xml_limits::{DEFAULT_MAX_ATTRIBUTES, MarkupRejection, check_markup};

assert_eq!(check_markup(b"<a x='1'/>", DEFAULT_MAX_ATTRIBUTES), Ok(()));
assert_eq!(
    check_markup(b"<!DOCTYPE a><a/>", DEFAULT_MAX_ATTRIBUTES),
    Err(MarkupRejection::Declaration)
);
```

## Marcado antes de `roxmltree`

`roxmltree` 0.21.1 compara cada atributo con los anteriores del mismo
elemento para detectar duplicados (40 000 atributos tardan casi un segundo;
los 8 MiB de un CDR, minutos) y cada elemento que declara un espacio de
nombres copia las referencias de todos los heredados (250 `xmlns` en la raíz
y miles de hijos que redeclaran un prefijo multiplican la memoria por
cientos). `check_tree_markup` recibe el texto ya decodificado y rechaza los
elementos con más de `max_attributes` atributos (contando los `xmlns`) y los
documentos cuyo coste de ámbitos (por cada elemento que declara, los
espacios en ámbito tras declarar) supera el máximo:

```rust
use cpeforge_xml_limits::{
    DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE, MarkupRejection, check_tree_markup,
};

let declarations: String = (0..250).map(|i| format!(" xmlns:p{i}='urn:{i}'")).collect();
let attack = format!("<r{declarations}>{}</r>", "<x xmlns:z='u'/>".repeat(20_000));
assert_eq!(
    check_tree_markup(&attack, DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE),
    Err(MarkupRejection::TooManyNamespaces)
);
```

No comprueba codificación ni `DOCTYPE` (el parser los rechaza) y se detiene
donde el parser se detendría. Lo usan `cpeforge-ws` (CDR y SOAP),
`cpeforge-xml-parser` y `cpeforge-report` (`XmlUtils`).

`with_libxml_init_lock` serializa la inicialización de libxml2 entre los
crates que la enlazan (antes de 2.11 `xmlInitParser` no es seguro en
paralelo). Los usan `cpeforge-cpe-validator` y `cpeforge-ubl-validator`.

## Estado

Versión estable. La API pública sigue SemVer: los errores, resultados y
opciones pueden crecer sin romper el código que la usa (ver el
[README del repositorio](https://github.com/alvarez25leo/cpeforge#estado)).

## Licencia

MIT. Ver `LICENSE`.
