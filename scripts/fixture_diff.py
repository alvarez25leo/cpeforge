#!/usr/bin/env python3
"""Compara fixtures versionados con los regenerados por un oráculo PHP.

Uso: fixture_diff.py <versionado> <regenerado>   (archivos o directorios)

Los JSON se comparan completos salvo los campos de primer nivel que solo
describen el entorno que ejecutó el oráculo (ENV_FIELDS): la versión de PHP
o de OpenSSL/libxml2 cambia entre macOS y el CI sin que cambie ningún
resultado. Es una normalización por campo y motivo
(docs/verificacion-paridad.md, sección 4); los resultados se siguen
comparando exactos. El resto de archivos se compara byte a byte. Sale con 1
y muestra un diff si algo difiere.
"""

from __future__ import annotations

import difflib
import json
import sys
from pathlib import Path

# Metadatos del entorno, no resultados del oráculo.
ENV_FIELDS = ("reference_version", "timezone_db", "openssl", "libxml", "iconv_profile", "libxml_cdata", "libxml_cr", "libxml_xpath")


def normalized(path: Path) -> str:
    if path.suffix != ".json":
        return path.read_bytes().decode("utf-8", "surrogateescape")
    data = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(data, dict):
        for field in ENV_FIELDS:
            if field in data:
                data[field] = "<entorno>"
    return json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def files(root: Path) -> list[Path]:
    return sorted(p.relative_to(root) for p in root.rglob("*") if p.is_file()) if root.is_dir() else [Path()]


def main() -> int:
    committed, generated = Path(sys.argv[1]), Path(sys.argv[2])
    failed = False
    left, right = files(committed), files(generated)
    for missing in sorted(set(left) ^ set(right)):
        print(f"solo en {'versionado' if missing in left else 'regenerado'}: {missing}")
        failed = True
    for rel in sorted(set(left) & set(right)):
        a, b = normalized(committed / rel), normalized(generated / rel)
        if a != b:
            failed = True
            name = str(committed / rel)
            sys.stdout.writelines(difflib.unified_diff(
                a.splitlines(True), b.splitlines(True), name, str(generated / rel), n=2))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
