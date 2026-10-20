# Benchmark: PHP y Rust

Los 44 comprobantes de [`example/`](../example) generados, firmados y enviados
con la implementación PHP (`example/php`) y con CPEForge (`example/rust`), con
los mismos datos y la misma fecha base.

**Entorno:** Apple M5 (10 núcleos), macOS 27.2. PHP 8.4.17 CLI; Rust 1.99.1
en modo release. Cada fase es la mediana de 30 llamadas por caso; las tablas
suman los 44 casos.

## Resultado funcional

| Comprobación | PHP | Rust |
| --- | --- | --- |
| XML equivalente entre ambos (sin firma ni identidad) | 44/44 | 44/44 |
| XML válido contra su XSD (UBL 2.0/2.1) | - | 44/44 |
| Aceptados por SUNAT beta / API de guías de prueba | 43/43 | 43/43 |
| Rechazos esperados con el mismo código (3617, 3315, 2992) | 3/3 | 3/3 |

`factura-beatose` no se envía: su demo apunta a un servidor de terceros.

## CPU, sin red (PHP con la configuración por defecto de la CLI)

`bash oracle/ejemplos/comparar.sh` con `EJEMPLO_REPETICIONES=30`.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| construir | 73 µs | 25 µs | Rust | ×2.9 | 44 | 0 |
| xml | 2.9 ms | 113 µs | Rust | ×25.8 | 44 | 0 |
| firma | 25.7 ms | 15.9 ms | Rust | ×1.6 | 44 | 0 |
| getXmlSigned | 31.8 ms | 16.2 ms | Rust | ×2.0 | 44 | 0 |
| 1ª llamada | 80.1 ms | 21.2 ms | Rust | ×3.8 | 44 | 0 |

## CPU con OPcache y JIT (`tracing`) en PHP

Mismas mediciones con `php -d opcache.enable_cli=1 -d opcache.jit=tracing`.
La firma depende de OpenSSL en ambos lados y el JIT apenas cambia la
generación del XML; en la primera llamada PHP paga además la compilación.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| construir | 66 µs | 25 µs | Rust | ×2.6 | 43 | 0 |
| xml | 2.2 ms | 113 µs | Rust | ×19.3 | 44 | 0 |
| firma | 25.6 ms | 15.9 ms | Rust | ×1.6 | 44 | 0 |
| getXmlSigned | 31.0 ms | 16.2 ms | Rust | ×1.9 | 44 | 0 |
| 1ª llamada | 132.7 ms | 21.2 ms | Rust | ×6.3 | 44 | 0 |

## Red: envío a SUNAT beta y a la API de guías de prueba

`bash oracle/ejemplos/enviar.sh`: una sola medición por caso, sin contar las
pausas entre envíos. Depende de la latencia del servicio de prueba en ese
momento; es orientativa.

| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| envío | 8.03 s | 5.58 s | Rust | ×1.4 | 41 | 2 |
| estado | 3.19 s | 3.36 s | PHP | ×1.1 | 4 | 6 |

## Detalle por caso (CPU, sin red)

## construir

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 1 µs | 1 µs | Rust | ×1.1 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 2 µs | 1 µs | Rust | ×2.1 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 1 µs | 0 µs | Rust | ×3.6 | - | - |
| factura | 20123456789-01-F001-123.xml | 2 µs | 1 µs | Rust | ×3.1 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 3 µs | 1 µs | Rust | ×3.4 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 1 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 2 µs | 1 µs | Rust | ×3.7 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 2 µs | 1 µs | Rust | ×3.0 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 1 µs | 0 µs | Rust | ×2.6 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 1 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 1 µs | 1 µs | Rust | ×2.4 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 1 µs | 0 µs | Rust | ×3.1 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 1 µs | 0 µs | Rust | ×2.6 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 1 µs | 0 µs | Rust | ×3.2 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 1 µs | 0 µs | Rust | ×2.8 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 1 µs | 0 µs | Rust | ×2.9 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 2 µs | 1 µs | Rust | ×2.9 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 4 µs | 1 µs | Rust | ×3.8 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 2 µs | 1 µs | Rust | ×2.8 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 2 µs | 1 µs | Rust | ×2.6 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 2 µs | 1 µs | Rust | ×2.7 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 2 µs | 1 µs | Rust | ×3.1 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 2 µs | 1 µs | Rust | ×2.5 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 2 µs | 0 µs | Rust | ×3.3 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 2 µs | 1 µs | Rust | ×4.0 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 1 µs | 0 µs | Rust | ×3.9 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 1 µs | 0 µs | Rust | ×4.0 | - | - |
| retencion | 20123456789-20-R001-123.xml | 2 µs | 0 µs | Rust | ×3.3 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 1 µs | 0 µs | Rust | ×3.8 | - | - |

