# cpeforge-xcodes

Catálogo fijo de los 1 710 códigos y mensajes de error de SUNAT. Implementa
`cpeforge_core::ports::ErrorCodeProvider`. El XML incrustado se comprueba por
tamaño, BOM UTF-8 y SHA-256, se analiza una sola vez y se consulta
literalmente. `0100` y `100` son códigos distintos. Los códigos desconocidos y
`None` devuelven `Some("")`.

```rust
use cpeforge_core::ports::ErrorCodeProvider;
use cpeforge_xcodes::XmlErrorCodeProvider;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let catalog = XmlErrorCodeProvider::new()?;
assert_eq!(catalog.iter().len(), 1_710);
assert_eq!(catalog.value(Some("desconocido"))?, Some(String::new()));
# Ok(()) }
```

`cargo run -p cpeforge-xcodes --example lookup` y
`cargo run -p cpeforge-xcodes --example with_validator` muestran el uso
independiente y con `cpeforge-validator`. La feature `xcodes` de `cpeforge`
conecta la misma instancia a `See`. Los textos se conservan literalmente, sin
afirmar su vigencia normativa actual.

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
