#!/usr/bin/env python3
"""Matriz campo a campo de greenter/xml-parser v5.3.0 para la evidencia.

Para cada llamada `->setX(expresión)` de los parsers PHP fijados extrae la
expresión literal (ruta XPath, contexto, valor por defecto y conversión) y
cuenta en cuántos casos del oráculo la propiedad tiene valor PHP no nulo.

Uso: python3 -I oracle/cases/xml_parser/matrix.py <src> <expected.json> <salida.md>
"""

from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path

SETTER = re.compile(r"->set(\w+)\((.*)$")


def args_of(rest: str) -> str:
    depth, out = 1, []
    for ch in rest:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                break
        out.append(ch)
    return "".join(out).strip()


def coverage(expected: dict) -> Counter:
    counts: Counter = Counter()

    def walk(value):
        if isinstance(value, dict):
            if "$class" in value:
                for key, item in value.items():
                    if key != "$class" and item is not None:
                        counts[(value["$class"], key)] += 1
            for item in value.values():
                walk(item)
        elif isinstance(value, list):
            for item in value:
                walk(item)

    for result in expected["results"].values():
        for call in result.get("calls", [result]):
            walk(call.get("snapshot"))
    return counts


def conversion(expr: str) -> str:
    kinds = []
    if "(float)" in expr:
        kinds.append("`(float)` → Decimal")
    if "(int)" in expr:
        kinds.append("`(int)`")
    if "new DateTime" in expr:
        kinds.append("`new DateTime`")
    if "trim(" in expr:
        kinds.append("`trim`")
    if "== 'true'" in expr:
        kinds.append("`== 'true'`")
    return ", ".join(kinds) or "texto"


def main() -> int:
    src, expected_path, out_path = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    counts = coverage(json.loads(expected_path.read_text()))
    by_key = Counter()
    for (_, key), n in counts.items():
        by_key[key] += n
    lines = [
        "# Matriz de campos — cpeforge-xml-parser 0.1.0",
        "",
        "Generada con `python3 -I oracle/cases/xml_parser/matrix.py` desde las fuentes PHP fijadas",
        "(`f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`) y `tests/reference/xml-parser/expected.json`.",
        "Cada fila es una llamada a un setter en el orden del código: la expresión PHP literal",
        "(ruta XPath, nodo de contexto, valor por defecto y conversión) y el número de",
        "objetos del oráculo en que una propiedad con ese nombre tiene valor PHP no nulo (todas las",
        "clases y casos, por eso `tipoDoc` o `ruc` suman varias familias). La prueba",
        "`crates/xml-parser/tests/parity.rs` compara el modelo completo campo a campo y falla si una",
        "propiedad asignada por un setter no se compara nunca con un valor no nulo.",
    ]
    total = 0
    for path in sorted(src.rglob("*Parser.php")):
        rows = []
        for number, line in enumerate(path.read_text().splitlines(), 1):
            for match in SETTER.finditer(line):
                prop = match.group(1)[0].lower() + match.group(1)[1:]
                expr = args_of(match.group(2)) or "(en la línea siguiente)"
                rows.append((number, prop, expr))
        total += len(rows)
        lines += ["", f"## {path.stem}", "", "| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |", "| --- | --- | --- | --- | --- |"]
        for number, prop, expr in rows:
            cell = expr.replace("|", "\\|")
            lines.append(f"| {number} | `{prop}` | `{cell}` | {conversion(expr)} | {by_key.get(prop, 0)} |")
    lines += ["", f"Total de llamadas a setters: {total}.", ""]
    out_path.write_text("\n".join(lines))
    print(total, "filas")
    return 0


if __name__ == "__main__":
    sys.exit(main())
