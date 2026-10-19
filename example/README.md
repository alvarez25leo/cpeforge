# Ejemplo de emisión con CPEForge

`rust/` construye, firma y (opcionalmente) envía 43 comprobantes de ejemplo
con los crates de CPEForge, contra los entornos de prueba de SUNAT.

```bash
cargo run --release --manifest-path example/rust/Cargo.toml                    # todos: XML firmado en out/
cargo run --release --manifest-path example/rust/Cargo.toml -- factura boleta  # solo esos
cargo run --release --manifest-path example/rust/Cargo.toml -- --enviar factura
cargo run --release --manifest-path example/rust/Cargo.toml -- --errores       # casos de rechazo
```

Cada caso está en `rust/src/casos/<caso>.rs`; los datos compartidos (empresa,
cliente y fecha base) en `rust/src/datos.rs` y los casos de rechazo en
`rust/src/errores.rs`. El ejemplo verifica la firma de cada XML con el
certificado que lleva incluido.

## Casos

| Tipo | Casos |
| --- | --- |
| Facturas (24) | `factura`, `factura-anticipo`, `factura-beatose`, `factura-compleja`, `factura-contingencia`, `factura-credito`, `factura-descuento-global`, `factura-descuento-linea`, `factura-descuento-nobase`, `factura-detraccion`, `factura-exonerado`, `factura-exportacion`, `factura-gratuita`, `factura-icbper`, `factura-icbper-gratuita`, `factura-inafecta`, `factura-isc`, `factura-ivap`, `factura-percepcion`, `factura-rel-guia-remision`, `factura-retencion`, `inicio-factura-gravada`, `inicio-factura-exonerada`, `inicio-factura-credito` |
| Boletas | `boleta`, `boleta-icbper`, `inicio-boleta` |
| Notas | `nota-credito`, `nota-credito-boleta`, `nota-credito-forma-pago`, `nota-debito` |
| Resúmenes diarios (RC) | `resumen`, `resumen-contingencia`, `resumen-usd` |
| Baja (RA) y reversión (RR) | `comunicacion-baja`, `reversion` |
| Percepción y retención | `percepcion`, `retencion` |
| Guías de remisión 2022 (GRE) | `guia-remision`, `guia-extra-atributos`, `guia-misma-empresa`, `guia-transporteM1L`, `guia-transportePrivado` |

Casos de rechazo (`--errores`), con el código que devuelve el entorno de
prueba:

| Caso | Cambio deliberado | Código |
| --- | --- | --- |
| `guia-sin-fecha-entrega` | GRE de transporte público sin fecha de entrega de bienes | `3617` |
| `nota-motivo13-con-total` | Nota de crédito con motivo 13 e importe total distinto de cero | `3315` |
| `resumen-sin-tasa` | Resumen RC sin `porcentajeIgv` en sus líneas | `2992` |

## Credenciales

Ningún secreto está en el código. El ejemplo lee de la raíz del repositorio:

- `cert/certificate.pem`: clave privada y certificado en PEM;
- `cert/user.txt`: líneas `RUC: …`, `Usuario: …` y `Contraseña: …` (Clave SOL
  de SUNAT BETA), y `GRE_API_URL=`, `GRE_CLIENT_ID=`, `GRE_CLIENT_SECRET=`,
  `GRE_RUC=`, `GRE_USUARIO=` y `GRE_CLAVE=` (API GRE de prueba). Sin las
  `GRE_*`, las guías solo generan XML.

`EJEMPLO_CERT` y `EJEMPLO_CREDENCIALES` cambian esas rutas. `cert/` está en
`.gitignore`.

Los servicios están fijos en entornos de prueba: SUNAT BETA (`FE_BETA` y
`RETENCION_BETA`) y la API GRE de `GRE_API_URL`; se rechaza una URL de
producción de SUNAT. `factura-beatose` nunca se envía.

Cada tipo va por su vía:
- comprobantes: `sendBill` y CDR;
- percepción y retención: `sendBill` al servicio de retenciones;
- resumen y baja: `sendSummary`, ticket y `getStatus`;
- reversión: igual, en el servicio de retenciones;
- guías: API GRE, ticket y estado.

SUNAT BETA responde `401` a los envíos muy seguidos: entre uno y otro se
esperan `EJEMPLO_PAUSA` segundos (5 por defecto). `EJEMPLO_FECHA`
(`AAAA-MM-DD HH:MM:SS`, hora de Lima) fija la fecha base; sin ella se usa la
hora actual.

## Ajustes para los entornos de prueba actuales

| Caso | Cambio | Motivo |
| --- | --- | --- |
| `resumen`, `resumen-contingencia`, `resumen-usd` | `SummaryDetail::porcentaje_igv = 18` | SUNAT rechaza el RC sin la tasa del IGV por línea (error 2992). |
| `guia-remision`, `guia-extra-atributos` | `Shipment::fec_entrega_bienes` | La API GRE la exige en transporte público (error 3617). |
| `guia-misma-empresa` | Destinatario y anexos de partida y llegada con el RUC del emisor GRE | Si el motivo es traslado en la misma empresa, deben coincidir con el emisor (errores 2554 y 3414). |
| `nota-credito-forma-pago` | Importes en cero y leyenda «SON CERO» | Una nota con motivo 13 debe tener total cero (error 3315). |

## Rendimiento

Cada ejecución mide cada caso por fases (construir, xml, firma,
`get_xml_signed`, primera llamada y, con `--enviar`, envío y estado) y deja
`rust/out/benchmark.json`. Compilar en release para medir. Suma de medianas de
los 43 casos en Apple Silicon: construir 24 µs, xml 148 µs, firma 18,2 ms,
`get_xml_signed` 18,7 ms.

## Contraste

`php/` contiene los mismos casos en la implementación de referencia y se
conserva tal cual. Los scripts que generan ambos y comparan el XML están en
`oracle/ejemplos/` (fuera de los ejemplos).
