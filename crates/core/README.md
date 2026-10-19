# cpeforge-core

Modelos y contratos de comprobantes de pago electrónicos (CPE) de Perú para
[CPEForge](https://github.com/alvarez25leo/cpeforge).

## Alcance

- Modelos de factura, boleta, notas, recibo por honorarios, guía de remisión,
  resumen diario, comunicación de baja, reversión, retención, percepción,
  emisor/receptor y respuestas de servicio (CDR, tickets, errores).
- Nombre de archivo de cada documento (`Document::name`) e identificador de
  resúmenes y bajas con la fecha de Perú (`xml_id`).
- Contratos (`ports`, `service`) para builders, envío, parseo, reportes,
  validación, tokens y catálogos de errores.
- Importe en letras para la leyenda 1000 (`amount_words`,
  `Legend::amount_in_words`).

**No incluye** XML, firma, compresión, transporte ni llamadas a SUNAT.

## Decisiones de diseño

- Importes como `rust_decimal::Decimal`: valores exactos, sin redondeo
  binario. Límite: 28 dígitos.
- Fechas como `jiff::Zoned`: rango entre los años -9999 y 9999.
- `null` y «no establecido» son `None`; `""`, `0` y `[]` se conservan.
- Con la feature `serde`, los nombres JSON son los de las propiedades del
  modelo en camelCase; los importes viajan como texto y los números JSON con
  decimales se rechazan. También se rechaza `_` en un importe (`"1_00"`). Los
  mensajes de error recortan la entrada a 64 caracteres.

## Aceptación de un CDR

`CdrResponse::is_accepted` convierte el código con una regla permisiva
(prefijo numérico) y acepta `0` o `>= 4000`; un código **ausente o no
numérico cuenta como aceptado**. Para decidir si un comprobante quedó
aceptado, rechazado o sin conclusión, usar `TaxStatus` de `cpeforge-ws`, que
trata esos casos como indeterminados y separa excepciones (`0100`–`1999`) de
rechazos (`2000`–`3999`).

## Rendimiento

`cargo test --release -p cpeforge-core --all-features --test bench -- --ignored --nocapture`
(Apple Silicon, mediana de 5 ejecuciones):

| Operación | Tiempo |
| --- | --- |
| Serializar una factura de 1000 líneas | 449 µs |
| Deserializar la misma factura | 750 µs |
| Serializar un CDR de 1 MiB (Base64) | 704 µs |
| `Summary::name` (`RC` + fecha de Perú) | 145 ns |

## Ejemplo

```bash
cargo run -p cpeforge-core --example invoice_model
```

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
