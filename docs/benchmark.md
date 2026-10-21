# Benchmark: PHP y Rust

Los 44 comprobantes de [`example/`](../example) generados, firmados y enviados
con la implementación PHP (`example/php`) y con CPEForge (`example/rust`), con
los mismos datos y la misma fecha base. Rust genera además 4 casos propios
(`guia-transportista`, `liquidacion-compra`, `liquidacion-compra-oro` y
`reversion-liquidacion`), que se validan y envían pero no entran en la
comparación.

**Entorno:** Apple M5 (10 núcleos), macOS 27.2. PHP 8.4.17 CLI; Rust 1.99.1
en modo release. Cada fase es la mediana de 30 llamadas por caso; las tablas
suman los 44 casos. Corrida del 2026-10-20 con cpeforge-core 0.2.2,
cpeforge-xml 0.3.2, cpeforge-xmldsig 0.2.2, cpeforge-gre-api 0.2.1 y
cpeforge-ws 0.3.1.

## Resultado funcional

| Comprobación | PHP | Rust |
| --- | --- | --- |
| XML equivalente entre ambos (sin firma ni identidad) | 44/44 | 44/44 |
| XML válido contra su XSD (UBL 2.0/2.1) | - | 48/48 |
| Aceptados por SUNAT beta / API de guías de prueba | 43/43 | 47/47 |
| Rechazos esperados con el mismo código (3617, 3315, 2992; 2800 solo Rust) | 3/3 | 4/4 |

`factura-beatose` no se envía: su demo apunta a un servidor de terceros.

## CPU, sin red (PHP con la configuración por defecto de la CLI)

`bash oracle/ejemplos/comparar.sh` con `EJEMPLO_REPETICIONES=30`.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| construir | 73 µs | 28 µs | Rust | ×2.6 | 43 | 1 |
| xml | 3.0 ms | 47 µs | Rust | ×63.6 | 44 | 0 |
| firma | 26.8 ms | 15.7 ms | Rust | ×1.7 | 44 | 0 |
| getXmlSigned | 33.1 ms | 16.0 ms | Rust | ×2.1 | 44 | 0 |
| 1ª llamada | 103.4 ms | 28.9 ms | Rust | ×3.6 | 44 | 0 |

## CPU con OPcache y JIT (`tracing`) en PHP

Mismas mediciones con `php -d opcache.enable_cli=1 -d opcache.jit=tracing`.
La firma depende de OpenSSL en ambos lados y el JIT apenas cambia la
generación del XML; en la primera llamada PHP paga además la compilación.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| construir | 69 µs | 27 µs | Rust | ×2.6 | 43 | 1 |
| xml | 2.2 ms | 47 µs | Rust | ×47.7 | 44 | 0 |
| firma | 26.6 ms | 15.5 ms | Rust | ×1.7 | 44 | 0 |
| getXmlSigned | 32.9 ms | 16.5 ms | Rust | ×2.0 | 43 | 1 |
| 1ª llamada | 150.4 ms | 30.4 ms | Rust | ×4.9 | 43 | 1 |

## Red: envío a SUNAT beta y a la API de guías de prueba

`bash oracle/ejemplos/enviar.sh`: una sola medición por caso, sin contar las
pausas entre envíos. Depende de la latencia del servicio de prueba en ese
momento; es orientativa. En esta corrida el total de Rust lo explica un solo
caso: `factura-descuento-global` tardó 9,21 s en responder (86 ms en PHP un
instante antes); sin él, el envío suma 10,40 s en Rust frente a 10,58 s en
PHP. Rust fue más rápido en 37 de los 43 envíos.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| envío | 10.67 s | 19.61 s | PHP | ×1.8 | 37 | 6 |
| estado | 4.31 s | 3.18 s | Rust | ×1.4 | 6 | 4 |

## Detalle por caso (CPU, sin red)

