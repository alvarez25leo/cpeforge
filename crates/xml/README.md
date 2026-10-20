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
  formado, así que esos textos deben llegar ya escapados. Son las dos únicas
  estrategias (contrato del crate): un XML UBL no contiene JavaScript, CSS ni
  URL que necesiten otro escape, así que no se añadirán `js`, `css` ni `url`.
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
- Todo XML generado valida contra el XSD (`tests/schema.rs`, con `xmllint`)
  salvo los documentos vacíos o incompletos y los textos sin escapar con
  `Autoescape::Disabled`. Para lograrlo:
  - cantidades, porcentajes e importes de cargos de línea se escriben en
    notación posicional (`0.000001`, `100000000000000`), nunca con exponente;
  - los importes con límite de decimales desde `1e14` no se rellenan con
    ceros (`1000000000000000`, no `1000000000000000.0000000000`, que supera
    la precisión de `xsd:decimal` en libxml2);
  - un cargo o descuento sin factor omite `cbc:MultiplierFactorNumeric` en
    vez de dejarlo vacío;
  - una retención o percepción con más de un pago o cobro en un comprobante
    relacionado devuelve `XmlError::MultiplePayments`: el esquema admite un
    `cac:Payment` por comprobante, así que cada pago se informa como un
    comprobante relacionado propio.
- `cpeforge_xml::format` conserva además las conversiones exactas
  (`float_string`, `n_format`, `n_format_limit`) y ofrece las variantes
  válidas para XSD (`xsd_float_string`, `xsd_n_format_limit`).

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

Versión 0.2.0 en desarrollo. Lo que falta para la primera versión estable está en
[docs/pendientes.md](https://github.com/alvarez25leo/cpeforge/blob/main/docs/pendientes.md).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido, `LICENSE-THIRD-PARTY` y `NOTICE`.
