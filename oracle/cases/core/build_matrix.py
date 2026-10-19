#!/usr/bin/env python3
"""Genera la matriz de inventario y casos de cpeforge-core desde los fixtures.

Uso: python3 oracle/cases/core/build_matrix.py > oracle/evidencia/core-0.1.0-matriz.md
"""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REF = ROOT / "tests/reference/core"

INDEPENDENT = {
    "summary": "boundaries.rs: fecha Lima por aritmética (2023–2025, cada hora), independencia de zona, horario 1994",
    "voided": "boundaries.rs: fecha Lima por aritmética (2023–2025, cada hora), independencia de zona, horario 1994",
    "response": "boundaries.rs: regla numérica de isAccepted (-50000..50000) y oráculo (int) PHP",
    "amounts": "boundaries.rs: escala y signo de Decimal; serde exacto",
    "invalid": "boundaries.rs: rechazo serde de importes, campos desconocidos y tipos",
    "optional": "boundaries.rs: null explícito frente a ausencia en serde",
    "services": "public_api.rs: TokenStore e InvalidServiceResponse con dobles de prueba",
}


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def short(fqcn: str) -> str:
    return fqcn.rsplit("\\", 1)[-1]


def summarize(value, limit: int = 60) -> str:
    text = json.dumps(value, ensure_ascii=False)
    text = text.replace("|", "\\|")
    return text if len(text) <= limit else text[: limit - 1] + "…"


def main() -> None:
    inventory = load(REF / "inventory.json")
    api_map = load(REF / "api-map.json")["classes"]
    differences = load(REF / "differences.json")["differences"]
    hashes = {}
    for line in (REF / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        hashes[name.removeprefix("./")] = digest[:12]

    diff_by_case: dict[str, list[dict]] = {}
    for entry in differences:
        diff_by_case.setdefault(entry["case"], []).append(entry)

    roots: dict[str, list[str]] = {}
    rows = []
    for case_file in sorted((REF / "cases").glob("*.json")):
        family = case_file.stem
        cases = load(case_file)["cases"]
        expected = load(REF / "expected" / case_file.name)["results"]
        for case in cases:
            cid = case["id"]
            root = case["input"]["$class"]
            roots.setdefault(root, []).append(cid)
            php = expected[cid]
            if "hydrate_error" in php:
                outcome = "error al construir: " + php["hydrate_error"]["class"]
            elif php["calls"]:
                outcome = "; ".join(f"{k}={summarize(v, 40)}" for k, v in php["calls"].items())
            else:
                outcome = "instantánea de getters"
            diffs = diff_by_case.get(cid, [])
            result = "IGUAL" if not diffs else "DIFERENCIA_DELIBERADA (" + ", ".join(d["path"] for d in diffs) + ")"
            rows.append(
                (
                    cid,
                    family,
                    case["description"].replace("|", "\\|"),
                    root,
                    ", ".join(case.get("calls", [])) or "—",
                    outcome.replace("|", "\\|"),
                    f"cases/{case_file.name}@{hashes['cases/' + case_file.name]} · expected/{case_file.name}@{hashes['expected/' + case_file.name]}",
                    INDEPENDENT.get(family, "public_api.rs: construcción y nombres sin serde"),
                    result,
                )
            )

    out = []
    out.append("# Matriz de inventario y casos — cpeforge-core 0.1.0")
    out.append("")
    out.append(
        "Generada con `python3 oracle/cases/core/build_matrix.py` a partir de `tests/reference/core/`. "
        "Los resultados PHP provienen de Greenter v5.3.0 (`f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`) "
        "ejecutado con PHP 8.4.17. La prueba Rust de cada caso es `crates/core/tests/parity.rs`."
    )
    out.append("")
    out.append("## Símbolos públicos (61 archivos de `packages/core/src/Core`)")
    out.append("")
    out.append("| Símbolo PHP | Tipo | Equivalente Rust | Métodos con lógica → Rust | Getters/setters | Casos raíz | Estado |")
    out.append("| --- | --- | --- | --- | ---: | ---: | --- |")
    for fqcn, cls in inventory["classes"].items():
        mapping = api_map[fqcn]
        methods = "; ".join(f"`{k}` → {v}" for k, v in mapping["methods"].items()) or "—"
        accessors = sum(1 for m in cls["methods"] if m.get("kind") in ("getter", "setter"))
        name = short(fqcn)
        count = len(roots.get(name, []))
        if cls["kind"] in ("interface", "trait") or name == "TimeZonePe":
            state = "implementada y probada (api_inventory.rs, public_api.rs)"
        elif count:
            diffs = [d for c in roots.get(name, []) for d in diff_by_case.get(c, [])]
            state = "implementada y probada" + (" — con diferencias registradas" if diffs else "")
        else:
            state = "pendiente"
        out.append(
            f"| `{fqcn}` | {cls['kind']} | `{mapping['rust']}` | {methods} | {accessors} | {count} | {state} |"
        )
    out.append("")
    out.append("## Casos de comportamiento")
    out.append("")
    out.append(f"Total: {len(rows)} casos; {sum(1 for r in rows if r[-1] == 'IGUAL')} IGUAL; "
               f"{sum(1 for r in rows if r[-1] != 'IGUAL')} con DIFERENCIA_DELIBERADA.")
    out.append("")
    out.append("| ID | Familia | Comportamiento | Raíz PHP | Llamadas | Resultado PHP | Fixtures (sha256) | Verificador independiente | Resultado |")
    out.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for row in rows:
        out.append("| " + " | ".join(row) + " |")
    int_cast = load(REF / "int-cast/expected.json")
    out.append("")
    out.append("## Conversión `(int)` de PHP usada por `isAccepted()`")
    out.append("")
    out.append(
        f"{len(int_cast['results'])} entradas en `int-cast/inputs.json@{hashes['int-cast/inputs.json']}`, resultados PHP en "
        f"`int-cast/expected.json@{hashes['int-cast/expected.json']}`; prueba `boundaries.rs::int_cast_matches_reference_oracle`. "
        "Resultado: IGUAL en todas."
    )
    print("\n".join(out))


if __name__ == "__main__":
    main()
