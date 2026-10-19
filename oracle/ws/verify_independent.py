#!/usr/bin/env python3
"""Segunda verificación de WSDL/XSD y CDR con xmllint y ElementTree."""

import json
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

cases = {c["id"]: c for c in json.loads(Path(sys.argv[1]).read_text())["cases"]}
expected = json.loads(Path(sys.argv[2]).read_text())["results"]
resources = Path(sys.argv[3])
files = sorted([*resources.glob("*.wsdl"), *resources.glob("*.xsd")])
assert len(files) == 6, f"se esperaban 6 WSDL/XSD, hay {len(files)}"
for path in files:
    result = subprocess.run(["xmllint", "--nonet", "--noout", str(path)], capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(f"XML inválido: {path.name}: {result.stderr}")
    root = ET.parse(path).getroot()
    for node in root.iter():
        for key in ("location", "schemaLocation"):
            location = node.attrib.get(key)
            if location and not location.startswith("https://") and not (resources / location).is_file():
                raise SystemExit(f"importación WSDL/XSD no local: {location}")

CBC = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"
CAC = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"
for case_id in ("cdr-ok", "cdr-no-notes"):
    root = ET.fromstring(cases[case_id]["xml"])
    response = root.find(f"{{{CAC}}}DocumentResponse/{{{CAC}}}Response")
    docref = root.find(f"{{{CAC}}}DocumentResponse/{{{CAC}}}DocumentReference")
    def text(node, name):
        value = node.find(f"{{{CBC}}}{name}") if node is not None else None
        return value.text if value is not None else None
    actual = {
        "id": text(response, "ReferenceID"),
        "code": text(response, "ResponseCode"),
        "description": text(response, "Description"),
        "notes": [node.text or "" for node in root.findall(f"{{{CBC}}}Note")],
        "reference": text(docref, "DocumentDescription"),
    }
    if actual != expected[case_id]["result"]:
        raise SystemExit(f"CDR independiente distinto en {case_id}: {actual}")
print("seis WSDL/XSD y dos CDR: verificación independiente OK")
