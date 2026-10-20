# Ejemplos de rechazos SUNAT en entornos de prueba

Estos casos parten de los ejemplos de éxito de `example/php/casos/` y
`example/rust/src/casos/`. Cambian el mismo dato en PHP y Rust, generan y
firman el XML y comprueban que ambos envían el mismo documento. Viven aparte
para que `oracle/ejemplos/comparar.sh` y `oracle/ejemplos/enviar.sh` mantengan sus 44 casos de
éxito.

| Caso | Cambio deliberado | Código observado en prueba |
| --- | --- | --- |
| `guia-sin-fecha-entrega` | GRE de transporte público sin fecha de entrega de bienes | `3617` |
| `nota-motivo13-con-total` | Nota de crédito con motivo 13, datos de pago al crédito e importe total 236.00 en vez de cero | `3315` |
| `resumen-sin-tasa` | Resumen RC sin `porcentajeIgv` en sus líneas | `2992` |

El alcance y el código esperado están en [`casos.json`](casos.json). Los
constructores PHP están en `example/php/errores/`; los Rust, en
`example/rust/src/errores.rs`.

## Generar y comparar sin red

```bash
EJEMPLO_REPETICIONES=1 bash oracle/ejemplos/errores/comparar.sh
```

También se puede elegir un caso:

```bash
bash oracle/ejemplos/errores/comparar.sh resumen-sin-tasa
```

Usa una fecha fija y compara el XML sin la firma ni la identidad (`normalizar.py`).
Comprueba el XSD de PHP y Rust cuando está instalado el directorio de esquemas
del oráculo. El Rust verifica además la firma digital. Ninguno de estos
comandos llama a SUNAT.

## Enviar a los servicios de prueba y comparar las respuestas

```bash
EJEMPLO_REPETICIONES=1 bash oracle/ejemplos/errores/enviar.sh
```

El script usa `FE_BETA` para la nota y el resumen, y `gre.api_url` de prueba
para la guía; no usa producción. Genera los tres casos con la misma fecha y,
en cada lenguaje, los envía a su servicio de prueba. Exige el código esperado,
el mismo tipo de rechazo, mensaje y observaciones. Se espera `EJEMPLO_PAUSA`
segundos entre envíos (5 por defecto) por el límite de BETA. Se puede pasar un
nombre para enviar solo ese caso. Para GRE se usa la sección `gre` de
`example/credenciales/credenciales-prueba.json`.

La respuesta SOAP de BETA incrusta un **ticket distinto por solicitud** dentro
del mensaje. `verificar.py` conserva los mensajes originales en
`example/{php,rust}/out/errores/benchmark.json` y sustituye únicamente los
dígitos de `ticket: <número> error:` durante la comparación. Cualquier otra
diferencia de texto, código u observaciones hace fallar el comando. El ticket
no se considera parte del mensaje estable que puede igualarse entre dos
envíos distintos.

Resultado observado el 18/10/2026: los tres XML firmados coincidieron byte a
byte y pasaron XSD; PHP y Rust recibieron `3617` en GRE y `3315` y `2992` en
BETA. El mensaje GRE coincidió exactamente; los dos mensajes SOAP coincidieron
salvo el ticket. Una respuesta futura puede cambiar por reglas o disponibilidad
del servicio; el script fallará y mostrará la discrepancia.
