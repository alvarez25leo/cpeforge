# CPEForge

**Facturación electrónica SUNAT en Rust: rápida, segura y sin sorpresas.**

[![crates.io](https://img.shields.io/crates/v/cpeforge.svg)](https://crates.io/crates/cpeforge)
[![docs.rs](https://img.shields.io/docsrs/cpeforge)](https://docs.rs/cpeforge)
[![CI](https://github.com/alvarez25leo/cpeforge/actions/workflows/ci.yml/badge.svg)](https://github.com/alvarez25leo/cpeforge/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](#requisitos)
[![Licencia: MIT](https://img.shields.io/badge/licencia-MIT-blue.svg)](LICENSE)

CPEForge es un SDK de código abierto para emitir comprobantes de pago
electrónicos (CPE) de Perú: construye el XML UBL, lo firma, lo envía a
SUNAT, lee el CDR y genera la representación impresa. Está escrito en Rust,
dividido en paquetes independientes y pensado para que tu sistema de
facturación sea tan confiable como el dinero que mueve.

## ¿Por qué facturar con Rust?

Facturar no es un CRUD más. Cada comprobante tiene importes que deben cuadrar
al céntimo, fechas que dependen de la zona horaria de Lima, una firma que no
admite ni un byte de diferencia y un servidor de SUNAT que responde cuando
puede. Rust convierte buena parte de esos riesgos en errores que ves antes de
que lleguen a tu cliente:

- **El dinero nunca es `float`.** Todos los importes son `Decimal` exacto y
  las fechas son `Zoned`, con zona horaria. Sin redondeos silenciosos ni
  comprobantes emitidos con la fecha de otro día.
- **Errores que se ven en el tipo.** Cada operación devuelve un `Result` con
  un error que dice qué falló: el XML, la firma, el transporte o la respuesta
  de SUNAT. El compilador te obliga a tratarlo, y el estado tributario se
  clasifica con `TaxStatus` (aceptado, con observaciones, rechazado,
  pendiente) en vez de adivinarlo por un código suelto.
- **Rendimiento predecible.** Código nativo, sin intérprete ni recolector de
  basura. Construir, firmar y enviar una factura de 1000 líneas (2,6 MB
  firmada) toma unos 36 ms con un transporte en memoria
  (`crates/cpeforge/tests/bench.rs`). Un servidor pequeño atiende el volumen
  de muchas empresas.
- **Seguro por defecto.** El XML se lee sin DTD ni entidades externas y con
  límites de tamaño, profundidad, atributos y espacios de nombres. TLS
  siempre verificado, ZIP validado antes de abrirlo y secretos ocultos en
  `Debug` y borrados de memoria al soltarse. Sin `unsafe`, salvo el puente
  con libxml2 y libxslt de los validadores, revisado bloque a bloque.
- **Concurrencia sin miedo.** Los tipos principales son `Send + Sync` y la red
  es `async` sobre Tokio. La caché de tokens de la API de guías separa
  empresas y ambientes: un servicio multiempresa no mezcla credenciales.
- **Un binario y listo.** No hay runtime que instalar en el servidor: compilas
  y despliegas un ejecutable o un contenedor pequeño. Solo necesitas OpenSSL
  del sistema (y libxml2/libxslt si usas los validadores XSD y XSL).
- **Solo lo que necesitas.** 13 paquetes con responsabilidades claras: usa la
  fachada completa o solo el firmador, el lector de XML o el generador de la
  representación impresa.
- **Verificado de verdad.** Más de 370 pruebas, y la CI compila y prueba en
  Linux con Rust 1.85, 1.88 y estable. Las firmas se verifican fuera del
  crate con `xmllint` y `openssl`, la transformación XPath se contrasta byte a
  byte con libxml2 y los XML generados validan contra los esquemas UBL.

## Empieza en cinco minutos

```toml
[dependencies]
cpeforge = "0.2"
cpeforge-core = "0.1"
cpeforge-gre-api = "0.2"
cpeforge-ws = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Una factura gravada con IGV, firmada y enviada al entorno beta de SUNAT:

```rust,no_run
use cpeforge::{DocumentRef, See, SendResult};
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, SaleDetail};
use cpeforge_core::{Decimal, Zoned};
use cpeforge_gre_api::ReqwestTransport;
use cpeforge_ws::outcome::TaxStatus;
use cpeforge_ws::soap::SunatEndpoints;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let total = Decimal::new(118, 0);
    let invoice = Invoice {
        base: BaseSale {
            ubl_version: Some("2.1".into()),
            tipo_doc: Some("01".into()), // factura
            serie: Some("F001".into()),
            correlativo: Some("1".into()),
            fecha_emision: Some(Zoned::now()),
            tipo_moneda: Some("PEN".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                razon_social: Some("MI EMPRESA S.A.C.".into()),
                address: Some(Address {
                    ubigueo: Some("150101".into()),
                    direccion: Some("Av. Siempre Viva 123".into()),
                    cod_local: Some("0000".into()),
                    ..Address::default()
                }),
                ..Company::default()
            }),
            client: Some(Client {
                tipo_doc: Some("6".into()), // RUC
                num_doc: Some("20123456789".into()),
                rzn_social: Some("CLIENTE S.A.C.".into()),
                ..Client::default()
            }),
            mto_oper_gravadas: Some(Decimal::new(100, 0)),
            mto_igv: Some(Decimal::new(18, 0)),
            total_impuestos: Some(Decimal::new(18, 0)),
            mto_imp_venta: Some(total),
            details: Some(vec![SaleDetail {
                cod_producto: Some("P001".into()),
                unidad: Some("NIU".into()),
                cantidad: Some(Decimal::new(2, 0)),
                descripcion: Some("PRODUCTO".into()),
                mto_valor_unitario: Some(Decimal::new(50, 0)),
                mto_precio_unitario: Some(Decimal::new(59, 0)),
                mto_valor_venta: Some(Decimal::new(100, 0)),
                mto_base_igv: Some(Decimal::new(100, 0)),
                porcentaje_igv: Some(Decimal::new(18, 0)),
                igv: Some(Decimal::new(18, 0)),
                total_impuestos: Some(Decimal::new(18, 0)),
                tip_afe_igv: Some("10".into()), // gravado
                ..SaleDetail::default()
            }]),
            // «SON CIENTO DIECIOCHO CON 00/100 SOLES», sin escribirla a mano.
            legends: Some(vec![Legend::amount_in_words(total, "PEN")?]),
            ..BaseSale::default()
        },
        tipo_operacion: Some("0101".into()),
        valor_venta: Some(Decimal::new(100, 0)),
        sub_total: Some(total),
        ..Invoice::default()
    };

    let mut see = See::new(ReqwestTransport::new()?);
    see.set_certificate(&std::fs::read_to_string("certificado.pem")?)?;
    see.set_service(SunatEndpoints::FE_BETA);
    see.set_clave_sol("20000000001", "MODDATOS", "moddatos");

    match see.send(DocumentRef::Invoice(&invoice)).await? {
        SendResult::Bill(bill) => println!("SUNAT: {:?}", TaxStatus::of_bill(&bill)),
        other => println!("respuesta: {other:?}"),
    }
    Ok(())
}
```

El proyecto [`example/rust`](example/rust) emite 43 casos: facturas
(gravadas, exoneradas, de exportación, con detracción, percepción o
anticipo…), boletas, notas de crédito y débito, resúmenes diarios, bajas,
retenciones, percepciones y guías de remisión por la API de SUNAT.

## Qué cubre

- **Comprobantes:** factura, boleta, notas de crédito y débito, resumen
  diario, comunicación de baja, reversión, retención, percepción y guía de
  remisión (UBL 2.0 y 2.1).
- **Envío:** servicios SOAP de SUNAT (`sendBill`, `sendSummary`, consulta de
  tickets y de CDR) y la API REST de guías de remisión con OAuth2.
- **Firma:** XMLDSig con el perfil de SUNAT, C14N inclusiva y exclusiva,
  certificados PEM y PKCS#12, verificación y cifrado XML.
- **Validación:** reglas de los modelos, esquemas XSD de UBL y reglas XSL de
  SUNAT, más el catálogo de los 2 077 códigos de error de SUNAT.
- **Lectura:** de un XML de comprobante a los modelos tipados.
- **Representación impresa:** HTML con plantillas, código QR e importe en
  letras.

## Paquetes

| Crate | Contenido |
| --- | --- |
| [`cpeforge`](crates/cpeforge) | Paquete principal: fachadas `See` (SOAP) y `Api` (guías de remisión) |
| [`cpeforge-core`](crates/core) | Modelos de comprobantes, importe en letras y contratos |
| [`cpeforge-xml`](crates/xml) | XML UBL 2.0/2.1 sin firma |
| [`cpeforge-xmldsig`](crates/xmldsig) | Firma XMLDSig, C14N, X.509 y cifrado XML |
| [`cpeforge-gre-api`](crates/gre-api) | Cliente de la API de guías de remisión |
| [`cpeforge-ws`](crates/ws) | SOAP, ZIP, CDR y estados de SUNAT |
| [`cpeforge-validator`](crates/validator) | Reglas de validación de los modelos |
| [`cpeforge-xcodes`](crates/xcodes) | Catálogo de códigos de error de SUNAT |
| [`cpeforge-xml-parser`](crates/xml-parser) | Lectura de XML de comprobantes a modelos |
| [`cpeforge-xml-limits`](crates/xml-limits) | Límites de XML antes de analizarlo (apoyo) |
| [`cpeforge-cpe-validator`](crates/cpe-validator) | Validación con las reglas XSL de SUNAT |
| [`cpeforge-ubl-validator`](crates/ubl-validator) | Validación XSD de UBL |
| [`cpeforge-report`](crates/report) | Representación impresa HTML y QR |

Cada paquete tiene su README con alcance, seguridad y ejemplos.

## Estado

**Versión estable.** La API pública sigue SemVer:

- los errores, resultados y opciones son `#[non_exhaustive]`: pueden crecer
  sin romper tu código (un `match` lleva una rama `_` y las opciones se crean
  desde `Default`);
- los modelos de comprobantes se construyen con `..Default::default()`;
- ningún tipo de una dependencia interna (`openssl`, `roxmltree`, `reqwest`,
  `zip`…) forma parte de la API; solo `jiff` y `rust_decimal`, que son las
  fechas y los importes de los modelos.

## Requisitos

- Rust 1.85 o posterior (1.88 para `cpeforge` y `cpeforge-ws`).
- OpenSSL 3 del sistema (`libssl-dev` en Debian/Ubuntu).
- libxml2 ≥ 2.9.11 y libxslt ≥ 1.1.34, solo para `cpeforge-ubl-validator` y
  `cpeforge-cpe-validator` (`libxml2-dev` y `libxslt1-dev`).

## Desarrollo

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Ninguna prueba llama a SUNAT ni a un OSE de producción. Los reportes de
errores y las propuestas son bienvenidos en
[GitHub](https://github.com/alvarez25leo/cpeforge/issues).

## Licencia

MIT. Ver [LICENSE](LICENSE); las atribuciones de los componentes de terceros
están en [NOTICE](NOTICE). CPEForge no está afiliado a SUNAT.
