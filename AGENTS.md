# Instrucciones para agentes y contribuidores de CPEForge

CPEForge es un SDK modular en Rust para facturación electrónica en Perú: 13
crates en `crates/` (`cpeforge` en `crates/cpeforge` es el paquete principal y los
demás se publican como `cpeforge-*`). La API es estable y sigue SemVer; la
tabla de equivalencias con el proyecto de referencia está en
[docs/README.md](docs/README.md). Aplicar también las instrucciones del
workspace padre.

## Estructura

- `crates/`: los crates publicables. Su código, pruebas, ejemplos y README no
  mencionan el proyecto de referencia ni su lenguaje: usar el nombre CPEForge
  y describir el comportamiento en términos técnicos.
- `tests/reference/`: fixtures (golden files) que usan las pruebas Rust.
  `tests/support/perfil_seguro.rs` valida los registros `differences.json`.
- `example/rust/`: ejemplo completo de emisión con los crates.
- `oracle/`, `example/php/` y `scripts/*_parity.sh`: herramienta de contraste
  que ejecuta la referencia PHP sobre los mismos casos. No forma parte de los
  crates; no editar `greenter-php/` (clon de consulta, ignorado por Git).
  Toda salida nueva del oráculo pasa por `oracle/identidad.py` antes de
  compararse con los fixtures; un identificador nuevo de la referencia se
  traduce allí, no en los fixtures a mano.

## Implementación

- Rust idiomático, pequeño y documentado, con tipos explícitos para dinero
  (`Decimal`), fechas (`jiff`, zona `America/Lima`), códigos y errores. Nada de
  `f32`/`f64` para decisiones monetarias.
- Una entrada externa inválida nunca causa `panic!`, `unwrap()` ni `expect()`.
  Errores tipados con contexto; no ocultar fallos.
- Separar lógica pura de IO; reloj y transporte inyectables para pruebas
  deterministas.
- XML: sin DTD ni entidades externas, con límites de tamaño, profundidad y
  nodos. Firma: OpenSSL a través del crate `openssl`, sin criptografía propia.
  HTTP/SOAP/ZIP: TLS verificado, tiempos límite, límites de tamaño y nombres
  de ZIP validados.
- `unsafe` solo en el FFI ya revisable (`cpe-validator/src/ffi.rs`,
  `ubl-validator/src/ffi.rs`), justificado bloque a bloque.
- Dependencias: elegir versiones vigentes compatibles con el MSRV, revisar
  mantenimiento, licencia y avisos de seguridad; compartir versiones en
  `[workspace.dependencies]`.
- Un cambio que altere la salida observable actualiza sus fixtures y, si se
  aparta de la referencia, se registra en `tests/reference/<crate>/differences.json`
  con motivo y nota.
- No usar datos reales de contribuyentes ni llamar a SUNAT/OSE de producción
  en pruebas.

## Puerta de calidad

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
cargo deny check
cargo audit
```

Reportar los comandos ejecutados y su resultado real; si algo no se ejecutó,
decir «no ejecutado». No declarar un crate estable sin pruebas y revisión.

No hacer commit, push, publicar crates ni abrir PR sin solicitud del usuario.
