# cpeforge-xcodes

Catálogo de los 2 077 códigos y mensajes de retorno de SUNAT, tomado de la
hoja «CódigosRetorno» de las reglas de validación vigentes
(`CATALOG_VERSION`, hoy `2026-08-26`). Implementa
`cpeforge_core::ports::ErrorCodeProvider`. El XML incrustado se comprueba por
tamaño, BOM UTF-8 y SHA-256, se analiza una sola vez y se consulta
literalmente. `0100` y `100` son códigos distintos. Los códigos desconocidos y
`None` devuelven `Some("")`.

```rust
use cpeforge_core::ports::ErrorCodeProvider;
use cpeforge_xcodes::XmlErrorCodeProvider;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let catalog = XmlErrorCodeProvider::new()?;
assert_eq!(catalog.iter().len(), cpeforge_xcodes::ENTRY_COUNT);
assert_eq!(catalog.value(Some("desconocido"))?, Some(String::new()));
# Ok(()) }
```

`cargo run -p cpeforge-xcodes --example lookup` y
`cargo run -p cpeforge-xcodes --example with_validator` muestran el uso
independiente y con `cpeforge-validator`. La feature `xcodes` de `cpeforge`
conecta la misma instancia a `See`.

## Actualización del catálogo

SUNAT publica las reglas de validación en
<https://cpe.sunat.gob.pe/guias-y-manuales> («Reglas de validación -
actualizado al …», `.xlsx`). Para actualizar:

```bash
python3 -I scripts/update_xcodes.py "<Reglas de validación>.xlsx" AAAA-MM-DD
```

El script lee la hoja «CódigosRetorno», conserva byte a byte los textos que no
cambiaron, toma de SUNAT los cambiados y los nuevos, falla si desaparece un
código, y escribe `src/data/CodeErrors.xml` y
`tests/reference/xcodes/sunat-catalog.json`. Después hay que copiar a
`src/lib.rs` las constantes que imprime, registrar en
`tests/reference/xcodes/differences.json` los códigos cambiados y correr
`python3 scripts/check_xcodes_xml.py` y las pruebas del crate. Conviene
revisarlo con cada versión nueva de las reglas.

## Estado

Versión 0.3.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
