#!/usr/bin/env python3
"""Compara los tiempos de los ejemplos PHP y Rust (out/benchmark.json).

Uso: benchmark.py [php.json] [rust.json] [--md salida.md]

Muestra en la terminal, para cada caso y cada fase, qué lenguaje fue más
rápido y cuántas veces, y los totales por fase. Con --md (por defecto
example/benchmark.md) escribe además todas las cifras.

Fases:
- construir: crear el modelo (mediana de N llamadas);
- xml: el builder, sin firmar (mediana);
- firma: SignedXml sobre el XML ya construido (mediana);
- getXmlSigned: See::getXmlSigned completo, como se usa (mediana);
- 1ª llamada: la primera llamada a getXmlSigned del caso (incluye
  preparaciones que no se repiten, como compilar plantillas Twig en PHP);
- envío y estado: una sola medición de red (send y las consultas de ticket,
  sin las pausas). Depende de la latencia del servicio de prueba en ese
  momento: orientativo.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
EJEMPLOS = AQUI.parent.parent / "example"
FASES = [
    ("construir_us", "construir", "us"),
    ("xml_us", "xml", "us"),
    ("firma_us", "firma", "us"),
    ("see_us", "getXmlSigned", "us"),
    ("see_primera_us", "1ª llamada", "us"),
    ("envio_ms", "envío", "ms"),
    ("estado_ms", "estado", "ms"),
]


def legible(valor: float | None, unidad: str) -> str:
    if valor is None:
        return "-"
    us = valor * 1000 if unidad == "ms" else valor
    if us < 1000:
        return f"{us:.0f} µs"
    if us < 1_000_000:
        return f"{us / 1000:.1f} ms"
    return f"{us / 1_000_000:.2f} s"


def comparar(php: float | None, rust: float | None) -> tuple[str, str]:
    """(ganador, «×factor») o ("-", "-") si falta una medida."""
    if php is None or rust is None or php <= 0 or rust <= 0:
        return "-", "-"
    if rust < php:
        return "Rust", f"×{php / rust:.1f}"
    if php < rust:
        return "PHP", f"×{rust / php:.1f}"
    return "empate", "×1.0"


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    md = EJEMPLOS / "benchmark.md"
    if "--md" in sys.argv:
        md = Path(sys.argv[sys.argv.index("--md") + 1])
        args = [a for a in args if a != str(md)]
    php_path = Path(args[0]) if args else EJEMPLOS / "php/out/benchmark.json"
    rust_path = Path(args[1]) if len(args) > 1 else EJEMPLOS / "rust/out/benchmark.json"
    php = json.loads(php_path.read_text(encoding="utf-8"))
    rust = json.loads(rust_path.read_text(encoding="utf-8"))
    pc = {c["caso"]: c for c in php["casos"]}
    rc = {c["caso"]: c for c in rust["casos"]}
    casos = [c for c in pc if c in rc]
    if sorted(pc) != sorted(rc):
        print(f"aviso: casos solo en PHP {sorted(set(pc) - set(rc))}, solo en Rust {sorted(set(rc) - set(pc))}")
    fases = [f for f in FASES if any(pc[c][f[0]] is not None and rc[c][f[0]] is not None for c in casos)]

    titulo = (f"Benchmark: PHP {php['version']} (opcache {'sí' if php['opcache'] else 'no'}, JIT {php['jit'] or 'no'})"
              f" frente a Rust ({rust['perfil']}); mediana de {php['repeticiones']}/{rust['repeticiones']} llamadas")
    print(titulo)
    if rust["perfil"] != "release":
        print("aviso: Rust compilado en debug; para medir use cargo run --release")

    # Terminal: ganador y factor por caso y fase.
    ancho = max(len(c) for c in casos + ["TOTAL"]) + 2
    print("".ljust(ancho) + "".join(nombre.ljust(15) for _, nombre, _ in fases))
    for caso in casos:
        celdas = []
        for clave, _, _ in fases:
            ganador, factor = comparar(pc[caso][clave], rc[caso][clave])
            celdas.append((f"{ganador} {factor}" if ganador not in ("-", "empate") else ganador).ljust(15))
        print(caso.ljust(ancho) + "".join(celdas))

    # Totales por fase.
    print()
    print("Totales por fase (suma de los casos):")
    resumen = []
    for clave, nombre, unidad in fases:
        pares = [(pc[c][clave], rc[c][clave]) for c in casos if pc[c][clave] is not None and rc[c][clave] is not None]
        tp, tr = sum(p for p, _ in pares), sum(r for _, r in pares)
        gana_rust = sum(1 for p, r in pares if r < p)
        gana_php = sum(1 for p, r in pares if p < r)
        ganador, factor = comparar(tp, tr)
        linea = (f"  {nombre:14} PHP {legible(tp, unidad):>10}   Rust {legible(tr, unidad):>10}   "
                 f"más rápido: {ganador} {factor}   (casos: Rust {gana_rust}, PHP {gana_php})")
        print(linea)
        resumen.append((nombre, legible(tp, unidad), legible(tr, unidad), ganador, factor, gana_rust, gana_php))

    # Markdown con todas las cifras.
    lineas = [f"# {titulo}", "",
              "Generado por `oracle/ejemplos/benchmark.py` a partir de `php/out/benchmark.json` y "
              "`rust/out/benchmark.json`. Las fases de red son una sola medición y dependen de la "
              "latencia del servicio de prueba.", "", "## Totales", "",
              "| Fase | PHP | Rust | Más rápido | Factor | Casos que gana Rust | Casos que gana PHP |",
              "| --- | ---: | ---: | --- | ---: | ---: | ---: |"]
    lineas += [f"| {n} | {p} | {r} | {g} | {f} | {gr} | {gp} |" for n, p, r, g, f, gr, gp in resumen]
    for clave, nombre, unidad in fases:
        lineas += ["", f"## {nombre}", "", "| Caso | Archivo | PHP | Rust | Más rápido | Factor | Resultado PHP | Resultado Rust |",
                   "| --- | --- | ---: | ---: | --- | ---: | --- | --- |"]
        for caso in casos:
            p, r = pc[caso][clave], rc[caso][clave]
            ganador, factor = comparar(p, r)
            lineas.append(f"| {caso} | {pc[caso]['archivo']} | {legible(p, unidad)} | {legible(r, unidad)} | {ganador} | "
                          f"{factor} | {pc[caso]['resultado'] or '-'} | {rc[caso]['resultado'] or '-'} |")
    md.write_text("\n".join(lineas) + "\n", encoding="utf-8")
    print(f"\nDetalle completo: {md}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
