# Benchmark: PHP y Rust

Los 43 comprobantes de [`example/`](../example) generados, firmados y enviados con la
implementación PHP (`example/php`) y con CPEForge (`example/rust`), con los
mismos datos y la misma fecha base.

**Entorno:** Apple M5 (10 núcleos), macOS 27.2. PHP 8.4.17 CLI; Rust 1.99.1
en modo release. Cada fase es la mediana de 30 llamadas por caso; las tablas
suman los 43 casos.

## Resultado funcional

| Comprobación | PHP | Rust |
| --- | --- | --- |
| XML equivalente entre ambos (sin firma ni identidad) | 43/43 | 43/43 |
| XML válido contra su XSD (UBL 2.0/2.1) | - | 43/43 |
| Aceptados por SUNAT beta / API de guías de prueba | 42/42 | 42/42 |
| Rechazos esperados con el mismo código (3617, 3315, 2992) | 3/3 | 3/3 |

`factura-beatose` no se envía: su demo apunta a un servidor de terceros.

## CPU, sin red (PHP con la configuración por defecto de la CLI)

`bash oracle/ejemplos/comparar.sh` con `EJEMPLO_REPETICIONES=30`.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| construir | 73 µs | 27 µs | Rust | ×2.7 | 42 | 1 |
| xml | 2.9 ms | 113 µs | Rust | ×25.5 | 43 | 0 |
| firma | 25.6 ms | 16.0 ms | Rust | ×1.6 | 43 | 0 |
| getXmlSigned | 31.9 ms | 16.3 ms | Rust | ×2.0 | 43 | 0 |
| 1ª llamada | 97.8 ms | 29.9 ms | Rust | ×3.3 | 43 | 0 |

## CPU con OPcache y JIT (`tracing`) en PHP

Mismas mediciones con `php -d opcache.enable_cli=1 -d opcache.jit=tracing`.
La firma depende de OpenSSL en ambos lados y el JIT apenas cambia la
generación del XML; en la primera llamada PHP paga además la compilación.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| construir | 67 µs | 27 µs | Rust | ×2.5 | 42 | 1 |
| xml | 2.2 ms | 113 µs | Rust | ×19.6 | 43 | 0 |
| firma | 25.8 ms | 16.0 ms | Rust | ×1.6 | 43 | 0 |
| getXmlSigned | 31.7 ms | 16.3 ms | Rust | ×1.9 | 43 | 0 |
| 1ª llamada | 155.3 ms | 29.9 ms | Rust | ×5.2 | 43 | 0 |

## Red: envío a SUNAT beta y a la API de guías de prueba

`bash oracle/ejemplos/enviar.sh`: una sola medición por caso, sin contar las
pausas entre envíos. Depende de la latencia del servicio de prueba en ese
momento; es orientativa.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| envío | 11.33 s | 5.61 s | Rust | ×2.0 | 39 | 3 |
| estado | 3.82 s | 3.31 s | Rust | ×1.2 | 6 | 4 |

## Detalle por caso (CPU, sin red)

## construir

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 1 µs | 2 µs | PHP | ×1.6 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 2 µs | 1 µs | Rust | ×2.0 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 1 µs | 0 µs | Rust | ×3.4 | - | - |
| factura | 20123456789-01-F001-123.xml | 2 µs | 1 µs | Rust | ×3.6 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 2 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 3 µs | 1 µs | Rust | ×3.3 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 1 µs | 1 µs | Rust | ×2.5 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 2 µs | 1 µs | Rust | ×3.6 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 2 µs | 1 µs | Rust | ×3.1 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 1 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 1 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 1 µs | 1 µs | Rust | ×2.3 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 2 µs | 1 µs | Rust | ×2.5 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 1 µs | 0 µs | Rust | ×2.8 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 1 µs | 0 µs | Rust | ×2.6 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 1 µs | 0 µs | Rust | ×3.3 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 1 µs | 0 µs | Rust | ×2.0 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 1 µs | 0 µs | Rust | ×2.8 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 4 µs | 1 µs | Rust | ×3.6 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 2 µs | 1 µs | Rust | ×2.5 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 1 µs | 1 µs | Rust | ×2.3 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 2 µs | 1 µs | Rust | ×2.5 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 2 µs | 1 µs | Rust | ×2.5 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 2 µs | 1 µs | Rust | ×3.2 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 2 µs | 0 µs | Rust | ×3.4 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 2 µs | 1 µs | Rust | ×4.3 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 1 µs | 0 µs | Rust | ×4.2 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 1 µs | 0 µs | Rust | ×3.6 | - | - |
| retencion | 20123456789-20-R001-123.xml | 2 µs | 0 µs | Rust | ×3.4 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 1 µs | 0 µs | Rust | ×3.3 | - | - |

