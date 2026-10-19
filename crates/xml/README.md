# cpeforge-xml

XML UBL sin firma de comprobantes electrónicos peruanos. Recibe los modelos de
`cpeforge-core`.

## Alcance

Siete builders y 13 plantillas: factura y boleta UBL 2.0/2.1 (con guía
embebida), notas de crédito y débito 2.0/2.1, guía de remisión anterior y
2022, resumen diario, baja y reversión, retención y percepción. Opciones:
escape `html` o desactivado, `strict_variables` y reloj inyectable para las
fechas nulas.

**No incluye** firma, compresión, envío a SUNAT ni PDF.

## Seguridad e integridad del XML

- Con `Autoescape::Html` (valor por defecto) los textos se escapan como
  `htmlspecialchars(ENT_QUOTES)`. Con `Autoescape::Disabled` (el modo que usa
  la fachada `cpeforge`) se insertan tal cual: un `&` o `<` produce XML mal
  formado, así que esos textos deben llegar ya escapados.
- Los textos impresos dentro de `<![CDATA[…]]>` (descripciones, razones
  sociales, direcciones, observaciones, motivos) no se escapan. Un `]]>` en
  el valor se divide en `]]]]><![CDATA[>`: el XML sigue bien formado y
  conserva el texto, sin permitir inyectar elementos UBL. `tests/injection.rs`
  lo comprueba en todos los comprobantes.
- Los importes son `Decimal` exactos; no se usa `float`.
- La firma se referencia como `#CPEFORGE-SIGN` en `cac:Signature`.

## Conformidad SUNAT

- La factura, la boleta y las notas en UBL 2.0 y la guía de remisión anterior
  (`despatch`) existen por compatibilidad. SUNAT exige UBL 2.1 para
  comprobantes de pago y la guía 2022 (`DespatchBuilder` con
  `version = "2022"`) para la guía electrónica.
- La validación XSD (`tests/schema.rs`) documenta los casos que aún generan
  XML no válido: cantidades menores que `1e-4` impresas como `1.0E-6`
  (no admitido por `xsd:decimal`), cargos globales sin factor
  (`MultiplierFactorNumeric` vacío), importes con más dígitos de los admitidos
  y retenciones con varios `cac:Payment`.

## Rendimiento

`cargo test --release -p cpeforge-xml --all-features --test bench -- --ignored --nocapture`
(solo render, Apple Silicon, mediana de 5 ejecuciones):

| Comprobante (1000 líneas) | Tiempo |
| --- | --- |
| Factura UBL 2.1 (2,7 MB) | 1,69 ms |
| Factura UBL 2.0 (1,4 MB) | 0,76 ms |
| Nota de crédito 2.1 (2,0 MB) | 1,19 ms |
| Guía 2022 (0,5 MB) | 0,31 ms |
| Resumen diario (1,5 MB) | 0,82 ms |

Pico de memoria al renderizar una factura de 5000 líneas (13,3 MB de XML):
20,7 MB.

## Ejemplos

```bash
cargo run -p cpeforge-xml --example invoice_xml
cargo run -p cpeforge-xml --example despatch_2022_xml
```

## Estado

Versión 0.1.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
