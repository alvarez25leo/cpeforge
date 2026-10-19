#!/usr/bin/env python3
"""Verificación independiente de cpeforge-cpe-validator (ni PHP ni Rust).

Para los casos con `independent` en tests/reference/cpe-validator/cases.json:
- `rules`: evalúa el mini DSL de reglas del generador con ElementTree y `re`
  de Python (otro motor XML y otro motor de expresiones regulares) y obtiene
  los mensajes esperados, que se convierten en errores con una
  reimplementación propia del formato `nivel|código|mensaje|ruta|valor`.
- `xsltproc`: ejecuta la misma hoja con el procesador XSLT 1.0 `xsltproc`
  (binario del sistema, --nonet) y toma sus `xsl:message` de stderr. Para los
  casos `cpe-type-*` el tipo y la ruta se deducen aquí de la raíz del XML.
Compara con la salida Rust (CPE_VALIDATOR_RUST_OUT de tests/parity.rs) y,
como control, con el oráculo PHP.

Uso: python3 -I scripts/cpe_validator_independent_verify.py <cases.json> <expected.json> <rust.json> [<salida.json>]
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

CBC = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"


def to_error(raw: str) -> list | None:
    parts = raw.split("|")
    if len(parts) < 3:
        return None
    level = "1" if parts[0] == "1" else "2"
    path, value = (parts[3], parts[4]) if len(parts) >= 5 else (None, None)
    return [parts[1], parts[2], level, path, value]


def first_child_text(root: ET.Element, local: str) -> str | None:
    for child in root:
        if child.tag == f"{{{CBC}}}{local}":
            return "".join(child.itertext())
    return None


def eval_rules(rules: list[dict], xml: str, filename: str) -> list[str]:
    root = ET.fromstring(xml)
    out = []
    for rule in rules:
        if "param" in rule:
            out.append(f"{rule['level']}|{rule['code']}|{rule['message']}|{filename}")
            if rule.get("terminate"):
                break
            continue
        value = first_child_text(root, rule["select"]) or ""
        matched = re.search(rule["regex"], value) is not None
        fire = not matched if rule.get("negate", True) else matched
        if fire:
            out.append(f"{rule['level']}|{rule['code']}|{rule['message']}|{rule['path']}|{value}")
            if rule.get("terminate"):
                break
    return out


def local(tag: str) -> str:
    return tag.split("}", 1)[-1]


def independent_type(xml: str) -> str | None:
    root = ET.fromstring(xml)
    name = local(root.tag)
    if name == "Invoice":
        return "03" if first_child_text(root, "InvoiceTypeCode") == "03" else "01"
    if name == "VoidedDocuments":
        return "RR" if (first_child_text(root, "ID") or "").startswith("RR") else "RA"
    return {"CreditNote": "07", "DebitNote": "08", "SummaryDocuments": "RC", "DespatchAdvice": "09",
            "Retention": "20", "Perception": "40"}.get(name)


def xsltproc(xsl: str, xml: str, filename: str) -> list[str]:
    with tempfile.TemporaryDirectory() as tmp:
        sheet = Path(tmp) / "s.xsl"
        doc = Path(tmp) / "d.xml"
        sheet.write_text(xsl, encoding="utf-8")
        doc.write_text(xml, encoding="utf-8")
        proc = subprocess.run(
            ["xsltproc", "--nonet", "--novalid", "--stringparam", "nombreArchivoEnviado", filename, str(sheet), str(doc)],
            capture_output=True, text=True, check=False,
        )
    return [line for line in proc.stderr.splitlines() if line and not line.startswith("no result for")]


def main() -> int:
    cases = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    php = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))["results"]
    rust_doc = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
    rust = rust_doc["results"]
    report = {"engine": rust_doc.get("engine"), "checked": [], "failures": []}
    for case in cases["cases"]:
        spec = case.get("independent")
        if not spec:
            continue
        cid = case["id"]
        steps = case["steps"]
        last = steps[-1]
        xml = last["xml"]["xml"]
        filename = last["filename"]
        if spec["kind"] == "rules":
            messages = eval_rules(spec["rules"], xml, filename)
            engine = "python-re+elementtree"
        elif case["family"] == "cpe":
            code = independent_type(xml)
            rel = cases["paths"].get(code)
            for i, want in ((0, code), (1, f"{{ROOT}}/{rel}" if rel else None)):
                got = rust[cid]["steps"][i].get("value")
                if got != want:
                    report["failures"].append(f"{cid} paso {i}: independiente {want!r} ≠ Rust {got!r}")
            messages = xsltproc(case["rules"][rel], xml, filename)
            engine = "xsltproc"
        else:
            messages = xsltproc(case["rules"]["main.xsl"], xml, filename)
            engine = "xsltproc"
        expected = {"outcome": "errors", "errors": [e for e in map(to_error, messages) if e is not None]}
        got_rust = rust[cid]["steps"][-1]
        got_php = php[cid]["steps"][-1]
        ok = got_rust == expected
        report["checked"].append({"id": cid, "engine": engine, "messages": len(messages), "ok": ok, "php_agrees": got_php == expected})
        if not ok:
            report["failures"].append(f"{cid} ({engine}): independiente {expected} ≠ Rust {got_rust}")
        if got_php != expected:
            report["failures"].append(f"{cid} ({engine}): independiente {expected} ≠ PHP {got_php}")
    if len(sys.argv) > 4:
        Path(sys.argv[4]).write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    for failure in report["failures"]:
        print("FALLO", failure)
    print(f"{len(report['checked'])} casos verificados de forma independiente; {len(report['failures'])} fallos")
    return 1 if report["failures"] or not report["checked"] else 0


if __name__ == "__main__":
    sys.exit(main())
