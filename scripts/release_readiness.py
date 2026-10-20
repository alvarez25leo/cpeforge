#!/usr/bin/env python3
"""Resume, por paquete, lo que impide declarar LISTO según el perfil seguro.

Lee solo archivos del repositorio: los registros
tests/reference/<paquete>/differences.json y los manifiestos Cargo.toml. No
ejecuta pruebas: los gates siguen siendo los scripts de paridad y el CI. Sale
con código 1 si algún paquete tiene bloqueos detectables aquí.

Con `--ci` solo falla por inconsistencias (registro inválido, manifiesto sin
licencia o repositorio, oracle/compatibilidad.md desactualizado); los
pendientes `CORREGIR` de un paquete en desarrollo se informan sin fallar.

También genera oracle/compatibilidad.md desde los registros. Con
`--write-compat` lo reescribe; sin la opción falla si está desactualizado.
"""

from __future__ import annotations

import json
import sys
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACKAGES = {
    "core": "core",
    "xml": "xml",
    "xmldsig": "xmldsig",
    "gre-api": "gre-api",
    "ws": "ws",
    "cpeforge": "cpeforge",
    "validator": "validator",
    "xcodes": "xcodes",
    "xml-parser": "xml-parser",
    "cpe-validator": "cpe-validator",
    "ubl-validator": "ubl-validator",
    "report": "report",
}
MOTIVOS = {
    "seguridad",
    "conformidad-xsd",
    "precision-decimal",
    "rechazo-explicito",
    "tipado-estatico",
    "diagnostico-runtime",
    "rango-de-tipo",
    "compresion-equivalente",
    "codificacion-fuera-de-alcance",
    "dependiente-de-iconv",
    "dependiente-de-libxml",
}


def manifest_fields(path: Path) -> dict[str, str]:
    """`license` y `repository` de `[package]` o `[workspace.package]`.

    Devuelve "workspace" si el campo se hereda (`campo.workspace = true`).
    Basta un análisis por líneas: los manifiestos del repositorio los
    escribimos nosotros (Python 3.9 sin `tomllib`).
    """
    fields: dict[str, str] = {}
    section = ""
    for line in path.read_text(encoding="utf-8").splitlines():
        header = re.match(r"\s*\[([^\]]+)\]", line)
        if header:
            section = header.group(1).strip()
            continue
        if section not in ("package", "workspace.package"):
            continue
        match = re.match(r"\s*(license|repository)(\.workspace)?\s*=\s*(.+?)\s*$", line)
        if match:
            fields[match.group(1)] = "workspace" if match.group(2) else match.group(3).strip('"')
    return fields


def entries(registry: dict) -> list[dict]:
    out = []
    for key in ("differences", "filter_differences", "not_applicable"):
        out.extend(registry.get(key) or [])
    return out


def label(entry: dict) -> str:
    for key in ("id", "case"):
        if entry.get(key):
            family = entry.get("family")
            path = entry.get("path")
            base = f"{family}/{entry[key]}" if family else entry[key]
            return f"{base} ({path})" if path else base
    return json.dumps(entry.get("input"), ensure_ascii=False)


MOTIVO_TEXT = {
    "seguridad": "Seguridad",
    "conformidad-xsd": "Conformidad con el esquema XSD",
    "precision-decimal": "Precisión decimal exacta",
    "rechazo-explicito": "Rechazo explícito de entradas inválidas",
    "tipado-estatico": "Tipado estático",
    "diagnostico-runtime": "Diagnóstico del runtime",
    "rango-de-tipo": "Rango de tipo",
    "compresion-equivalente": "Compresión equivalente",
    "codificacion-fuera-de-alcance": "Codificación fuera de alcance",
    "dependiente-de-iconv": "Depende del iconv de la plataforma",
    "dependiente-de-libxml": "Depende de la versión de libxml2",
}


