#!/usr/bin/env python3
"""Genera crates/validator/src/setters.json desde el inventario PHP de core.

tests/reference/core/inventory.json se obtiene por reflexión de Greenter
v5.3.0 (oracle/php/core). Para cada clase de `Greenter\\Model` se resuelven,
recorriendo `parent`, los setters heredados: el TypeError de PHP nombra la
clase que DECLARA el método (`PaymentTerms::setTipo` aunque el objeto sea un
`FormaPagoContado`). También se guardan los ancestros e interfaces, porque un
parámetro de tipo clase acepta subclases.

Uso: build_validator_setters.py [--output RUTA]
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INVENTORY = ROOT / "tests/reference/core/inventory.json"
DEFAULT_OUTPUT = ROOT / "crates/validator/src/setters.json"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    classes = json.loads(INVENTORY.read_text(encoding="utf-8"))["classes"]
    models = {fq: info for fq, info in classes.items() if fq.startswith("CPEForge\\Model\\")}

    def chain(fq: str) -> list[str]:
        out = []
        while fq in classes:
            out.append(fq)
            fq = classes[fq].get("parent") or ""
        return out

    out: dict[str, dict] = {}
    for fq, info in sorted(models.items()):
        if info.get("kind") != "class":
            continue
        short = fq.rsplit("\\", 1)[-1]
        if short in out:
            raise SystemExit(f"nombre corto repetido: {short}")
        setters: dict[str, list] = {}
        # Del ancestro más lejano al más cercano: la subclase redefine.
        for owner in reversed(chain(fq)):
            for method in classes[owner].get("methods", []):
                params = method.get("params") or []
                if method.get("kind") == "setter" and len(params) == 1:
                    setters[method["name"]] = [owner, params[0]["name"], params[0]["type"]]
        ancestors = chain(fq)
        interfaces = sorted({i for c in ancestors for i in classes[c].get("interfaces", [])})
        out[short] = {"class": fq, "is_a": ancestors + interfaces, "setters": dict(sorted(setters.items()))}

    text = json.dumps(
        {
            "source": "tests/reference/core/inventory.json (reflexión de la referencia)",
            "generator": "scripts/build_validator_setters.py",
            "models": out,
        },
        ensure_ascii=False,
        indent=1,
        sort_keys=True,
    ) + "\n"
    args.output.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
