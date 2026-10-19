#!/usr/bin/env python3
"""Auditoría de los 70 XML de prueba de greenter/xml-parser v5.3.0.

Se ejecuta ANTES de la suite upstream. No imprime contenido de los archivos:
solo agregados (raíz, namespace, UBLVersionID, DTD/entidades, firma y
certificado embebidos, y recuentos de patrones identificables). Los XML no se
copian a fixtures ni a evidencias.

Uso: python3 -I scripts/audit_xml_parser_resources.py <dir_resources> <salida.json>
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
import xml.parsers.expat
from collections import Counter
from pathlib import Path

RUC = re.compile(rb"(?<!\d)(10|15|16|17|20)\d{9}(?!\d)")
DNI = re.compile(rb"(?<![\d.])\d{8}(?![\d.])")
EMAIL = re.compile(rb"[\w.+-]+@[\w-]+\.[\w.]+")
PHONE = re.compile(rb"(?<!\d)9\d{8}(?!\d)")
SYNTHETIC_RUCS = {b"20000000001", b"20123456789", b"20100066603"}


def root_info(data: bytes) -> dict:
    info = {"root": None, "namespace": None, "ubl_version": None}
    parser = xml.parsers.expat.ParserCreate(namespace_separator="|")
    stack = []
    text = []

    def start(name, attrs):
        if info["root"] is None:
            ns, _, local = name.rpartition("|")
            info["root"], info["namespace"] = local, ns
        stack.append(name)
        text.clear()

    def end(name):
        if name.endswith("|UBLVersionID") and info["ubl_version"] is None and len(stack) == 2:
            info["ubl_version"] = "".join(text).strip()
        stack.pop()

    parser.StartElementHandler = start
    parser.EndElementHandler = end
    parser.CharacterDataHandler = lambda t: text.append(t)
    try:
        parser.Parse(data, True)
        info["well_formed"] = True
    except xml.parsers.expat.ExpatError as e:
        info["well_formed"] = False
        info["error"] = str(e).split(":")[0]
    return info


def main() -> int:
    base = Path(sys.argv[1])
    files = sorted(p for p in base.rglob("*") if p.is_file() and p.suffix.lower() == ".xml")
    rows = []
    totals = Counter()
    for path in files:
        data = path.read_bytes()
        rucs = {m.group(0) for m in RUC.finditer(data)}
        info = root_info(data)
        row = {
            "file": str(path.relative_to(base)),
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            **info,
            "doctype": b"<!DOCTYPE" in data,
            "entity_decl": b"<!ENTITY" in data,
            "signature": b"SignatureValue" in data,
            "x509_certificate": b"X509Certificate" in data,
            "ruc_like": len(rucs),
            "ruc_not_synthetic": len(rucs - SYNTHETIC_RUCS),
            "dni_like": len({m.group(0) for m in DNI.finditer(data)}),
            "email_like": len({m.group(0) for m in EMAIL.finditer(data)}),
            "phone_like": len({m.group(0) for m in PHONE.finditer(data)}),
        }
        row["identifiable"] = bool(row["ruc_not_synthetic"] or row["email_like"] or row["x509_certificate"])
        rows.append(row)
        totals["files"] += 1
        for k in ("doctype", "entity_decl", "signature", "x509_certificate", "identifiable"):
            totals[k] += int(row[k])
        totals[f"root:{info['root']}"] += 1
        totals[f"ubl:{info['ubl_version']}"] += 1
    summary = {"directory": str(base), "totals": dict(sorted(totals.items())), "files": rows}
    Path(sys.argv[2]).write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary["totals"], indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