## construir

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 1 µs | 3 µs | PHP | ×2.2 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 2 µs | 1 µs | Rust | ×2.1 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 1 µs | 0 µs | Rust | ×3.3 | - | - |
| factura | 20123456789-01-F001-123.xml | 2 µs | 1 µs | Rust | ×3.0 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 3 µs | 1 µs | Rust | ×3.6 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 2 µs | 1 µs | Rust | ×3.3 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 2 µs | 1 µs | Rust | ×2.1 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 2 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 1 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 1 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 1 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 1 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 2 µs | 1 µs | Rust | ×2.1 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 1 µs | 0 µs | Rust | ×2.8 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 1 µs | 0 µs | Rust | ×2.5 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 1 µs | 0 µs | Rust | ×2.9 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 1 µs | 0 µs | Rust | ×2.6 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 1 µs | 0 µs | Rust | ×2.5 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 2 µs | 1 µs | Rust | ×3.1 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 4 µs | 1 µs | Rust | ×3.9 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 1 µs | 1 µs | Rust | ×2.3 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 2 µs | 1 µs | Rust | ×3.0 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 2 µs | 1 µs | Rust | ×2.5 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 2 µs | 0 µs | Rust | ×3.6 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 2 µs | 0 µs | Rust | ×4.5 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 1 µs | 0 µs | Rust | ×3.6 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 1 µs | 0 µs | Rust | ×3.6 | - | - |
| retencion | 20123456789-20-R001-123.xml | 2 µs | 0 µs | Rust | ×3.4 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 1 µs | 0 µs | Rust | ×3.6 | - | - |

## xml

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 67 µs | 1 µs | Rust | ×49.0 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 93 µs | 1 µs | Rust | ×63.9 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 14 µs | 0 µs | Rust | ×33.1 | - | - |
| factura | 20123456789-01-F001-123.xml | 93 µs | 1 µs | Rust | ×74.5 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 79 µs | 1 µs | Rust | ×70.5 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 71 µs | 1 µs | Rust | ×74.6 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 74 µs | 1 µs | Rust | ×77.3 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 149 µs | 2 µs | Rust | ×74.3 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 70 µs | 1 µs | Rust | ×73.4 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 78 µs | 1 µs | Rust | ×64.2 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 88 µs | 1 µs | Rust | ×75.6 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 87 µs | 1 µs | Rust | ×74.3 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 76 µs | 1 µs | Rust | ×75.8 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 71 µs | 1 µs | Rust | ×65.3 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 65 µs | 1 µs | Rust | ×71.0 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 62 µs | 1 µs | Rust | ×67.7 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 67 µs | 1 µs | Rust | ×69.7 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 89 µs | 1 µs | Rust | ×71.5 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 92 µs | 1 µs | Rust | ×69.0 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 67 µs | 1 µs | Rust | ×73.5 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 74 µs | 1 µs | Rust | ×70.6 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 70 µs | 1 µs | Rust | ×67.6 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 73 µs | 1 µs | Rust | ×73.1 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 70 µs | 1 µs | Rust | ×72.6 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 70 µs | 1 µs | Rust | ×70.1 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 44 µs | 1 µs | Rust | ×48.1 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 36 µs | 1 µs | Rust | ×51.1 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 38 µs | 1 µs | Rust | ×42.2 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 34 µs | 1 µs | Rust | ×47.4 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 45 µs | 1 µs | Rust | ×60.2 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 103 µs | 2 µs | Rust | ×56.2 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 140 µs | 2 µs | Rust | ×64.7 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 66 µs | 1 µs | Rust | ×69.2 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 66 µs | 1 µs | Rust | ×68.4 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 77 µs | 1 µs | Rust | ×68.7 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 72 µs | 1 µs | Rust | ×55.9 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 83 µs | 1 µs | Rust | ×68.9 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 71 µs | 1 µs | Rust | ×61.1 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 33 µs | 1 µs | Rust | ×33.4 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 70 µs | 1 µs | Rust | ×58.3 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 26 µs | 1 µs | Rust | ×45.5 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 28 µs | 1 µs | Rust | ×44.7 | - | - |
| retencion | 20123456789-20-R001-123.xml | 36 µs | 1 µs | Rust | ×33.3 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 15 µs | 0 µs | Rust | ×35.3 | - | - |