## xml

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 60 µs | 3 µs | Rust | ×18.1 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 87 µs | 4 µs | Rust | ×23.5 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 14 µs | 1 µs | Rust | ×13.1 | - | - |
| factura | 20123456789-01-F001-123.xml | 85 µs | 3 µs | Rust | ×26.8 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 80 µs | 3 µs | Rust | ×28.5 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 67 µs | 2 µs | Rust | ×27.8 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 72 µs | 3 µs | Rust | ×28.3 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 138 µs | 5 µs | Rust | ×27.0 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 66 µs | 2 µs | Rust | ×29.7 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 78 µs | 3 µs | Rust | ×28.7 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 88 µs | 3 µs | Rust | ×30.2 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 87 µs | 3 µs | Rust | ×28.9 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 76 µs | 3 µs | Rust | ×28.0 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 71 µs | 3 µs | Rust | ×27.5 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 65 µs | 2 µs | Rust | ×29.6 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 62 µs | 2 µs | Rust | ×29.8 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 66 µs | 2 µs | Rust | ×29.0 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 88 µs | 3 µs | Rust | ×28.2 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 89 µs | 3 µs | Rust | ×26.8 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 66 µs | 2 µs | Rust | ×29.1 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 74 µs | 3 µs | Rust | ×26.8 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 67 µs | 3 µs | Rust | ×26.0 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 71 µs | 3 µs | Rust | ×25.5 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 69 µs | 3 µs | Rust | ×26.9 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 71 µs | 2 µs | Rust | ×28.9 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 44 µs | 2 µs | Rust | ×19.6 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 37 µs | 2 µs | Rust | ×22.2 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 37 µs | 2 µs | Rust | ×21.6 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 34 µs | 2 µs | Rust | ×20.6 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 41 µs | 2 µs | Rust | ×22.5 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 101 µs | 4 µs | Rust | ×27.3 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 139 µs | 5 µs | Rust | ×26.7 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 67 µs | 2 µs | Rust | ×29.1 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 68 µs | 2 µs | Rust | ×28.6 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 74 µs | 3 µs | Rust | ×26.4 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 71 µs | 3 µs | Rust | ×25.2 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 79 µs | 3 µs | Rust | ×27.5 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 72 µs | 3 µs | Rust | ×24.7 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 34 µs | 2 µs | Rust | ×18.2 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 68 µs | 3 µs | Rust | ×22.5 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 26 µs | 1 µs | Rust | ×19.0 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 29 µs | 1 µs | Rust | ×19.6 | - | - |
| retencion | 20123456789-20-R001-123.xml | 34 µs | 2 µs | Rust | ×18.2 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 13 µs | 1 µs | Rust | ×14.0 | - | - |

## firma

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 571 µs | 500 µs | Rust | ×1.1 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 618 µs | 427 µs | Rust | ×1.4 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 545 µs | 332 µs | Rust | ×1.6 | - | - |
| factura | 20123456789-01-F001-123.xml | 597 µs | 374 µs | Rust | ×1.6 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 614 µs | 360 µs | Rust | ×1.7 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 587 µs | 349 µs | Rust | ×1.7 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 580 µs | 354 µs | Rust | ×1.6 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 660 µs | 413 µs | Rust | ×1.6 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 576 µs | 350 µs | Rust | ×1.6 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 582 µs | 356 µs | Rust | ×1.6 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 597 µs | 368 µs | Rust | ×1.6 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 597 µs | 368 µs | Rust | ×1.6 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 593 µs | 355 µs | Rust | ×1.7 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 595 µs | 355 µs | Rust | ×1.7 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 577 µs | 353 µs | Rust | ×1.6 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 573 µs | 347 µs | Rust | ×1.7 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 577 µs | 352 µs | Rust | ×1.6 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 606 µs | 371 µs | Rust | ×1.6 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 610 µs | 375 µs | Rust | ×1.6 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 576 µs | 359 µs | Rust | ×1.6 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 590 µs | 359 µs | Rust | ×1.6 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 579 µs | 353 µs | Rust | ×1.6 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 583 µs | 356 µs | Rust | ×1.6 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 578 µs | 351 µs | Rust | ×1.6 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 584 µs | 353 µs | Rust | ×1.7 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 569 µs | 346 µs | Rust | ×1.6 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 561 µs | 341 µs | Rust | ×1.6 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 556 µs | 335 µs | Rust | ×1.7 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 553 µs | 332 µs | Rust | ×1.7 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 560 µs | 338 µs | Rust | ×1.7 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 619 µs | 381 µs | Rust | ×1.6 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 667 µs | 414 µs | Rust | ×1.6 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 582 µs | 350 µs | Rust | ×1.7 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 575 µs | 348 µs | Rust | ×1.6 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 601 µs | 365 µs | Rust | ×1.6 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 596 µs | 364 µs | Rust | ×1.6 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 598 µs | 374 µs | Rust | ×1.6 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 603 µs | 365 µs | Rust | ×1.7 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 554 µs | 333 µs | Rust | ×1.7 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 603 µs | 376 µs | Rust | ×1.6 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 553 µs | 330 µs | Rust | ×1.7 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 552 µs | 333 µs | Rust | ×1.7 | - | - |
| retencion | 20123456789-20-R001-123.xml | 555 µs | 333 µs | Rust | ×1.7 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 541 µs | 321 µs | Rust | ×1.7 | - | - |

