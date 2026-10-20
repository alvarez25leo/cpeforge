# cpeforge-report

Representación impresa HTML de comprobantes electrónicos peruanos (siete
plantillas, CSS y QR SVG). Recibe los modelos de `cpeforge-core`.

## Alcance

- `HtmlReport` (contrato `Report` de `cpeforge-core`): siete plantillas
  (`invoice`, `invoice2`, `despatch`, `perception`, `retention`, `summary`,
  `voided`) y su CSS. La plantilla inicial es `invoice.html.twig`; se cambia
  con `set_template`. Opciones: carpeta de plantillas propias,
  `strict_variables`, escape automático, globales (`max_items`) y reloj
  inyectable para `date(null)`.
- `DefaultTemplateResolver`: plantilla por tipo de documento.
- `qr`: SVG del QR de ventas, guías y URL (`qrcodegen` codifica; la máscara,
  la versión y la serialización SVG son propias).
- `XmlUtils`: `DigestValue` de un XML firmado (no verifica la firma).
- `format`: `decimal_to_f64`, `number_format`, `float_to_string`,
  `round_half_up` y `format_date`.

**No incluye** PDF, firma ni envío a SUNAT.

## Seguridad

- `user.header` y `user.footer` se imprimen sin escapar: son `TrustedHtml`.
  El resto de textos se escapa como `htmlspecialchars(ENT_QUOTES)`.
- Las plantillas propias no pueden salir de su carpeta (`..`, rutas absolutas
  o con unidad, enlaces simbólicos), deben ser archivos regulares, tienen
  tamaño máximo (1 MiB) y el motor no ofrece acceso a archivos, red ni
  entorno. Una plantilla propia es código de confianza: puede desactivar el
  escape (`|raw`, `|safe`, `{% autoescape false %}`) y `fuel` limita sus
  instrucciones pero no la memoria de filtros u operadores. El HTML generado
  tiene un máximo de 32 MiB (`max_output_bytes`), `n_format` admite hasta 100
  decimales y el QR rechaza contenidos mayores que la versión 40 antes de
  codificarlos. La carpeta de plantillas no debe ser escribible por terceros
  (la comprobación de rutas no cubre enlaces duros ni cambios simultáneos).
- El HTML lo convierte a PDF un navegador: ejecutarlo sin JavaScript y sin
  salida a red (Gotenberg con `--chromium-disable-javascript` y egress
  bloqueado) como defensa en profundidad.
- Logo de hasta 10 MiB por defecto (`max_logo_bytes`).
- `XmlUtils` rechaza DTD, XML mal formado, anidamiento mayor de 64 niveles
  (configurable), más de 1 000 000 de nodos (`MAX_NODES`), elementos con más
  de 256 atributos y ámbitos de espacios de nombres desmedidos
  (`cpeforge_xml_limits::check_tree_markup`).

## Importes

En la frontera de la plantilla cada `Decimal` se convierte en el `double` más
cercano (como `strtod`) y `number_format` y la impresión usan redondeo
half-up sobre ese valor. No se hace aritmética con esos valores ni se
recalculan impuestos: se muestran los del documento. `tests/amounts.rs` acota
la diferencia con el redondeo decimal exacto: son iguales mientras
`|importe| · 10^decimales < 2^50` (con dos decimales, hasta ≈ 11 billones) y
el importe tenga como mucho 15 cifras significativas. Esta conversión es parte
del contrato: solo afecta a cómo se muestran los importes, nunca a su cálculo.

## Plantillas propias

`ReportOptions::template_dir` se consulta antes que las plantillas
integradas, así que un archivo `invoice.html.twig` o `assets/style.css` propio
las sustituye. Las plantillas se escriben y ejecutan en el dialecto
**MiniJinja** (contrato del crate; no se implementará la sintaxis exclusiva de
Twig): la sintaxis común
(`{{ }}`, `{% if/for/set/include %}`, `|filtro`, `is defined`, `loop.index`)
funciona, pero no operadores como `?:`, `??` o `..`, ni etiquetas como
`apply`, `with`, `embed` o `sandbox`.

- `? :`, `?:`, `??`, `apply`, `with` y `starts with` son errores de sintaxis
  (`ReportError::Syntax`). Equivalentes: `a if cond else b`,
  `x|default('…')`, `x is twig_true`.
- **El rango `1..3` no falla**: se evalúa como indefinido y se imprime vacío,
  también con `strict_variables`. Usar `range(1, 4)`.
- `|format` usa el formato `%` de Python (próximo a `sprintf`).
- Cada plantilla se valida al cargarla (también las de `include`, `extends` e
  `import`): un filtro, test o función que no existe es `ReportError::Syntax`
  aunque esté en una rama que nunca se ejecuta.

Filtros, funciones y tests disponibles:

| Nombre | Tipo | Comportamiento |
| --- | --- | --- |
| `catalog(código)` | filtro | Texto del catálogo SUNAT; `''` si no existe |
| `image_b64(mime='')` | filtro | `data:image/<mime>;base64,…`; detecta PNG, GIF o JPEG |
| `n_format(decimales=2)` | filtro | `number_format($x, $d, '.', '')`; `null` o texto → error |
| `date(formato='F j, Y H:i')` | filtro | Formato de fecha de una letra en hora de Lima; `null` → ahora |
| `raw`, `escape`/`e`, `upper`, `lower`, `length`, `default` | filtro | Filtros de texto habituales |
| `twig_string` | filtro | Conversión a texto (`null` → `''`, `18.0` → `18`) |
| `legend(leyendas, código)` | función | Valor de la primera leyenda con ese código |
| `qrCode(venta)`, `qrCodeDespatch(guía)`, `qrUrl(url)` | función | SVG del QR |
| `twig_true`, `twig_in(x)`, `twig_eq(x)` | test | Verdad, pertenencia e igualdad laxas |

Las variables son `doc` (getters del modelo, sin distinguir mayúsculas),
`params` y los globales añadidos con `HtmlReport::add_global`.

## Rendimiento y concurrencia

`HtmlReport` es `Send + Sync`: se crea una vez y se comparte entre hilos; las
plantillas compiladas se reutilizan.

```text
cargo bench -p cpeforge-report --bench render            # latencia y throughput
cargo bench -p cpeforge-report --bench render -- --dump D  # salidas para diff
```

Medición local (Apple Silicon, p50): factura completa con QR 124 µs, 10 000
líneas 11,9 ms (+13,5 MiB por render), QR de venta 92 µs.

## Estado

Versión estable. La API pública sigue SemVer: los errores, resultados y
opciones pueden crecer sin romper el código que la usa (ver el
[README del repositorio](https://github.com/alvarez25leo/cpeforge#estado)).

## Licencia

MIT. Ver `LICENSE` y, para el material de terceros incluido (plantillas, CSS y
catálogos), `LICENSE-THIRD-PARTY` y `NOTICE`.