## xml

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 60 µs | 3 µs | Rust | ×22.7 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 92 µs | 4 µs | Rust | ×22.4 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 14 µs | 1 µs | Rust | ×13.3 | - | - |
| factura | 20123456789-01-F001-123.xml | 94 µs | 3 µs | Rust | ×30.7 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 84 µs | 3 µs | Rust | ×29.1 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 68 µs | 2 µs | Rust | ×27.1 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 139 µs | 5 µs | Rust | ×27.2 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 66 µs | 2 µs | Rust | ×29.2 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 79 µs | 3 µs | Rust | ×28.7 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 94 µs | 3 µs | Rust | ×29.1 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 86 µs | 3 µs | Rust | ×28.2 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 80 µs | 3 µs | Rust | ×29.8 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 71 µs | 3 µs | Rust | ×27.3 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 65 µs | 2 µs | Rust | ×26.8 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 64 µs | 2 µs | Rust | ×28.1 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 65 µs | 2 µs | Rust | ×28.5 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 88 µs | 3 µs | Rust | ×25.4 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 89 µs | 3 µs | Rust | ×25.7 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 64 µs | 2 µs | Rust | ×28.1 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 72 µs | 3 µs | Rust | ×25.6 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 66 µs | 2 µs | Rust | ×26.6 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 71 µs | 3 µs | Rust | ×27.6 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 71 µs | 3 µs | Rust | ×27.2 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 71 µs | 2 µs | Rust | ×28.8 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 45 µs | 2 µs | Rust | ×19.4 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 36 µs | 2 µs | Rust | ×21.2 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 37 µs | 2 µs | Rust | ×19.0 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 34 µs | 2 µs | Rust | ×20.7 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 42 µs | 2 µs | Rust | ×21.9 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 102 µs | 4 µs | Rust | ×27.4 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 140 µs | 5 µs | Rust | ×25.8 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 67 µs | 2 µs | Rust | ×28.8 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 65 µs | 2 µs | Rust | ×27.0 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 74 µs | 3 µs | Rust | ×25.3 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 72 µs | 3 µs | Rust | ×25.3 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 78 µs | 3 µs | Rust | ×25.5 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 73 µs | 3 µs | Rust | ×26.1 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 34 µs | 2 µs | Rust | ×17.7 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 70 µs | 3 µs | Rust | ×21.3 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 26 µs | 2 µs | Rust | ×16.5 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 28 µs | 1 µs | Rust | ×19.2 | - | - |
| retencion | 20123456789-20-R001-123.xml | 35 µs | 2 µs | Rust | ×19.3 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 14 µs | 1 µs | Rust | ×14.2 | - | - |

## firma

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 576 µs | 456 µs | Rust | ×1.3 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 655 µs | 467 µs | Rust | ×1.4 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 562 µs | 344 µs | Rust | ×1.6 | - | - |
| factura | 20123456789-01-F001-123.xml | 659 µs | 382 µs | Rust | ×1.7 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 619 µs | 376 µs | Rust | ×1.6 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 601 µs | 362 µs | Rust | ×1.7 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 671 µs | 431 µs | Rust | ×1.6 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 600 µs | 359 µs | Rust | ×1.7 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 603 µs | 371 µs | Rust | ×1.6 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 636 µs | 379 µs | Rust | ×1.7 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 606 µs | 388 µs | Rust | ×1.6 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 610 µs | 371 µs | Rust | ×1.6 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 589 µs | 365 µs | Rust | ×1.6 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 584 µs | 358 µs | Rust | ×1.6 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 575 µs | 364 µs | Rust | ×1.6 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 585 µs | 366 µs | Rust | ×1.6 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 609 µs | 380 µs | Rust | ×1.6 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 620 µs | 384 µs | Rust | ×1.6 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 584 µs | 366 µs | Rust | ×1.6 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 595 µs | 374 µs | Rust | ×1.6 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 580 µs | 365 µs | Rust | ×1.6 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 594 µs | 365 µs | Rust | ×1.6 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 582 µs | 373 µs | Rust | ×1.6 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 581 µs | 368 µs | Rust | ×1.6 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 571 µs | 352 µs | Rust | ×1.6 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 578 µs | 347 µs | Rust | ×1.7 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 562 µs | 351 µs | Rust | ×1.6 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 563 µs | 342 µs | Rust | ×1.6 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 579 µs | 349 µs | Rust | ×1.7 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 634 µs | 399 µs | Rust | ×1.6 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 668 µs | 423 µs | Rust | ×1.6 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 582 µs | 367 µs | Rust | ×1.6 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 581 µs | 368 µs | Rust | ×1.6 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 602 µs | 386 µs | Rust | ×1.6 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 603 µs | 371 µs | Rust | ×1.6 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 607 µs | 377 µs | Rust | ×1.6 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 605 µs | 378 µs | Rust | ×1.6 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 563 µs | 350 µs | Rust | ×1.6 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 640 µs | 392 µs | Rust | ×1.6 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 552 µs | 348 µs | Rust | ×1.6 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 554 µs | 339 µs | Rust | ×1.6 | - | - |
| retencion | 20123456789-20-R001-123.xml | 557 µs | 343 µs | Rust | ×1.6 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 539 µs | 328 µs | Rust | ×1.6 | - | - |