## firma

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 677 µs | 493 µs | Rust | ×1.4 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 675 µs | 403 µs | Rust | ×1.7 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 591 µs | 329 µs | Rust | ×1.8 | - | - |
| factura | 20123456789-01-F001-123.xml | 670 µs | 365 µs | Rust | ×1.8 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 628 µs | 357 µs | Rust | ×1.8 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 621 µs | 346 µs | Rust | ×1.8 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 625 µs | 349 µs | Rust | ×1.8 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 696 µs | 393 µs | Rust | ×1.8 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 604 µs | 347 µs | Rust | ×1.7 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 606 µs | 363 µs | Rust | ×1.7 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 620 µs | 364 µs | Rust | ×1.7 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 625 µs | 361 µs | Rust | ×1.7 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 604 µs | 359 µs | Rust | ×1.7 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 611 µs | 356 µs | Rust | ×1.7 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 594 µs | 351 µs | Rust | ×1.7 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 596 µs | 352 µs | Rust | ×1.7 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 580 µs | 351 µs | Rust | ×1.7 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 609 µs | 362 µs | Rust | ×1.7 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 632 µs | 368 µs | Rust | ×1.7 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 596 µs | 354 µs | Rust | ×1.7 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 628 µs | 358 µs | Rust | ×1.8 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 604 µs | 355 µs | Rust | ×1.7 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 607 µs | 350 µs | Rust | ×1.7 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 596 µs | 348 µs | Rust | ×1.7 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 599 µs | 353 µs | Rust | ×1.7 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 590 µs | 349 µs | Rust | ×1.7 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 577 µs | 341 µs | Rust | ×1.7 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 568 µs | 346 µs | Rust | ×1.6 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 566 µs | 342 µs | Rust | ×1.7 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 569 µs | 344 µs | Rust | ×1.7 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 625 µs | 378 µs | Rust | ×1.7 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 673 µs | 389 µs | Rust | ×1.7 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 597 µs | 348 µs | Rust | ×1.7 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 592 µs | 350 µs | Rust | ×1.7 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 624 µs | 357 µs | Rust | ×1.7 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 611 µs | 353 µs | Rust | ×1.7 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 609 µs | 357 µs | Rust | ×1.7 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 608 µs | 360 µs | Rust | ×1.7 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 568 µs | 335 µs | Rust | ×1.7 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 624 µs | 364 µs | Rust | ×1.7 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 557 µs | 334 µs | Rust | ×1.7 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 565 µs | 338 µs | Rust | ×1.7 | - | - |
| retencion | 20123456789-20-R001-123.xml | 578 µs | 340 µs | Rust | ×1.7 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 558 µs | 331 µs | Rust | ×1.7 | - | - |

## getXmlSigned

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 813 µs | 675 µs | Rust | ×1.2 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 919 µs | 443 µs | Rust | ×2.1 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 639 µs | 340 µs | Rust | ×1.9 | - | - |
| factura | 20123456789-01-F001-123.xml | 851 µs | 361 µs | Rust | ×2.4 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 802 µs | 352 µs | Rust | ×2.3 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 768 µs | 352 µs | Rust | ×2.2 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 767 µs | 357 µs | Rust | ×2.1 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 942 µs | 394 µs | Rust | ×2.4 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 760 µs | 351 µs | Rust | ×2.2 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 757 µs | 367 µs | Rust | ×2.1 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 777 µs | 364 µs | Rust | ×2.1 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 774 µs | 366 µs | Rust | ×2.1 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 741 µs | 361 µs | Rust | ×2.1 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 736 µs | 359 µs | Rust | ×2.1 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 730 µs | 350 µs | Rust | ×2.1 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 730 µs | 347 µs | Rust | ×2.1 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 728 µs | 353 µs | Rust | ×2.1 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 798 µs | 365 µs | Rust | ×2.2 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 793 µs | 372 µs | Rust | ×2.1 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 736 µs | 352 µs | Rust | ×2.1 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 769 µs | 356 µs | Rust | ×2.2 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 743 µs | 354 µs | Rust | ×2.1 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 767 µs | 362 µs | Rust | ×2.1 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 770 µs | 358 µs | Rust | ×2.2 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 756 µs | 358 µs | Rust | ×2.1 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 709 µs | 349 µs | Rust | ×2.0 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 683 µs | 349 µs | Rust | ×2.0 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 688 µs | 337 µs | Rust | ×2.0 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 667 µs | 342 µs | Rust | ×1.9 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 689 µs | 346 µs | Rust | ×2.0 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 818 µs | 380 µs | Rust | ×2.2 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 907 µs | 392 µs | Rust | ×2.3 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 738 µs | 350 µs | Rust | ×2.1 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 733 µs | 352 µs | Rust | ×2.1 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 769 µs | 354 µs | Rust | ×2.2 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 768 µs | 360 µs | Rust | ×2.1 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 772 µs | 360 µs | Rust | ×2.1 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 759 µs | 357 µs | Rust | ×2.1 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 680 µs | 341 µs | Rust | ×2.0 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 778 µs | 359 µs | Rust | ×2.2 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 676 µs | 340 µs | Rust | ×2.0 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 663 µs | 339 µs | Rust | ×2.0 | - | - |
| retencion | 20123456789-20-R001-123.xml | 668 µs | 343 µs | Rust | ×1.9 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 617 µs | 333 µs | Rust | ×1.9 | - | - |

