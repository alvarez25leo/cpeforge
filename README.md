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
cpeforge = "0.1.0"
```

Componentes independientes:

```toml
[dependencies]
cpeforge-core = "0.1.0"
cpeforge-xml = "0.1.0"
cpeforge-xmldsig = "0.1.0"
cpeforge-ubl-validator = "0.1.0"
cpeforge-report = "0.1.0"
```

Los paquetes `cpeforge-*` aún no están publicados en crates.io; mientras
tanto se pueden usar como dependencias `path` o `git`.

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
| `cpeforge-xml-limits` | `crates/xml-limits` | Límite de anidamiento XML (apoyo) |
| `cpeforge-cpe-validator` | `crates/cpe-validator` | Validación con reglas XSL de SUNAT |
| `cpeforge-ubl-validator` | `crates/ubl-validator` | Validación XSD de UBL |
| `cpeforge-report` | `crates/report` | Representación impresa HTML y QR |

Cada crate tiene su README con alcance, seguridad y ejemplos. Lo que falta
para la primera versión estable está en [docs/pendientes.md](docs/pendientes.md).

## Desarrollo

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

MSRV: Rust 1.85 (`cpeforge` y `cpeforge-ws`: 1.88). `cpeforge-cpe-validator`
y `cpeforge-ubl-validator` necesitan libxml2/libxslt del sistema;
`cpeforge-xmldsig` necesita OpenSSL 3. El ejemplo completo de emisión está en
`example/rust/`. Ninguna prueba llama a SUNAT/OSE de producción.

## Origen y reconocimientos

CPEForge es un proyecto independiente desarrollado en Rust que toma como
referencia funcional determinadas capacidades de Greenter PHP. La
compatibilidad, las diferencias de implementación y las atribuciones de los
componentes derivados se documentan en este repositorio
([equivalencias](docs/README.md), [NOTICE](NOTICE)).

CPEForge no está afiliado oficialmente a Greenter ni a SUNAT.