## getXmlSigned

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 731 µs | 548 µs | Rust | ×1.3 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 851 µs | 520 µs | Rust | ×1.6 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 653 µs | 368 µs | Rust | ×1.8 | - | - |
| factura | 20123456789-01-F001-123.xml | 827 µs | 385 µs | Rust | ×2.1 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 839 µs | 373 µs | Rust | ×2.2 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 754 µs | 370 µs | Rust | ×2.0 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 905 µs | 431 µs | Rust | ×2.1 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 727 µs | 368 µs | Rust | ×2.0 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 776 µs | 376 µs | Rust | ×2.1 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 800 µs | 387 µs | Rust | ×2.1 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 790 µs | 380 µs | Rust | ×2.1 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 789 µs | 380 µs | Rust | ×2.1 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 743 µs | 376 µs | Rust | ×2.0 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 725 µs | 368 µs | Rust | ×2.0 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 713 µs | 368 µs | Rust | ×1.9 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 735 µs | 364 µs | Rust | ×2.0 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 780 µs | 385 µs | Rust | ×2.0 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 783 µs | 400 µs | Rust | ×2.0 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 721 µs | 362 µs | Rust | ×2.0 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 750 µs | 380 µs | Rust | ×2.0 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 726 µs | 372 µs | Rust | ×2.0 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 739 µs | 374 µs | Rust | ×2.0 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 732 µs | 369 µs | Rust | ×2.0 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 731 µs | 376 µs | Rust | ×1.9 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 699 µs | 358 µs | Rust | ×2.0 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 664 µs | 349 µs | Rust | ×1.9 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 672 µs | 350 µs | Rust | ×1.9 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 665 µs | 350 µs | Rust | ×1.9 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 679 µs | 357 µs | Rust | ×1.9 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 810 µs | 405 µs | Rust | ×2.0 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 895 µs | 429 µs | Rust | ×2.1 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 723 µs | 368 µs | Rust | ×2.0 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 716 µs | 368 µs | Rust | ×1.9 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 748 µs | 378 µs | Rust | ×2.0 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 746 µs | 379 µs | Rust | ×2.0 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 763 µs | 382 µs | Rust | ×2.0 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 751 µs | 381 µs | Rust | ×2.0 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 664 µs | 346 µs | Rust | ×1.9 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 811 µs | 400 µs | Rust | ×2.0 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 662 µs | 347 µs | Rust | ×1.9 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 652 µs | 340 µs | Rust | ×1.9 | - | - |
| retencion | 20123456789-20-R001-123.xml | 673 µs | 349 µs | Rust | ×1.9 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 578 µs | 333 µs | Rust | ×1.7 | - | - |

## 1ª llamada

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 33.8 ms | 13.0 ms | Rust | ×2.6 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 952 µs | 561 µs | Rust | ×1.7 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 2.0 ms | 393 µs | Rust | ×5.0 | - | - |
| factura | 20123456789-01-F001-123.xml | 896 µs | 403 µs | Rust | ×2.2 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 1.4 ms | 376 µs | Rust | ×3.7 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 841 µs | 370 µs | Rust | ×2.3 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 1.8 ms | 439 µs | Rust | ×4.2 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 824 µs | 360 µs | Rust | ×2.3 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 836 µs | 532 µs | Rust | ×1.6 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 958 µs | 382 µs | Rust | ×2.5 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 891 µs | 374 µs | Rust | ×2.4 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 841 µs | 426 µs | Rust | ×2.0 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 920 µs | 379 µs | Rust | ×2.4 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 786 µs | 368 µs | Rust | ×2.1 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 732 µs | 373 µs | Rust | ×2.0 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 747 µs | 368 µs | Rust | ×2.0 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 826 µs | 535 µs | Rust | ×1.5 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 957 µs | 416 µs | Rust | ×2.3 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 753 µs | 363 µs | Rust | ×2.1 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 794 µs | 375 µs | Rust | ×2.1 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 743 µs | 381 µs | Rust | ×1.9 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 869 µs | 380 µs | Rust | ×2.3 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 782 µs | 370 µs | Rust | ×2.1 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 770 µs | 388 µs | Rust | ×2.0 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 5.1 ms | 524 µs | Rust | ×9.7 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 691 µs | 350 µs | Rust | ×2.0 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 704 µs | 344 µs | Rust | ×2.0 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 687 µs | 426 µs | Rust | ×1.6 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 721 µs | 363 µs | Rust | ×2.0 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 873 µs | 420 µs | Rust | ×2.1 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 966 µs | 436 µs | Rust | ×2.2 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 743 µs | 380 µs | Rust | ×2.0 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 753 µs | 522 µs | Rust | ×1.4 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 7.6 ms | 409 µs | Rust | ×18.7 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 800 µs | 395 µs | Rust | ×2.0 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 814 µs | 376 µs | Rust | ×2.2 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 7.8 ms | 380 µs | Rust | ×20.6 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 3.2 ms | 346 µs | Rust | ×9.3 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 4.5 ms | 414 µs | Rust | ×10.9 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 835 µs | 352 µs | Rust | ×2.4 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 689 µs | 481 µs | Rust | ×1.4 | - | - |
| retencion | 20123456789-20-R001-123.xml | 3.9 ms | 349 µs | Rust | ×11.1 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 628 µs | 335 µs | Rust | ×1.9 | - | - |
