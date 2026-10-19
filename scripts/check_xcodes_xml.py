#!/usr/bin/env python3
"""Comprobación independiente de CodeErrors.xml con ElementTree (no roxmltree)."""

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
SHA256 = "dbfaaa646d538ce693d9129fc02ddc4adb660271167ee8cccd0624e9c8997fdb"


def check() -> None:
    original = REFERENCE.read_bytes()
    packaged = PACKAGED.read_bytes()
    assert original == packaged, "El XML empaquetado difiere de la referencia PHP"
    assert len(original) == 182038
    assert original.startswith(b"\xef\xbb\xbf")
    assert hashlib.sha256(original).hexdigest() == SHA256
    original[3:].decode("utf-8", errors="strict")
    root = ET.fromstring(original)
    assert root.tag == "errors" and not root.attrib
    rows = list(root)
    codes = [node.attrib["code"] for node in rows]
    assert len(rows) == len(set(codes)) == 1710
    assert all(node.tag == "error" and not list(node) and len(node.attrib) == 1 for node in rows)
    assert all(code.isascii() and code.isdigit() and (node.text or "") for code, node in zip(codes, rows))
    fixture = json.loads(EXPECTED.read_text(encoding="utf-8"))
    assert len(fixture["all"]) == 1710
    for index, (node, item) in enumerate(zip(rows, fixture["all"])):
        assert item["xml_code"] == node.attrib["code"], f"código {index}"
        assert item["message"] == node.text, f"mensaje {index}"
    samples = {"0100": "El sistema no puede responder su solicitud.",
               "1001": "ID - El dato SERIE-CORRELATIVO",
               "4366": "Para el tipo de operación 2104"}
    for code, start in samples.items():
        index = codes.index(code)
        assert (rows[index].text or "").startswith(start), code
    print("XML independiente: SHA-256, BOM, UTF-8, 1 710 códigos únicos, orden y mensajes OK")


if __name__ == "__main__":
    try:
        check()
    except (AssertionError, KeyError, ValueError, ET.ParseError) as error:
        print(f"xcodes XML: {error}", file=sys.stderr)
        raise SystemExit(1) from error
