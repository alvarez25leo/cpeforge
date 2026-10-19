#!/usr/bin/env python3
"""Adaptaciones de las plantillas Twig de greenter/report v5.3.0 a MiniJinja.

Las plantillas de crates/report/templates son las de la referencia con estas
sustituciones exactas y nada más. Cada una conserva la semántica de PHP donde
MiniJinja difiere (ver crates/report/README.md):

- `in` y `!=` de Twig usan la comparación laxa de PHP 8 ("1" == "01");
- una cadena "0" es falsa en PHP;
- `null ~ 'x'` concatena una cadena vacía.

Uso:
  report_template_adaptations.py check <plantillas PHP> <plantillas Rust>
  report_template_adaptations.py apply <plantillas PHP> <plantillas Rust>
"""

from __future__ import annotations

import sys
from pathlib import Path

HASH = ("{% if params.system.hash is defined and params.system.hash%}",
        "{% if params.system.hash is defined and params.system.hash is twig_true %}")
HASH_SP = ("{% if params.system.hash is defined and params.system.hash %}",
           "{% if params.system.hash is defined and params.system.hash is twig_true %}")
OBS = ("{% if doc.observacion %}", "{% if doc.observacion is twig_true %}")

ADAPTATIONS: dict[str, list[tuple[str, str, int]]] = {
    "invoice.html.twig": [
        ("{% set isInvoice = doc.tipoDoc in ['01', '03'] %}",
         "{% set isInvoice = doc.tipoDoc is twig_in(['01', '03']) %}", 1),
        ("{% set isNota = doc.tipoDoc in ['07', '08'] %}",
         "{% set isNota = doc.tipoDoc is twig_in(['07', '08']) %}", 1),
        ("{% if doc.compra is defined and doc.compra %}",
         "{% if doc.compra is defined and doc.compra is twig_true %}", 1),
        ("{% if leg.code != '1000' %}", "{% if leg.code is not twig_eq('1000') %}", 1),
        (*HASH, 1),
    ],
    "invoice2.html.twig": [
        ("{% set serieNro = doc.serie ~ '-' ~ doc.correlativo %}",
         "{% set serieNro = doc.serie|twig_string ~ '-' ~ doc.correlativo|twig_string %}", 1),
        (*HASH, 1),
    ],
    "despatch.html.twig": [(*OBS, 1), (*HASH, 1)],
    "perception.html.twig": [(*OBS, 1), (*HASH, 1)],
    "retention.html.twig": [(*OBS, 1), (*HASH, 1)],
    "summary.html.twig": [(*HASH, 1)],
    "voided.html.twig": [
        ("{% if 'RA' in doc.name %}", "{% if 'RA' is twig_in(doc.name) %}", 1),
        (*HASH_SP, 1),
    ],
    "assets/style.css": [],
}


def adapt(name: str, text: str) -> str:
    for old, new, count in ADAPTATIONS[name]:
        found = text.count(old)
        if found != count:
            raise SystemExit(f"{name}: se esperaban {count} apariciones de {old!r}, hay {found}")
        text = text.replace(old, new)
    return text


def main() -> int:
    if len(sys.argv) != 4 or sys.argv[1] not in ("check", "apply"):
        print(__doc__, file=sys.stderr)
        return 2
    mode, php_dir, rust_dir = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
    php_files = sorted(str(p.relative_to(php_dir)) for p in php_dir.rglob("*") if p.is_file())
    rust_files = sorted(str(p.relative_to(rust_dir)) for p in rust_dir.rglob("*")
                        if p.is_file() and p.name != "README.md")
    if php_files != sorted(ADAPTATIONS) or rust_files != sorted(ADAPTATIONS):
        print(f"conjunto de plantillas distinto: PHP {php_files}, Rust {rust_files}", file=sys.stderr)
        return 1
    status = 0
    for name in ADAPTATIONS:
        expected = adapt(name, (php_dir / name).read_bytes().decode("utf-8"))
        target = rust_dir / name
        if mode == "apply":
            target.write_bytes(expected.encode("utf-8"))
        elif target.read_bytes().decode("utf-8") != expected:
            print(f"{name}: no coincide con la plantilla PHP + adaptaciones", file=sys.stderr)
            status = 1
    if status == 0:
        print(f"plantillas: {len(ADAPTATIONS)} archivos = referencia PHP + "
              f"{sum(len(v) for v in ADAPTATIONS.values())} adaptaciones")
    return status


if __name__ == "__main__":
    sys.exit(main())
