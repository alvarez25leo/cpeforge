#!/usr/bin/env python3
"""Revisor independiente de tabla PHP, cobertura y límites conocidos."""

import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
base = ROOT / "tests/reference/validator"
inventory = json.loads((base / "inventory.json").read_text())
rules = json.loads((ROOT / "crates/validator/src/rules.json").read_text())
cases = json.loads((base / "cases.json").read_text())
results = json.loads((base / "reference-results.json").read_text())
valid_roots = json.loads((base / "valid-roots.json").read_text())
by_id = {row["id"]: row for row in results}
case_ids = {case["id"] for case in cases}
assert len(inventory["files"]) == 45
assert len(cases) == len(case_ids) == len(results) == len(by_id)
assert len(valid_roots) == 16
assert {case["id"] for case in valid_roots} == {
    f"{profile}/{model}/_/valid-root"
    for profile in ("default", "2.1")
    for model in ("Invoice", "Note", "Despatch", "Summary", "Voided", "Reversion", "Retention", "Perception")
}
assert all(by_id[case["id"]].get("violations") == [] for case in valid_roots)
assert all(name.endswith(".php") and len(digest) == 64 for name, digest in inventory["files"].items())
assert sum(name.startswith("Loader/") and "/v21/" not in name for name in inventory["files"]) == 33
assert sum(name.startswith("Loader/v21/") for name in inventory["files"]) == 7

for profile, expected_rules, expected_valid in (("default", 293, 50), ("2.1", 307, 59)):
    php = inventory["profiles"][profile]
    rust = rules[profile]
    assert len(php) == len(rust) == 35
    assert sum(meta["has_metadata"] for meta in php.values()) == (33 if profile == "default" else 35)
    assert sum(len(items) for meta in php.values() for items in meta["fields"].values()) == expected_rules
    assert sum(rule["name"] == "Valid" for meta in php.values() for items in meta["fields"].values() for rule in items) == expected_valid
    for model, meta in php.items():
        frozen = rust[model]
        assert frozen["has_metadata"] == meta["has_metadata"]
        assert frozen["loader"] == meta["loader"]
        assert [(item["field"], [rule["name"] for rule in item["rules"]]) for item in frozen["fields"]] == [
            (field, [rule["name"] for rule in items]) for field, items in meta["fields"].items()
        ]
        for frozen_field, (field, php_rules) in zip(frozen["fields"], meta["fields"].items()):
            assert frozen_field["field"] == field
            assert frozen_field["rules"] == php_rules, (profile, model, field)
        assert f"{profile}/{model}/_/default" in case_ids
        for field, items in meta["fields"].items():
            assert any(case_id.startswith(f"{profile}/{model}/{field}/") for case_id in case_ids)
            if any(item["name"] == "Valid" for item in items):
                assert f"{profile}/{model}/{field}/valid-nested" in case_ids
            if any(item["name"] == "Length" for item in items):
                assert any(case_id.startswith(f"{profile}/{model}/{field}/max-") or case_id.startswith(f"{profile}/{model}/{field}/min-") for case_id in case_ids)


def paths(case_id):
    assert case_id in by_id, case_id
    assert "error" not in by_id[case_id], case_id
    return [v["path"] for v in by_id[case_id]["violations"]]


# Límites independientes de la implementación Rust.
assert "ubigueo" not in paths("default/Address/ubigueo/max-6")
assert "ubigueo" in paths("default/Address/ubigueo/max-7")
assert "ubigueo" in paths("default/Address/ubigueo/max-unicode-7")
assert "ruc" not in paths("default/Company/ruc/regex-valid")
assert "ruc" in paths("default/Company/ruc/regex-invalid")
assert "tipoDoc" not in paths("default/Invoice/tipoDoc/choice-valid")
assert "tipoDoc" in paths("default/Invoice/tipoDoc/choice-invalid")
assert "serie" not in paths("2.1/Invoice/serie/max-4")
assert "serie" in paths("2.1/Invoice/serie/max-5")
assert paths("default/Charge/_/default") == []
assert set(paths("2.1/Charge/_/default")) == {"factor", "monto", "montoBase", "codTipo"}
assert paths("default/DespatchDetail/_/default") == ["cantidad"]
assert "address.ubigueo" in paths("default/Client/address/valid-nested")
assert "mto" in paths("default/SalePerception/mto/raw-type-numeric-string")
assert "pesoBruto" not in paths("default/EmbededDespatch/pesoBruto/raw-type-numeric-string")

covered = Counter(rule["name"] for profile in ("default", "2.1") for meta in inventory["profiles"][profile].values() for items in meta["fields"].values() for rule in items)
assert set(covered) == {"NotBlank", "NotNull", "Length", "Valid", "Type", "Regex", "Required", "Choice"}
print(f"Tabla independiente: 45 fuentes, 33+7 loaders, {len(cases)} casos, {sum(covered.values())} reglas en 2 perfiles: OK")
