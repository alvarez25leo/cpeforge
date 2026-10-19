#!/usr/bin/env python3
"""Compara PHP y Rust sin excepciones ni normalización de violaciones."""

import json
import sys
from pathlib import Path

if len(sys.argv) != 4:
    raise SystemExit("Uso: compare_validator.py cases.json php.json rust.json")
cases, php, rust = (json.loads(Path(path).read_text()) for path in sys.argv[1:])
case_ids = [case["id"] for case in cases]
php_ids = [row["id"] for row in php]
rust_ids = [row["id"] for row in rust]
if len(set(case_ids)) != len(case_ids) or case_ids != php_ids or case_ids != rust_ids:
    raise SystemExit("Faltan casos, sobran casos o cambió su orden en uno de los runners")


def first_difference(left, right, path="$ "):
    if type(left) is not type(right):
        return path.strip()
    if isinstance(left, dict):
        if left.keys() != right.keys():
            return path.strip() + ".keys"
        for key in left:
            difference = first_difference(left[key], right[key], f"{path.strip()}.{key}")
            if difference:
                return difference
    elif isinstance(left, list):
        if len(left) != len(right):
            return path.strip() + ".length"
        for index, (a, b) in enumerate(zip(left, right)):
            difference = first_difference(a, b, f"{path.strip()}[{index}]")
            if difference:
                return difference
    elif left != right:
        return path.strip()
    return None


differences = []
for expected, actual in zip(php, rust):
    difference = first_difference(expected, actual)
    if difference:
        differences.append({
            "id": expected["id"],
            "path": difference,
            "php": expected.get("error", {}).get("message") if "error" in expected else expected,
            "rust": actual.get("error", {}).get("message") if "error" in actual else actual,
        })

report = Path("target/evidence/validator-differences.json")
report.parent.mkdir(parents=True, exist_ok=True)
report.write_text(json.dumps(differences, ensure_ascii=False, indent=2) + "\n")
registry = json.loads(Path("tests/reference/validator/differences.json").read_text())["differences"]
observed = [(row["id"], row["path"]) for row in differences]
registered = [(row["id"], row["path"]) for row in registry]
if registered != observed:
    raise SystemExit("El registro de diferencias no coincide con los casos actuales (faltante, nuevo u obsoleto)")
# Perfil seguro (docs/verificacion-paridad.md): una diferencia deliberada
# necesita motivo admitido y nota; una CORREGIR sigue bloqueando.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from release_readiness import MOTIVOS  # noqa: E402

invalid = [row["id"] for row in registry
           if not (row.get("classification") == "DIFERENCIA_DELIBERADA" and row.get("motivo") in MOTIVOS
                   and row.get("reason") and row.get("compatibility_note"))
           and not (row.get("classification") == "CORREGIR" and row.get("reason") and row.get("next_step"))]
if invalid:
    raise SystemExit(f"Registros inválidos según el perfil seguro: {invalid[:5]}")
pending = [row for row, diff in zip(registry, differences) if row["classification"] == "CORREGIR"]
if pending:
    first = next(diff for row, diff in zip(registry, differences) if row["classification"] == "CORREGIR")
    print(f"DIFERENCIAS CORREGIR {len(pending)}/{len(cases)}; primera: {first['id']} en {first['path']}")
    print(f"PHP: {first['php']}")
    print(f"Rust: {first['rust']}")
    raise SystemExit(1)
deliberate = len(differences)
print(f"IGUAL {len(cases) - deliberate}/{len(cases)} casos; {deliberate} diferencias del perfil seguro registradas")
