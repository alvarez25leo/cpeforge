# cpeforge-ubl-validator

Valida documentos UBL 2.0 y 2.1 con libxml2 (XSD 1.0) contra un directorio
local con los 45 esquemas fijados en `schema-manifest.json`. El crate
comprueba el SHA-256 de cada esquema alcanzado y valida sobre una copia
privada de esos bytes, para evitar cambios entre la comprobación y la carga.
La distribución de los XSD está pendiente de revisión de sus avisos OASIS,
W3C, ETSI y UN/CEFACT; por eso el crate aún no los empaqueta.

```rust,no_run
use cpeforge_ubl_validator::UblValidator;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut validator = UblValidator::new("/ruta/a/xsd")?;
let result = validator.validate(br#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"/>"#)?;
println!("{}", result.error);
# Ok(())
# }
```

Acepta las codificaciones XML que reconozca libxml2, sin DTD ni entidades
declaradas; tamaño máximo 10 MiB, 100.000 nodos y 128 niveles. Rechaza
esquemas modificados y referencias fuera del conjunto fijado. En macOS usa
libxml2 del SDK; en Linux requiere `libxml2-dev` y `pkg-config`. Sin soporte
Windows todavía. MSRV: Rust 1.85.

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
