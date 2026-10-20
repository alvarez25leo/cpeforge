#!/usr/bin/env python3
"""Actualiza el catálogo de cpeforge-xcodes desde las reglas de validación de SUNAT.

Uso:

    python3 -I scripts/update_xcodes.py <Reglas de validación.xlsx> <AAAA-MM-DD>

El libro se descarga de https://cpe.sunat.gob.pe/guias-y-manuales («Reglas de
validación - actualizado al …»); la fecha es la de esa versión. Solo usa la
biblioteca estándar y trata el libro como dato: lee la hoja «CódigosRetorno»
(columna A código, B descripción) sin ejecutar nada de él.

Reglas de fusión, para que la divergencia con la referencia sea solo la real:

- código de la referencia cuyo texto coincide con el de SUNAT salvo espacios:
  se conserva el texto de la referencia byte a byte;
- código de la referencia con otro texto en SUNAT: texto de SUNAT;
- código nuevo de SUNAT: texto de SUNAT;
- un código de la referencia ausente en SUNAT es un error (no se borra nada
  sin revisarlo).

Escribe crates/xcodes/src/data/CodeErrors.xml y
tests/reference/xcodes/sunat-catalog.json (versión, origen, SHA-256 del libro,
códigos nuevos y cambiados) e imprime las constantes de crates/xcodes/src/lib.rs.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
import unicodedata
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "target/reference/v5.3.0/packages/xcodes/src/data/CodeErrors.xml"
OUTPUT = ROOT / "crates/xcodes/src/data/CodeErrors.xml"
SUMMARY = ROOT / "tests/reference/xcodes/sunat-catalog.json"
SOURCE = "https://cpe.sunat.gob.pe/guias-y-manuales"
SHEET = "CódigosRetorno"
MAIN = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
REL = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"


def sheet_rows(book: Path) -> list[dict[str, str]]:
    with zipfile.ZipFile(book) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        rels = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
        targets = {rel.get("Id"): rel.get("Target") for rel in rels}
        sheet = next(
            (s for s in workbook.iter(MAIN + "sheet") if s.get("name") == SHEET), None
        )
        if sheet is None:
            raise SystemExit(f"el libro no tiene la hoja {SHEET!r}")
        strings = [
            "".join(t.text or "" for t in item.iter(MAIN + "t"))
            for item in ET.fromstring(archive.read("xl/sharedStrings.xml")).iter(MAIN + "si")
        ]
        data = ET.fromstring(archive.read("xl/" + targets[sheet.get(REL + "id")]))
    rows = []
    for row in data.iter(MAIN + "row"):
        cells = {}
        for cell in row.iter(MAIN + "c"):
            column = "".join(ch for ch in cell.get("r", "") if ch.isalpha())
            value = cell.find(MAIN + "v")
            kind = cell.get("t")
            if kind == "s" and value is not None:
                cells[column] = strings[int(value.text or "0")]
            elif kind == "inlineStr":
                cells[column] = "".join(t.text or "" for t in cell.iter(MAIN + "t"))
            elif value is not None:
                cells[column] = value.text or ""
        rows.append(cells)
    return rows


def escape(text: str) -> str:
    """Escape mínimo de texto XML; `>` se deja literal, como en la referencia."""
    return text.replace("&", "&amp;").replace("<", "&lt;")


def normalize(text: str) -> str:
    return re.sub(r"\s+", " ", unicodedata.normalize("NFC", text)).strip()


def main() -> None:
    if len(sys.argv) != 3 or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", sys.argv[2]):
        raise SystemExit(__doc__)
    book = Path(sys.argv[1])
    version = sys.argv[2]
    sunat: dict[str, str] = {}
    for row in sheet_rows(book):
        code = (row.get("A") or "").strip()
        text = (row.get("B") or "").strip()
        if not re.fullmatch(r"\d{4}", code):
            continue  # encabezado y separadores
        if not text:
            raise SystemExit(f"código {code} sin descripción")
        if code in sunat:
            raise SystemExit(f"código {code} duplicado en el libro")
        sunat[code] = text
    reference = {
        node.get("code"): node.text or "" for node in ET.parse(REFERENCE).getroot()
    }
    missing = sorted(set(reference) - set(sunat))
    if missing:
        raise SystemExit(f"códigos de la referencia ausentes en SUNAT: {missing}")
    merged: dict[str, str] = {}
    changed, added = [], []
    for code in sorted(sunat):
        if code in reference and normalize(reference[code]) == normalize(sunat[code]):
            merged[code] = reference[code]
        else:
            merged[code] = sunat[code]
            (changed if code in reference else added).append(code)
    lines = ['﻿<?xml version="1.0" encoding="utf-8"?>', "<errors>"]
    lines += [f'<error code="{code}">{escape(text)}</error>' for code, text in merged.items()]
    lines.append("</errors>")
    payload = "\n".join(lines).encode("utf-8")
    OUTPUT.write_bytes(payload)
    summary = {
        "version": version,
        "source": SOURCE,
        "sheet": SHEET,
        "book_sha256": hashlib.sha256(book.read_bytes()).hexdigest(),
        "entries": len(merged),
        "reference_entries": len(reference),
        "changed": changed,
        "added": added,
    }
    SUMMARY.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"pub const CATALOG_VERSION: &str = \"{version}\";")
    print(f"pub const XML_SHA256: &str = \"{hashlib.sha256(payload).hexdigest()}\";")
    print(f"pub const ENTRY_COUNT: usize = {len(merged)};")
    print(f"const XML_LENGTH: usize = {len(payload)};")
    print(f"cambiados: {len(changed)}; nuevos: {len(added)}")


if __name__ == "__main__":
    main()
