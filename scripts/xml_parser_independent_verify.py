#!/usr/bin/env python3
"""Verificación independiente de cpeforge-xml-parser.

Lee cada XML sintético de tests/reference/xml-parser/cases.json con expat
(xml.etree.ElementTree de la biblioteca estándar, sin libxml2 ni roxmltree) y
extrae raíz, espacio de nombres y campos críticos por familia siguiendo las
rutas UBL 2.0 que leen los parsers de Greenter. Comprueba que coinciden con
el documento que devuelve PHP (expected.json) y con el que devuelve Rust
(salida de la prueba de paridad con XML_PARSER_RUST_OUT).

Solo se verifican los casos en que PHP y Rust producen documento.

Uso: python3 -I scripts/xml_parser_independent_verify.py <cases.json> <expected.json> <rust.json>
"""

from __future__ import annotations

import base64
import json
import re
import sys
import xml.etree.ElementTree as ET
from decimal import Decimal, InvalidOperation
from pathlib import Path

CBC = "{urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2}"
CAC = "{urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2}"
SAC = "{urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1}"
PLAIN = re.compile(r"-?\d+(\.\d+)?")
ROOTS = {
    "invoice": {("urn:oasis:names:specification:ubl:schema:xsd:Invoice-2", "Invoice")},
    "receipt": {("urn:oasis:names:specification:ubl:schema:xsd:Invoice-2", "Invoice")},
    "note": {
        ("urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2", "CreditNote"),
        ("urn:oasis:names:specification:ubl:schema:xsd:DebitNote-2", "DebitNote"),
    },
    "despatch": {("urn:oasis:names:specification:ubl:schema:xsd:DespatchAdvice-2", "DespatchAdvice")},
    "summary": {("urn:sunat:names:specification:ubl:peru:schema:xsd:SummaryDocuments-1", "SummaryDocuments")},
    "voided": {("urn:sunat:names:specification:ubl:peru:schema:xsd:VoidedDocuments-1", "VoidedDocuments")},
    "retention": {("urn:sunat:names:specification:ubl:peru:schema:xsd:Retention-1", "Retention")},
    "perception": {("urn:sunat:names:specification:ubl:peru:schema:xsd:Perception-1", "Perception")},
}


def text(node) -> str:
    return "" if node is None else "".join(node.itertext())


def child_text(node, path: str) -> str:
    return text(node.find(path)) if node is not None else ""


def amount(value) -> Decimal | None:
    if isinstance(value, dict) and "$float" in value:
        value = value["$float"]
    try:
        return Decimal(str(value))
    except (InvalidOperation, TypeError):
        return None


