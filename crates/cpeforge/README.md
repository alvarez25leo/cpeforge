# CPEForge

**SDK en Rust para facturación electrónica en Perú.**

`cpeforge` es el paquete principal: fachadas tipadas `See` (servicios SOAP de
SUNAT) y `Api` (API GRE de guías de remisión) que componen `cpeforge-core`,
`cpeforge-xml`, `cpeforge-xmldsig` y `cpeforge-ws` para construir, firmar y
enviar comprobantes, y leer el CDR.

```toml
[dependencies]
cpeforge = "0.1.0"
```

La feature `xcodes` conecta el catálogo de códigos de error de
`cpeforge-xcodes` (`cargo run -p cpeforge --example see_with_xcodes --features xcodes`).

## Importe en letras (leyenda 1000)

Los comprobantes de venta llevan la leyenda `1000` con el total en letras
(«SON …»). `cpeforge::amount_words` la genera a partir del importe y la
moneda; no hace falta escribirla a mano:

```rust
use cpeforge::amount_words::{self, AmountWordsError};
use cpeforge_core::model::sale::{BaseSale, Legend};
use cpeforge_core::Decimal;

fn main() -> Result<(), AmountWordsError> {
    let total = Decimal::new(123_456_789, 2); // 1 234 567.89

    // Leyenda lista para el comprobante, a partir del código de moneda.
    let legend = Legend::amount_in_words(total, "PEN")?;
    assert_eq!(legend.code.as_deref(), Some("1000"));
    assert_eq!(
        legend.value.as_deref(),
        Some("UN MILLÓN DOSCIENTOS TREINTA Y CUATRO MIL QUINIENTOS SESENTA Y SIETE CON 89/100 SOLES")
    );

    let venta = BaseSale {
        tipo_moneda: Some("PEN".into()),
        mto_imp_venta: Some(total),
        legends: Some(vec![legend]),
        ..BaseSale::default()
    };
    assert_eq!(venta.legends.as_ref().map(Vec::len), Some(1));

    // Solo el texto, con el nombre de moneda que se quiera imprimir.
    let texto = amount_words::to_invoice(Decimal::new(21_000, 0), "dólares americanos")?;
    assert_eq!(texto, "VEINTIÚN MIL CON 00/100 DÓLARES AMERICANOS");
    Ok(())
}
```

`cargo run -p cpeforge --example importe_en_letras` ejecuta este ejemplo.

- El importe se redondea a dos decimales (mitad lejos de cero) y admite
  hasta 15 dígitos enteros; un importe negativo o mayor devuelve
  `AmountWordsError`.
- `Legend::amount_in_words` reconoce `PEN` (SOLES), `USD` (DÓLARES
  AMERICANOS) y `EUR` (EUROS); para otra moneda usar
  `amount_words::to_invoice` con su nombre.
- `UNO` se apocopa delante de `MIL`, `MILLONES` y `BILLONES` (`VEINTIÚN MIL`)
  y se conserva al final (`MIL UNO CON 00/100 SOLES`).

## Concurrencia y volumen

- `See::send`, `send_xml` y `get_xml_signed` toman `&mut self` (el resultado
  firmado queda en `last_xml`): una fachada procesa un comprobante a la vez.
  Para procesar en paralelo, usar una fachada por tarea o hilo; cada una
  carga la clave privada una sola vez y la reutiliza.
- Construir y firmar es trabajo de CPU que se ejecuta dentro del `async fn`
  antes de la primera espera de red. Con muchos envíos, ejecutar esa parte en
  un hilo de trabajo (por ejemplo `tokio::task::spawn_blocking`).
- Ningún método reintenta un envío: un error de transporte puede haber
  llegado a SUNAT. Antes de reenviar, consultar el estado (ticket o CDR);
  `cpeforge_ws::outcome::TaxStatus` clasifica las respuestas.
- Una firma válida no implica un certificado confiable: para eso usar
  `cpeforge_xmldsig::trust::TrustPolicy`.

## Rendimiento

`tests/bench.rs` mide el flujo completo con un transporte en memoria. En una
factura de 1000 líneas (2,6 MB firmada) el envío completo cuesta ~36 ms,
dominados por la firma XML.

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
