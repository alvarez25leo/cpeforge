# CPEForge

**Rust SDK for Peruvian Electronic Invoicing**

CPEForge es una biblioteca modular de código abierto, desarrollada en Rust para
la generación, procesamiento, firma digital y validación de comprobantes de
pago electrónicos (CPE) en Perú.

Proporciona componentes independientes para trabajar con documentos XML UBL,
firmas XMLDSig, servicios electrónicos de SUNAT, validaciones tributarias y
representaciones impresas de documentos.

El proyecto está diseñado para facilitar la integración de facturación
electrónica en aplicaciones empresariales mediante una arquitectura modular,
segura y extensible.

**Licencia:** MIT

## Uso

Biblioteca principal:

```toml
[dependencies]
cpeforge = "0.2"
```

Componentes independientes:

```toml
[dependencies]
cpeforge-core = "0.1"
cpeforge-xml = "0.2"
cpeforge-xmldsig = "0.2"
cpeforge-ubl-validator = "0.2"
cpeforge-report = "0.2"
```

Los 13 paquetes `cpeforge` y `cpeforge-*` están publicados en
[crates.io](https://crates.io/crates/cpeforge): la serie 0.2 (0.3 para
`cpeforge-xcodes`; `cpeforge-core` 0.1 y `cpeforge-xml-limits` 0.1 siguen
siendo compatibles). Siguen en desarrollo; lo que falta para una versión estable está en
[docs/pendientes.md](docs/pendientes.md).

## Paquetes

| Crate | Directorio | Contenido |
| --- | --- | --- |
| `cpeforge` | `crates/cpeforge` | Paquete principal: fachadas `See` (SOAP) y `Api` (GRE) |
| `cpeforge-core` | `crates/core` | Modelos de comprobantes y contratos |
| `cpeforge-xml` | `crates/xml` | XML UBL 2.0/2.1 sin firma |
| `cpeforge-xmldsig` | `crates/xmldsig` | Firma XMLDSig, C14N, X.509, XMLEnc |
| `cpeforge-gre-api` | `crates/gre-api` | Cliente de la API GRE de SUNAT |
| `cpeforge-ws` | `crates/ws` | SOAP, ZIP, CDR y adaptador GRE |
| `cpeforge-validator` | `crates/validator` | Reglas de validación de modelos |
| `cpeforge-xcodes` | `crates/xcodes` | Catálogo de códigos de error de SUNAT |
| `cpeforge-xml-parser` | `crates/xml-parser` | Lectura de XML de comprobantes a modelos |
| `cpeforge-xml-limits` | `crates/xml-limits` | Límites de anidamiento y marcado XML (apoyo) |
| `cpeforge-cpe-validator` | `crates/cpe-validator` | Validación con reglas XSL de SUNAT |
| `cpeforge-ubl-validator` | `crates/ubl-validator` | Validación XSD de UBL |
| `cpeforge-report` | `crates/report` | Representación impresa HTML y QR |

Cada crate tiene su README con alcance, seguridad y ejemplos. Las reglas de
compatibilidad de la API (qué puede crecer sin romper, dependencias públicas,
hilos y errores) están en [docs/estabilidad-api.md](docs/estabilidad-api.md);
los cambios de cada versión y la guía de migración, en
[CHANGELOG.md](CHANGELOG.md);
lo que falta para la primera versión estable, en
[docs/pendientes.md](docs/pendientes.md).

## Desarrollo

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

MSRV: Rust 1.85 (`cpeforge` y `cpeforge-ws`: 1.88). `cpeforge-cpe-validator`
y `cpeforge-ubl-validator` necesitan libxml2 ≥ 2.9.11 (y libxslt ≥ 1.1.34)
del sistema; `cpeforge-xmldsig` necesita OpenSSL 3. El ejemplo completo de
emisión está en `example/rust/`. Ninguna prueba llama a SUNAT/OSE de
producción.

## Origen y reconocimientos

CPEForge es un proyecto independiente desarrollado en Rust que toma como
referencia funcional determinadas capacidades de Greenter PHP. La
compatibilidad, las diferencias de implementación y las atribuciones de los
componentes derivados se documentan en este repositorio
([equivalencias](docs/README.md), [NOTICE](NOTICE)).

CPEForge no está afiliado oficialmente a Greenter ni a SUNAT.
