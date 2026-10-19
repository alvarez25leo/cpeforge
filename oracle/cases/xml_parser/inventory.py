#!/usr/bin/env python3
"""Inventario de los 10 archivos fuente de greenter/xml-parser v5.3.0.

Extrae, por archivo: SHA-256, líneas, métodos, rutas XPath consultadas (con
su contexto y valor por defecto cuando es literal), setters llamados,
conversiones `(float)`/`(int)`, `new DateTime`, `trim`, `explode` y las
comparaciones de `switch`/`==`. La prueba de paridad exige que cada propiedad
asignada por un setter se compare con un valor PHP no nulo en algún caso.

Uso: python3 -I oracle/cases/xml_parser/inventory.py <src> <salida.json>
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

QUERY = re.compile(r"(getValue|getNode|getNodes|query)\(\s*'([^']+)'(?:\s*,\s*([^,)]+))?(?:\s*,\s*'([^']*)')?")
SETTER = re.compile(r"->set(\w+)\(")
METHOD = re.compile(r"(public|private|protected)\s+function\s+(\w+)")
CASE = re.compile(r"case\s+'([^']*)'")


def main() -> int:
    src = Path(sys.argv[1])
    files = sorted(src.rglob("*.php"))
    out = {"source": "thegreenter/greenter f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f packages/xml-parser/src", "files": []}
    for path in files:
        code = path.read_text()
        rel = str(path.relative_to(src))
        setters = sorted({m.group(1)[0].lower() + m.group(1)[1:] for m in SETTER.finditer(code)})
        out["files"].append({
            "file": rel,
            "sha256": hashlib.sha256(code.encode()).hexdigest(),
            "lines": code.count("\n") + 1,
            "methods": [m.group(2) for m in METHOD.finditer(code)],
            "queries": [
                {"call": m.group(1), "xpath": m.group(2), "context": (m.group(3) or "").strip() or None, "default": m.group(4)}
                for m in QUERY.finditer(code)
            ],
            "setters": setters,
            "switch_cases": sorted({m.group(1) for m in CASE.finditer(code)}),
            "float_casts": code.count("(float)"),
            "int_casts": code.count("(int)"),
            "datetime": code.count("new DateTime("),
            "trim": code.count("trim("),
            "explode": code.count("explode("),
        })
    Path(sys.argv[2]).write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n")
    print(len(files), "archivos;", sum(len(f["setters"]) for f in out["files"]), "setters;", sum(len(f["queries"]) for f in out["files"]), "consultas")
    return 0


if __name__ == "__main__":
    sys.exit(main())