## getXmlSigned

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 717 µs | 640 µs | Rust | ×1.1 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 785 µs | 462 µs | Rust | ×1.7 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 585 µs | 335 µs | Rust | ×1.7 | - | - |
| factura | 20123456789-01-F001-123.xml | 788 µs | 373 µs | Rust | ×2.1 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 763 µs | 368 µs | Rust | ×2.1 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 722 µs | 356 µs | Rust | ×2.0 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 731 µs | 357 µs | Rust | ×2.0 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 878 µs | 428 µs | Rust | ×2.1 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 717 µs | 356 µs | Rust | ×2.0 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 735 µs | 358 µs | Rust | ×2.1 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 760 µs | 372 µs | Rust | ×2.0 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 755 µs | 370 µs | Rust | ×2.0 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 734 µs | 361 µs | Rust | ×2.0 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 725 µs | 360 µs | Rust | ×2.0 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 734 µs | 354 µs | Rust | ×2.1 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 706 µs | 352 µs | Rust | ×2.0 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 715 µs | 352 µs | Rust | ×2.0 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 775 µs | 377 µs | Rust | ×2.1 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 775 µs | 378 µs | Rust | ×2.0 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 716 µs | 353 µs | Rust | ×2.0 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 733 µs | 361 µs | Rust | ×2.0 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 722 µs | 355 µs | Rust | ×2.0 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 728 µs | 358 µs | Rust | ×2.0 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 722 µs | 364 µs | Rust | ×2.0 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 727 µs | 358 µs | Rust | ×2.0 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 688 µs | 347 µs | Rust | ×2.0 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 664 µs | 345 µs | Rust | ×1.9 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 663 µs | 338 µs | Rust | ×2.0 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 651 µs | 336 µs | Rust | ×1.9 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 675 µs | 342 µs | Rust | ×2.0 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 798 µs | 397 µs | Rust | ×2.0 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 885 µs | 424 µs | Rust | ×2.1 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 722 µs | 353 µs | Rust | ×2.0 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 730 µs | 362 µs | Rust | ×2.0 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 744 µs | 372 µs | Rust | ×2.0 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 739 µs | 369 µs | Rust | ×2.0 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 748 µs | 369 µs | Rust | ×2.0 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 737 µs | 366 µs | Rust | ×2.0 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 658 µs | 337 µs | Rust | ×2.0 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 753 µs | 381 µs | Rust | ×2.0 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 647 µs | 334 µs | Rust | ×1.9 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 649 µs | 334 µs | Rust | ×1.9 | - | - |
| retencion | 20123456789-20-R001-123.xml | 650 µs | 338 µs | Rust | ×1.9 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 572 µs | 320 µs | Rust | ×1.8 | - | - |

## 1ª llamada

| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| boleta | 20123456789-03-B001-1.xml | 20.1 ms | 4.5 ms | Rust | ×4.5 | - | - |
| boleta-icbper | 20123456789-03-B001-2.xml | 840 µs | 520 µs | Rust | ×1.6 | - | - |
| comunicacion-baja | 20123456789-RA-20261016-00111.xml | 1.6 ms | 352 µs | Rust | ×4.5 | - | - |
| factura | 20123456789-01-F001-123.xml | 856 µs | 386 µs | Rust | ×2.2 | - | - |
| factura-anticipo | 20123456789-01-F001-143.xml | 801 µs | 373 µs | Rust | ×2.1 | - | - |
| factura-beatose | 20123456789-01-F001-128.xml | 825 µs | 360 µs | Rust | ×2.3 | - | - |
| factura-cargos | 20123456789-01-F001-150.xml | 1.6 ms | 369 µs | Rust | ×4.4 | - | - |
| factura-compleja | 20123456789-01-F001-126.xml | 956 µs | 453 µs | Rust | ×2.1 | - | - |
| factura-contingencia | 20123456789-01-0001-1.xml | 741 µs | 501 µs | Rust | ×1.5 | - | - |
| factura-credito | 20123456789-01-F001-132.xml | 764 µs | 364 µs | Rust | ×2.1 | - | - |
| factura-descuento-global | 20123456789-01-F001-84.xml | 775 µs | 378 µs | Rust | ×2.1 | - | - |
| factura-descuento-linea | 20123456789-01-FE01-1.xml | 774 µs | 377 µs | Rust | ×2.1 | - | - |
| factura-descuento-nobase | 20123456789-01-FD01-1.xml | 745 µs | 366 µs | Rust | ×2.0 | - | - |
| factura-detraccion | 20123456789-01-F001-127.xml | 757 µs | 364 µs | Rust | ×2.1 | - | - |
| factura-exonerado | 20123456789-01-F001-124.xml | 797 µs | 357 µs | Rust | ×2.2 | - | - |
| factura-exportacion | 20123456789-01-F001-125.xml | 728 µs | 359 µs | Rust | ×2.0 | - | - |
| factura-gratuita | 20123456789-01-F001-131.xml | 729 µs | 496 µs | Rust | ×1.5 | - | - |
| factura-icbper | 20123456789-01-F001-129.xml | 788 µs | 383 µs | Rust | ×2.1 | - | - |
| factura-icbper-gratuita | 20123456789-01-F001-130.xml | 796 µs | 383 µs | Rust | ×2.1 | - | - |
| factura-inafecta | 20123456789-01-F001-137.xml | 734 µs | 359 µs | Rust | ×2.0 | - | - |
| factura-isc | 20123456789-01-F001-227.xml | 758 µs | 367 µs | Rust | ×2.1 | - | - |
| factura-ivap | 20123456789-01-F001-226.xml | 753 µs | 358 µs | Rust | ×2.1 | - | - |
| factura-percepcion | 20123456789-01-F001-167.xml | 765 µs | 365 µs | Rust | ×2.1 | - | - |
| factura-rel-guia-remision | 20123456789-01-F001-223.xml | 778 µs | 358 µs | Rust | ×2.2 | - | - |
| factura-retencion | 20123456789-01-F001-168.xml | 744 µs | 502 µs | Rust | ×1.5 | - | - |
| guia-extra-atributos | 20161515648-09-T001-124.xml | 5.5 ms | 365 µs | Rust | ×15.2 | - | - |
| guia-misma-empresa | 20161515648-09-T001-120.xml | 682 µs | 343 µs | Rust | ×2.0 | - | - |
| guia-remision | 20161515648-09-T001-121.xml | 680 µs | 345 µs | Rust | ×2.0 | - | - |
| guia-transporteM1L | 20161515648-09-T001-122.xml | 690 µs | 347 µs | Rust | ×2.0 | - | - |
| guia-transportePrivado | 20161515648-09-T001-123.xml | 706 µs | 345 µs | Rust | ×2.0 | - | - |
| inicio-boleta | 20000000001-03-B001-3.xml | 847 µs | 408 µs | Rust | ×2.1 | - | - |
| inicio-factura-credito | 20000000001-01-F001-3.xml | 921 µs | 434 µs | Rust | ×2.1 | - | - |
| inicio-factura-exonerada | 20000000001-01-F001-2.xml | 788 µs | 511 µs | Rust | ×1.5 | - | - |
| inicio-factura-gravada | 20000000001-01-F001-1.xml | 730 µs | 364 µs | Rust | ×2.0 | - | - |
| nota-credito | 20123456789-07-FF01-123.xml | 7.9 ms | 395 µs | Rust | ×20.0 | - | - |
| nota-credito-boleta | 20123456789-07-BB01-123.xml | 767 µs | 372 µs | Rust | ×2.1 | - | - |
| nota-credito-forma-pago | 20123456789-07-FF01-124.xml | 780 µs | 377 µs | Rust | ×2.1 | - | - |
| nota-debito | 20123456789-08-FF01-123.xml | 7.4 ms | 376 µs | Rust | ×19.8 | - | - |
| percepcion | 20123456789-40-P001-123.xml | 2.9 ms | 362 µs | Rust | ×7.9 | - | - |
| resumen | 20123456789-RC-20261016-001.xml | 3.4 ms | 397 µs | Rust | ×8.7 | - | - |
| resumen-contingencia | 20123456789-RC-20261016-002.xml | 672 µs | 484 µs | Rust | ×1.4 | - | - |
| resumen-usd | 20123456789-RC-20261016-211.xml | 664 µs | 337 µs | Rust | ×2.0 | - | - |
| retencion | 20123456789-20-R001-123.xml | 2.8 ms | 349 µs | Rust | ×8.0 | - | - |
| reversion | 20123456789-RR-20261016-001.xml | 601 µs | 330 µs | Rust | ×1.8 | - | - |
