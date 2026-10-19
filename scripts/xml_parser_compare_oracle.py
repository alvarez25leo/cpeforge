#!/usr/bin/env python3
"""Compara dos salidas del oráculo PHP de xml-parser ignorando lo que depende
del reloj: las ventanas `now` y las fechas a menos de tres días de la llamada
(`new DateTime()`, texto vacío o relativo como `tomorrow`), que se sustituyen
por su zona horaria. El resto debe coincidir exactamente.

Si la libxml2 del runtime no es la que generó el fixture, los textos de
`warnings` (diagnósticos de libxml2 que PHP reenvía) cambian de una versión a
otra; en ese caso se comparan todos los resultados salvo esos textos.

Uso: python3 -I scripts/xml_parser_compare_oracle.py <committed.json> <regenerated.json>
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

WINDOW = 3 * 24 * 3600


def normalize(value, before: int | None):
    if isinstance(value, dict):
        if "$datetime" in value and before is not None and abs(value["$datetime"]["unix"] - before) < WINDOW:
            return {"$clock": value["$datetime"]["tz"]}
        return {k: normalize(v, before) for k, v in value.items() if k != "now"}
    if isinstance(value, list):
        return [normalize(v, before) for v in value]
    return value


def result(entry: dict, with_warnings: bool = True):
    calls = entry.get("calls")
    if calls is not None:
        out = {"calls": [normalize(c, c["now"]["before"]["unix"]) for c in calls]}
        if with_warnings:
            out["warnings"] = entry.get("warnings")
        return out
    before = entry.get("now", {}).get("before", {}).get("unix")
    out = normalize(entry, before)
    if not with_warnings:
        out.pop("warnings", None)
    return out


def main() -> int:
    a = json.loads(Path(sys.argv[1]).read_text())
    b = json.loads(Path(sys.argv[2]).read_text())
    for key in ("reference", "composer_lock_sha256", "cases_sha256", "default_time_zone"):
        if a.get(key) != b.get(key):
            print(f"metadato distinto: {key} {a.get(key)} → {b.get(key)}")
            return 1
    for key in ("reference_version", "extensions", "libxml", "timezone_db"):
        if a.get(key) != b.get(key):
            print(f"aviso: {key} {a.get(key)} → {b.get(key)}")
    if a.get("load_warnings") != b.get("load_warnings"):
        print("diferencia en load_warnings")
        return 1
    same_libxml = a.get("libxml") == b.get("libxml")
    if not same_libxml:
        print("libxml2 distinta del fixture: se comparan los resultados sin los textos de warnings")
    ra, rb = a["results"], b["results"]
    bad = sorted(
        k
        for k in set(ra) | set(rb)
        if result(ra.get(k, {}), same_libxml) != result(rb.get(k, {}), same_libxml)
    )
    for key in bad:
        print("DIFIERE", key)
    print(f"resultados comparados: {len(ra)}; distintos: {len(bad)}")
    return 1 if bad or len(ra) != len(rb) else 0


if __name__ == "__main__":
    sys.exit(main())
