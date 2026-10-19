#!/usr/bin/env python3
"""Congela todas las opciones y el orden de las reglas observadas en PHP."""

import json
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
inventory = json.loads((ROOT / "tests/reference/validator/inventory.json").read_text())
profiles = {}
for profile in ("default", "2.1"):
    classes = {}
    for name, meta in inventory["profiles"][profile].items():
        fields = []
        for field, rules in meta["fields"].items():
            items = []
            for rule in rules:
                items.append({"name": rule["name"], "options": rule["options"]})
            fields.append({"field": field, "rules": items})
        classes[name] = {"loader": meta["loader"], "has_metadata": meta["has_metadata"], "fields": fields}
    profiles[profile] = classes
parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, default=ROOT / "crates/validator/src/rules.json")
args = parser.parse_args()
path = args.output
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(profiles, ensure_ascii=False, separators=(",", ":")) + "\n")
print(f"{len(profiles['default'])} tipos, {path.stat().st_size} bytes")
