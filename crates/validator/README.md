# cpeforge-validator

Reglas de validación para los modelos de `cpeforge-core`: 33 loaders base y
siete variantes UBL 2.1, metadata ordenada, validación de objetos anidados,
selección de versión, traducción con el contrato opcional `ErrorCodeProvider`
de `cpeforge-core` y extensión de reglas.

```rust
use cpeforge_core::model::client::Client;
use cpeforge_validator::Validator;

let errors = Validator::new().validate_model(&Client::default())?;
assert_eq!(errors[0].path, "tipoDoc");
# Ok::<(), cpeforge_validator::ValidatorError>(())
```

## Estado

Versión 0.2.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
