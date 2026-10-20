#!/usr/bin/env python3
"""Comprobación independiente de CodeErrors.xml con ElementTree (no roxmltree).

El catálogo es el de SUNAT (scripts/update_xcodes.py); se comprueba que
contiene el de la referencia salvo los textos que SUNAT cambió.
"""

from __future__ import annotations

import hashlib
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "target/reference/v5.3.0/packages/xcodes/src/data/CodeErrors.xml"
PACKAGED = ROOT / "crates/xcodes/src/data/CodeErrors.xml"
EXPECTED = ROOT / "tests/reference/xcodes/expected.json"
SUMMARY = ROOT / "tests/reference/xcodes/sunat-catalog.json"
SHA256 = "108d64164c0a3c0eb70de700c3e1f14e9a8aa724ce06aac52020170114f43216"
LENGTH = 222841


def rows_of(payload: bytes) -> list[ET.Element]:
    assert payload.startswith(b"\xef\xbb\xbf")
    payload[3:].decode("utf-8", errors="strict")
    root = ET.fromstring(payload)
    assert root.tag == "errors" and not root.attrib
    return list(root)


def check() -> None:
    packaged = PACKAGED.read_bytes()
    assert len(packaged) == LENGTH
    assert hashlib.sha256(packaged).hexdigest() == SHA256
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    rows = rows_of(packaged)
    codes = [node.attrib["code"] for node in rows]
    assert len(rows) == len(set(codes)) == summary["entries"]
    assert codes == sorted(codes)
    assert all(node.tag == "error" and not list(node) and len(node.attrib) == 1 for node in rows)
    assert all(code.isascii() and code.isdigit() and (node.text or "") for code, node in zip(codes, rows))
    messages = {node.attrib["code"]: node.text for node in rows}
    # La referencia (catálogo de 2018) está contenida: mismos códigos y mismos
    # textos salvo los que SUNAT cambió; los demás son los nuevos de SUNAT.
    reference = rows_of(REFERENCE.read_bytes())
    assert len(reference) == summary["reference_entries"]
    changed = set(summary["changed"])
    for node in reference:
        code = node.attrib["code"]
        assert code in messages, f"falta {code}"
        if code not in changed:
            assert messages[code] == node.text, f"mensaje {code}"
    assert set(messages) - {n.attrib["code"] for n in reference} == set(summary["added"])
    fixture = json.loads(EXPECTED.read_text(encoding="utf-8"))
    assert len(fixture["all"]) == len(reference)
    for index, (node, item) in enumerate(zip(reference, fixture["all"])):
        assert item["xml_code"] == node.attrib["code"], f"código {index}"
        assert item["message"] == node.text, f"mensaje {index}"
    samples = {"0100": "El sistema no puede responder su solicitud.",
               "1001": "ID - El dato SERIE-CORRELATIVO",
               "4366": "El XML no contiene tag"}
    for code, start in samples.items():
        assert (messages[code] or "").startswith(start), code
    print(
        f"XML independiente: SHA-256, BOM, UTF-8, {len(rows)} códigos únicos "
        f"(SUNAT {summary['version']}), referencia contenida y mensajes OK"
    )


if __name__ == "__main__":
    try:
        check()
    except (AssertionError, KeyError, ValueError, ET.ParseError) as error:
        print(f"xcodes XML: {error}", file=sys.stderr)
        raise SystemExit(1) from error