## 1ª llamada

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 38.0 ms | 12.3 ms | Rust | ×3.1 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 1.1 ms | 489 µs | Rust | ×2.3 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 2.7 ms | 402 µs | Rust | ×6.8 | - | - |
| factura | 20123456789-01-F001-123.xml | 1.0 ms | 368 µs | Rust | ×2.8 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 1.1 ms | 346 µs | Rust | ×3.2 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 808 µs | 357 µs | Rust | ×2.3 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 1.6 ms | 343 µs | Rust | ×4.7 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 1.0 ms | 391 µs | Rust | ×2.6 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 808 µs | 553 µs | Rust | ×1.5 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 799 µs | 371 µs | Rust | ×2.2 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 826 µs | 358 µs | Rust | ×2.3 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 806 µs | 404 µs | Rust | ×2.0 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 785 µs | 364 µs | Rust | ×2.2 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 778 µs | 358 µs | Rust | ×2.2 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 767 µs | 337 µs | Rust | ×2.3 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 741 µs | 342 µs | Rust | ×2.2 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 832 µs | 482 µs | Rust | ×1.7 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 870 µs | 415 µs | Rust | ×2.1 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 817 µs | 382 µs | Rust | ×2.1 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 762 µs | 350 µs | Rust | ×2.2 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 839 µs | 370 µs | Rust | ×2.3 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 839 µs | 367 µs | Rust | ×2.3 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 854 µs | 370 µs | Rust | ×2.3 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 822 µs | 367 µs | Rust | ×2.2 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 889 µs | 521 µs | Rust | ×1.7 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 5.9 ms | 372 µs | Rust | ×15.8 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 771 µs | 347 µs | Rust | ×2.2 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 720 µs | 344 µs | Rust | ×2.1 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 690 µs | 372 µs | Rust | ×1.9 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 732 µs | 350 µs | Rust | ×2.1 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 878 µs | 402 µs | Rust | ×2.2 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 1.0 ms | 548 µs | Rust | ×1.9 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 755 µs | 355 µs | Rust | ×2.1 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 744 µs | 349 µs | Rust | ×2.1 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 8.3 ms | 370 µs | Rust | ×22.4 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 773 µs | 353 µs | Rust | ×2.2 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 810 µs | 360 µs | Rust | ×2.2 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 7.7 ms | 569 µs | Rust | ×13.6 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 3.5 ms | 365 µs | Rust | ×9.5 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 3.7 ms | 359 µs | Rust | ×10.4 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 684 µs | 354 µs | Rust | ×1.9 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 670 µs | 329 µs | Rust | ×2.0 | - | - |
| retencion | 20123456789-20-R001-123.xml | 2.9 ms | 338 µs | Rust | ×8.7 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 715 µs | 329 µs | Rust | ×2.2 | - | - |
