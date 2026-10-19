#!/usr/bin/env python3
"""Genera la matriz de inventario y casos de cpeforge-xml desde los fixtures.

Uso: python3 oracle/cases/xml/build_matrix.py > oracle/evidencia/xml-0.1.0-matriz.md
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REF = ROOT / "tests/reference/xml"
XSD_TEST = ROOT / "crates/xml/tests/schema.rs"

INVENTORY = [
    ("Builder", "InvoiceBuilder", "InvoiceBuilder::render / Builder<Invoice>", "invoice{ublVersion}.xml.twig; error si no existe", "inv-version-*"),
    ("Builder", "NoteBuilder", "NoteBuilder::render / Builder<Note>", "notacr si tipoDoc === '07', si no notadb; + ublVersion", "note-selector-*, note-version-*"),
    ("Builder", "DespatchBuilder", "DespatchBuilder::render / Builder<Despatch>", "despatch2022 si version === '2022'", "desp-version-*"),
    ("Builder", "SummaryBuilder", "SummaryBuilder::render / Builder<Summary>", "summary.xml.twig", "summary-*"),
    ("Builder", "VoidedBuilder", "VoidedBuilder::render y render_reversion / Builder<Voided>, Builder<Reversion>", "voided.xml.twig (RA/RR)", "voided-*, reversion-*"),
    ("Builder", "PerceptionBuilder", "PerceptionBuilder::render / Builder<Perception>", "perception.xml.twig", "perception-*"),
    ("Builder", "RetentionBuilder", "RetentionBuilder::render / Builder<Retention>", "retention.xml.twig", "retention-*"),
    ("Builder", "TwigBuilder", "RenderOptions + render interno", "opciones autoescape (html/false), strict_variables, cache; zona America/Lima; filtros", "sufijos -default, -lite, -tests"),
    ("Filtro", "FormatFilter::number (n_format)", "format::n_format", "number_format($x, n, '.', '')", "filters/inputs.json"),
    ("Filtro", "FormatFilter::numberLimit (n_format_limit)", "format::n_format_limit", "texto PHP si ≤ n decimales y sin exponente", "filters/inputs.json"),
    ("Función", "TributoFunction::getByAfectacion", "format::tributo_by_afectacion", "(int) afectación → 1000/1016/9997/9998/9995, resto 9996", "filters/inputs.json"),
    ("Función", "TributoFunction::getByTributo", "format::tributo", "código exacto del catálogo 05", "filters/inputs.json"),
    ("Impresión", "{{ float }} en Twig", "format::float_string", "float → texto con precision=14", "filters/inputs.json"),
]

TEMPLATES = [
    "invoice2.0.xml.twig", "invoice2.1.xml.twig", "embededDespatch.xml.twig", "notacr2.0.xml.twig", "notacr2.1.xml.twig",
    "notadb2.0.xml.twig", "notadb2.1.xml.twig", "despatch.xml.twig", "despatch2022.xml.twig", "summary.xml.twig",
    "voided.xml.twig", "retention.xml.twig", "perception.xml.twig",
]


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def template_of(xml: str) -> str | None:
    root = re.search(r"<\?xml[^>]*\?><([A-Za-z]+)", xml)
    if not root:
        return None
    name = root.group(1)
    version = re.search(r"<cbc:UBLVersionID>([^<]*)<", xml)
    custom = re.search(r"<cbc:CustomizationID>([^<]*)<", xml)
    v = version.group(1) if version else ""
    c = custom.group(1) if custom else ""
    return {
        ("Invoice", "2.0"): "invoice2.0.xml.twig",
        ("Invoice", "2.1"): "invoice2.1.xml.twig",
        ("CreditNote", "2.0"): "notacr2.0.xml.twig",
        ("CreditNote", "2.1"): "notacr2.1.xml.twig",
        ("DebitNote", "2.0"): "notadb2.0.xml.twig",
        ("DebitNote", "2.1"): "notadb2.1.xml.twig",
    }.get((name, v)) or {
        "DespatchAdvice": "despatch2022.xml.twig" if c == "2.0" else "despatch.xml.twig",
        "SummaryDocuments": "summary.xml.twig",
        "VoidedDocuments": "voided.xml.twig",
        "Retention": "retention.xml.twig",
        "Perception": "perception.xml.twig",
    }.get(name)


def main() -> None:
    hashes = {}
    for line in (REF / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        hashes[name] = digest[:12]
    differences = load(REF / "differences.json")
    diff_cases = {d["case"] for d in differences["differences"]}
    invalid = set(re.findall(r'\("([a-z0-9-]+)", "', XSD_TEST.read_text()))

    rows = []
    template_cases: dict[str, list[str]] = {t: [] for t in TEMPLATES}
    for case_file in sorted((REF / "cases").glob("*.json")):
        family = case_file.stem
        expected = load(REF / "expected" / f"{family}.json")["results"]
        for case in load(case_file)["cases"]:
            cid = case["id"]
            php = expected[cid]
            options = json.dumps(case["options"], separators=(",", ":")) if case["options"] else "{}"
            if "error" in php:
                outcome = php["error"]["class"].replace("Twig\\Error\\", "")
                template = "—"
                xsd = "— (error)"
                fixture = f"expected/{family}.json@{hashes[f'expected/{family}.json']}"
            else:
                xml_path = f"expected/{family}/{cid}.xml"
                xml = (REF / xml_path).read_bytes().decode("utf-8", "replace")
                template = template_of(xml) or "?"
                template_cases.setdefault(template, []).append(cid)
                if "<sac:SUNATEmbededDespatchAdvice>" in xml:
                    template_cases["embededDespatch.xml.twig"].append(cid)
                outcome = f"XML {php['bytes']} bytes"
                xsd = "inválido esperado" if cid in invalid else "válido"
                fixture = f"{xml_path}@{hashes[xml_path]}"
            result = "DIFERENCIA_DELIBERADA" if cid in diff_cases else "IGUAL (bytes)" if "error" not in php else "IGUAL (error)"
            rows.append((cid, family, case["builder"], options, case["description"].replace("|", "\\|"), template, outcome, xsd, fixture, result))

    out = ["# Matriz de inventario y casos — cpeforge-xml 0.1.0", ""]
    out.append(
        "Generada con `python3 oracle/cases/xml/build_matrix.py` a partir de `tests/reference/xml/`. Resultados PHP de "
        "Greenter v5.3.0 (`f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`) con PHP 8.4.17 y Twig 3.30.0, un proceso por caso. "
        "Pruebas Rust: `crates/xml/tests/parity.rs` (bytes y errores) y `crates/xml/tests/schema.rs` (XSD con xmllint, "
        "salida PHP y Rust)."
    )
    out += ["", "## Builders, filtros y funciones", "", "| Tipo | PHP | Rust | Comportamiento | Casos | Estado |", "| --- | --- | --- | --- | --- | --- |"]
    for kind, php, rust, behaviour, cases in INVENTORY:
        out.append(f"| {kind} | `{php}` | `{rust}` | {behaviour} | {cases} | implementado y probado |")
    out += ["", "## Plantillas", "", "| Plantilla | Casos con XML igual al de PHP | Ejemplos |", "| --- | ---: | --- |"]
    for template in TEMPLATES:
        ids = template_cases.get(template, [])
        out.append(f"| `{template}` | {len(ids)} | {', '.join(ids[:4])}{' …' if len(ids) > 4 else ''} |")
    errors = sum(1 for r in rows if r[-1] == "IGUAL (error)")
    equal = sum(1 for r in rows if r[-1] == "IGUAL (bytes)")
    out += ["", "## Casos", "", f"Total: {len(rows)}; {equal} XML idénticos byte a byte, {errors} errores equivalentes, "
            f"{len(rows) - equal - errors} con DIFERENCIA_DELIBERADA.", ""]
    out.append("| ID | Familia | Builder | Opciones Twig | Comportamiento | Plantilla | Resultado PHP | XSD | Fixture (sha256) | Resultado |")
    out.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for row in rows:
        out.append("| " + " | ".join(row) + " |")
    filters = load(REF / "filters/expected.json")["results"]
    out += ["", "## Filtros", "", f"{len(filters)} entradas en `filters/inputs.json@{hashes['filters/inputs.json']}`, resultados PHP en "
            f"`filters/expected.json@{hashes['filters/expected.json']}`; prueba `parity.rs::filters_match_reference`. "
            f"{len(filters) - len(differences['filter_differences'])} IGUAL, {len(differences['filter_differences'])} DIFERENCIA_DELIBERADA "
            "(registradas en `differences.json`)."]
    print("\n".join(out))


if __name__ == "__main__":
    main()