def compat_markdown() -> str:
    lines = [
        "# Compatibilidad con Greenter PHP (perfil seguro)",
        "",
        "Archivo generado por `python3 scripts/release_readiness.py --write-compat` desde",
        "`tests/reference/*/differences.json`; no editar a mano.",
        "",
        "Los crates reproducen el comportamiento de Greenter v5.3.0 salvo en las diferencias",
        "deliberadas de esta lista, admitidas por el",
        "perfil seguro. Las entradas `CORREGIR` son",
        "pendientes: el paquete correspondiente sigue **NO LISTO** mientras existan.",
    ]
    for package in PACKAGES:
        registry_path = ROOT / "tests/reference" / package / "differences.json"
        if not registry_path.exists():
            continue
        registry = json.loads(registry_path.read_text(encoding="utf-8"))
        lines += ["", f"## cpeforge-{package}", ""]
        lines += ["| Caso | Clasificación | Motivo | Qué cambia | Nota de compatibilidad |", "| --- | --- | --- | --- | --- |"]
        for entry in entries(registry):
            cell = lambda value: str(value or "").replace("|", "\\|").replace("\n", " ")
            motivo = MOTIVO_TEXT.get(entry.get("motivo", ""), "—")
            note = entry.get("compatibility_note") or f"Pendiente: {entry.get('next_step', '')}"
            lines.append(
                f"| `{cell(label(entry))}` | {entry.get('classification')} | {motivo} | {cell(entry.get('reason'))} | {cell(note)} |"
            )
    return "\n".join(lines) + "\n"


def main() -> int:
    compat_path = ROOT / "oracle/compatibilidad.md"
    generated = compat_markdown()
    if "--write-compat" in sys.argv[1:]:
        compat_path.write_text(generated, encoding="utf-8")
    compat_outdated = not compat_path.exists() or compat_path.read_text(encoding="utf-8") != generated
    workspace = manifest_fields(ROOT / "Cargo.toml")
    blocked = False
    inconsistent = False
    for package, crate_dir in PACKAGES.items():
        problems: list[str] = []
        registry_path = ROOT / "tests/reference" / package / "differences.json"
        registry = json.loads(registry_path.read_text(encoding="utf-8")) if registry_path.exists() else {}
        motives: Counter[str] = Counter()
        for entry in entries(registry):
            kind = entry.get("classification")
            if kind == "DIFERENCIA_DELIBERADA" and entry.get("motivo") in MOTIVOS:
                motives[entry["motivo"]] += 1
            elif kind == "CORREGIR":
                problems.append(f"CORREGIR {label(entry)}: {entry.get('next_step', '')}")
            else:
                problems.append(f"registro inválido {label(entry)}")
                inconsistent = True
        manifest = manifest_fields(ROOT / "crates" / crate_dir / "Cargo.toml")
        for field in ("license", "repository"):
            value = manifest.get(field)
            if value == "workspace":
                value = workspace.get(field)
            if not value:
                problems.append(f"Cargo.toml sin {field}")
                inconsistent = True
        state = "NO LISTO" if problems else "sin bloqueos en registros y manifiesto"
        blocked |= bool(problems)
        summary = ", ".join(f"{m}: {n}" for m, n in sorted(motives.items())) or "ninguna"
        print(f"== cpeforge-{package}: {state}")
        print(f"   diferencias del perfil seguro: {summary}")
        for problem in problems[:10]:
            print(f"   - {problem}")
        if len(problems) > 10:
            print(f"   - ... {len(problems) - 10} bloqueos adicionales en el registro")
    if compat_outdated:
        blocked = inconsistent = True
        print("oracle/compatibilidad.md desactualizado: ejecutar con --write-compat")
    print()
    print("No comprobado aquí (requisito de LISTO): CI ejecutado, suites PHP, gates de paridad y revisión independiente.")
    if "--ci" in sys.argv[1:]:
        return 1 if inconsistent else 0
    return 1 if blocked else 0


if __name__ == "__main__":
    sys.exit(main())
