#!/usr/bin/env python3
"""Exige que un fixture PHP/reglas vuelva a generarse exactamente."""

import json
import sys
from pathlib import Path

if len(sys.argv) != 3:
    raise SystemExit("Uso: verify_validator_snapshot.py esperado.json generado.json")
expected, actual = (json.loads(Path(path).read_text()) for path in sys.argv[1:])
if expected != actual:
    raise SystemExit(f"Fixture no reproducible: {sys.argv[1]} != {sys.argv[2]}")
print(f"Fixture reproducible: {sys.argv[1]}")