def expected_fields(parser: str, root, data: bytes) -> dict:
    """Campos críticos leídos de forma independiente."""
    ident = child_text(root, CBC + "ID")
    parts = ident.split("-")
    get = lambda i: parts[i] if len(parts) > i else None
    out: dict = {}
    if parser in ("invoice", "note", "despatch", "receipt", "retention", "perception"):
        out["serie"], out["correlativo"] = get(0), get(1)
    if parser in ("summary", "voided"):
        out["correlativo"] = get(2)
    if parser == "voided":
        out["$class"] = "Voided" if get(0) == "RA" else "Reversion"
        out["company.ruc"] = child_text(root.find(CAC + "AccountingSupplierParty"), CBC + "CustomerAssignedAccountID")
        out["details#"] = len(root.findall(SAC + "VoidedDocumentsLine"))
    if parser == "summary":
        out["company.ruc"] = child_text(root.find(CAC + "AccountingSupplierParty"), CBC + "CustomerAssignedAccountID")
        out["details#"] = len(root.findall(SAC + "SummaryDocumentsLine"))
    if parser in ("invoice", "note"):
        out["company.ruc"] = child_text(root.find(CAC + "AccountingSupplierParty"), CBC + "CustomerAssignedAccountID")
        out["client.numDoc"] = child_text(root.find(CAC + "AccountingCustomerParty"), CBC + "CustomerAssignedAccountID")
        out["tipoMoneda"] = child_text(root, CBC + "DocumentCurrencyCode")
    if parser == "invoice":
        out["tipoDoc"] = child_text(root, CBC + "InvoiceTypeCode")
        payable = child_text(root.find(CAC + "LegalMonetaryTotal"), CBC + "PayableAmount").strip() or "0"
        # Solo importes decimales simples de hasta 15 cifras; las conversiones
        # de PHP `(float)` las cubre la prueba de paridad.
        if PLAIN.fullmatch(payable) and len(payable.replace("-", "").replace(".", "")) <= 15:
            out["mtoImpVenta"] = amount(payable)
        out["details#"] = len(root.findall(CAC + "InvoiceLine"))
    if parser == "note":
        # Greenter compara `nodeName` (con prefijo) con 'CreditNote': una raíz
        # `<cn:CreditNote>` se trata como débito (en PHP y en Rust).
        credit = root.tag == "{urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2}CreditNote" and re.search(
            rb"<CreditNote[\s>]", data
        ) is not None
        out["details#"] = len(root.findall(CAC + ("CreditNoteLine" if credit else "DebitNoteLine")))
    if parser == "despatch":
        out["company.ruc"] = child_text(root.find(CAC + "DespatchSupplierParty"), CBC + "CustomerAssignedAccountID")
        out["tipoDoc"] = child_text(root, CBC + "DespatchAdviceTypeCode")
        out["details#"] = len(root.findall(CAC + "DespatchLine"))
    if parser == "receipt":
        out["person.ruc"] = child_text(root.find(CAC + "AccountingSupplierParty"), CBC + "CustomerAssignedAccountID")
    if parser in ("retention", "perception"):
        kind = "Retention" if parser == "retention" else "Perception"
        out["company.ruc"] = child_text(root.find(CAC + "AgentParty"), CAC + "PartyIdentification/" + CBC + "ID")
        out["details#"] = len(root.findall(SAC + f"SUNAT{kind}DocumentReference"))
    return out


def actual_fields(snapshot: dict, keys) -> dict:
    out = {}
    for key in keys:
        if key == "$class":
            continue
        if key.endswith("#"):
            out[key] = len(snapshot.get(key[:-1]) or [])
            continue
        value = snapshot
        for part in key.split("."):
            value = (value or {}).get(part)
        out[key] = amount(value) if key.startswith("mto") else value
    return out


def main() -> int:
    cases = json.loads(Path(sys.argv[1]).read_text())["cases"]
    php = json.loads(Path(sys.argv[2]).read_text())["results"]
    rust = json.loads(Path(sys.argv[3]).read_text())["results"]
    root_dir = Path(sys.argv[1]).resolve().parents[3]
    checked = failures = 0
    for case in cases:
        parser = case["parser"]
        if parser == "reader" or "mode" in case:
            continue
        p, r = php[case["id"]], rust.get(case["id"], {})
        if "snapshot" not in p or "snapshot" not in r:
            continue
        inp = case["input"]
        if "xml" in inp:
            data = inp["xml"].encode()
        elif "b64" in inp:
            data = base64.b64decode(inp["b64"])
        elif "file" in inp:
            data = (root_dir / inp["file"]).read_bytes()
        else:
            continue
        root = ET.fromstring(data)
        ns, _, local = root.tag[1:].partition("}") if root.tag.startswith("{") else ("", "", root.tag)
        problems = []
        if parser != "invoice" and (ns, local) not in ROOTS[parser] and case["id"] not in ("invoice-root-other-namespace",):
            problems.append(f"raíz inesperada {ns} {local}")
        exp = expected_fields(parser, root, data)
        for side, out in (("PHP", p), ("Rust", r)):
            if "$class" in exp and out["class"] != exp["$class"]:
                problems.append(f"{side} clase {out['class']} != {exp['$class']}")
            got = actual_fields(out["snapshot"], exp.keys())
            for key, value in got.items():
                if value != exp[key]:
                    problems.append(f"{side} {key}: {value!r} != {exp[key]!r}")
        checked += 1
        if problems:
            failures += 1
            print(f"FALLO {case['id']}: " + "; ".join(problems))
    print(f"casos verificados: {checked}; fallos: {failures}")
    return 1 if failures or checked == 0 else 0


if __name__ == "__main__":
    sys.exit(main())
