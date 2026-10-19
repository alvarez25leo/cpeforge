#!/usr/bin/env python3
"""Inventario de greenter/cpe-validator v5.3.0 (14 archivos fuente).

Extrae, por archivo: SHA-256, líneas, tipo (clase/interfaz), métodos
públicos, constantes y literales relevantes (códigos de error, mensajes de
excepción, parámetros XSLT y funciones registradas). Además: el mapa de
rutas de XslPathResolver, los tests y recursos upstream (solo hashes, sin
copiarlos) y la licencia. La prueba Rust exige 14 archivos y el mismo mapa.

Uso: python3 -I oracle/cases/cpe_validator/inventory.py <paquete> <salida.json>
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

KIND = re.compile(r"^(?:final\s+)?(class|interface)\s+(\w+)", re.M)
METHOD = re.compile(r"(public|private|protected)\s+(static\s+)?function\s+(\w+)\s*\(([^)]*)\)\s*(?::\s*([?\w\\\[\]]+))?")
CONST = re.compile(r"public\s+const\s+(\w+)\s*=\s*'([^']*)'")
STRING = re.compile(r"'([^'\\]*(?:\\.[^'\\]*)*)'")
MAPPING = re.compile(r"DocumentType::(\w+)\s*=>\s*\"([^\"]+)\"")


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    package = Path(sys.argv[1])
    src = package / "src"
    files = sorted(src.rglob("*.php"))
    out: dict = {
        "source": "thegreenter/greenter f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f packages/cpe-validator",
        "files": [],
    }
    constants: dict[str, str] = {}
    for path in files:
        code = path.read_text(encoding="utf-8")
        kind = KIND.search(code)
        consts = {m.group(1): m.group(2) for m in CONST.finditer(code)}
        if path.name == "DocumentType.php":
            constants = consts
        out["files"].append({
            "file": str(path.relative_to(src)),
            "sha256": sha(path),
            "lines": code.count("\n") + 1,
            "kind": kind.group(1) if kind else None,
            "name": kind.group(2) if kind else None,
            "methods": [
                {"visibility": m.group(1), "static": bool(m.group(2)), "name": m.group(3),
                 "params": " ".join(m.group(4).split()), "returns": m.group(5)}
                for m in METHOD.finditer(code)
            ],
            "constants": consts,
            "literals": sorted({s for s in STRING.findall(code) if s not in consts.values()}),
        })
    resolver = (src / "Validator/Resolver/XslPathResolver.php").read_text(encoding="utf-8")
    out["path_mapping"] = {constants[name]: rel for name, rel in MAPPING.findall(resolver)}
    out["upstream_tests"] = [
        {"file": str(p.relative_to(package)), "sha256": sha(p), "bytes": p.stat().st_size}
        for p in sorted((package / "tests").rglob("*")) if p.is_file()
    ]
    out["license"] = {"file": "LICENSE", "sha256": sha(package / "LICENSE"),
                      "first_line": (package / "LICENSE").read_text(encoding="utf-8").splitlines()[0]}
    composer = json.loads((package / "composer.json").read_text(encoding="utf-8"))
    out["composer"] = {"license": composer.get("license"), "require": composer.get("require")}
    Path(sys.argv[2]).write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(len(files), "archivos;", len(out["path_mapping"]), "rutas;", len(out["upstream_tests"]), "archivos de prueba upstream")
    return 0 if len(files) == 14 and len(out["path_mapping"]) == 10 else 1


if __name__ == "__main__":
    sys.exit(main())
