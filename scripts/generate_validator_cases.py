#!/usr/bin/env python3
"""Casos sintéticos derivados solo del inventario PHP fijado."""

import json
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
inventory = json.loads((ROOT / "tests/reference/validator/inventory.json").read_text())
cases = []

# Tipo real de las propiedades `Valid`, obtenido de getters/constructores de core PHP.
NESTED = {
    "Client": {"address": "Address"},
    "Company": {"address": "Address"},
    "Despatch": {"destinatario": "Client", "company": "Company", "tercero": "Client", "envio": "Shipment", "docBaja": "Document", "relDoc": "Document", "details": "DespatchDetail"},
    "EmbededDespatch": {"llegada": "Direction", "partida": "Direction", "transportista": "Client"},
    "Invoice": {"client": "Client", "company": "Company", "formaPago": "FormaPagoCredito", "cuotas": "Cuota", "seller": "Client", "details": "SaleDetail", "legends": "Legend", "guias": "Document", "anticipos": "Prepayment", "detraccion": "Detraction", "relDocs": "Document", "perception": "SalePerception", "guiaEmbebida": "EmbededDespatch", "cargos": "Charge", "descuentos": "Charge"},
    "Note": {"client": "Client", "company": "Company", "formaPago": "FormaPagoCredito", "cuotas": "Cuota", "details": "SaleDetail", "legends": "Legend", "guias": "Document", "relDocs": "Document", "perception": "SalePerception"},
    "Perception": {"company": "Company", "proveedor": "Client", "details": "PerceptionDetail"},
    "PerceptionDetail": {"cobros": "Payment", "tipoCambio": "Exchange"},
    "Retention": {"company": "Company", "proveedor": "Client", "details": "RetentionDetail"},
    "RetentionDetail": {"pagos": "Payment", "tipoCambio": "Exchange"},
    "Reversion": {"company": "Company", "details": "VoidedDetail"},
    "SaleDetail": {"cargos": "Charge", "descuentos": "Charge", "atributos": "DetailAttribute"},
    "Shipment": {"transportista": "Transportist", "llegada": "Direction", "partida": "Direction"},
    "Summary": {"company": "Company", "details": "SummaryDetail"},
    "SummaryDetail": {"docReferencia": "Document", "percepcion": "SummaryPerception"},
    "Voided": {"company": "Company", "details": "VoidedDetail"},
}
LIST_FIELDS = {"details", "legends", "guias", "anticipos", "relDocs", "cargos", "descuentos", "cuotas", "atributos", "cobros", "pagos"}


def add(profile, model, field, label, value, raw=False):
    cases.append({
        "id": f"{profile}/{model}/{field}/{label}",
        "version": "2.1" if profile == "2.1" else None,
        "model": {"type": model, "fields": {} if field == "_" else {field: value}, **({"raw": True} if raw else {})},
    })


for profile in ("default", "2.1"):
    for model, meta in inventory["profiles"][profile].items():
        add(profile, model, "_", "default", None)
        for field, rules in meta["fields"].items():
            names = {rule["name"] for rule in rules}
            if "NotBlank" in names or "NotNull" in names:
                add(profile, model, field, "null", None)
            if "NotBlank" in names:
                for label, value in (("empty", ""), ("space", " "), ("zero", 0), ("false", False)):
                    add(profile, model, field, label, value)
                # Fronteras de NotBlank: otros espacios y "0" (no vacío).
                for label, value in (("tab", "\t"), ("newline", "\n"), ("nbsp", "\u00a0"), ("zero-string", "0")):
                    add(profile, model, field, label, value)
            for rule in rules:
                opts = rule["options"]
                if rule["name"] == "Length":
                    for bound in ("min", "max"):
                        if bound in opts:
                            n = opts[bound]
                            for length in (max(0, n - 1), n, n + 1):
                                add(profile, model, field, f"{bound}-{length}", "A" * length)
                            add(profile, model, field, f"{bound}-unicode-{n+1}", "ñ" * (n + 1))
                elif rule["name"] == "Regex":
                    add(profile, model, field, "regex-valid", "1" * (11 if "11" in opts["pattern"] else 8))
                    add(profile, model, field, "regex-invalid", "ABC")
                elif rule["name"] == "Choice":
                    add(profile, model, field, "choice-valid", opts["choices"][0])
                    add(profile, model, field, "choice-invalid", "99")
                elif rule["name"] == "Type":
                    add(profile, model, field, "type-string", "ABC")
                    add(profile, model, field, "type-zero", 0)
                    for label, value in (("null", None), ("empty", ""), ("space", " "), ("integer", 3), ("float", 3.5), ("numeric-string", "3.5"), ("text", "ABC"), ("false", False), ("true", True),
                                         ("exponent", "1e3"), ("negative", -1), ("large", 1e15), ("leading-zero", "007")):
                        add(profile, model, field, f"raw-type-{label}", value, True)
                elif rule["name"] == "Required":
                    for label, value in (("null", None), ("empty", ""), ("false", False)):
                        add(profile, model, field, f"raw-required-{label}", value, True)
            if "Valid" in names:
                child = NESTED[model][field]
                item = {"type": child, "fields": {}}
                add(profile, model, field, "valid-nested", [item] if field in LIST_FIELDS else item)

# Listas anidadas, fallback y ciclo de versiones se caracterizan por separado.
for profile in ("default", "2.1"):
    add(profile, "Invoice", "client", "nested-invalid", {"type": "Client", "fields": {"tipoDoc": "12"}})
    add(profile, "Invoice", "details", "nested-list", [{"type": "SaleDetail", "fields": {}}, {"type": "SaleDetail", "fields": {"descripcion": ""}}])
    add(profile, "Client", "address", "nested-address", {"type": "Address", "fields": {"ubigueo": "1234567"}})
    add(profile, "Voided", "details", "nested-voided", [{"type": "VoidedDetail", "fields": {}}, {"type": "VoidedDetail", "fields": {"correlativo": "ABC"}}])

# Ocho familias documentales válidas en ambos perfiles. Son entradas
# sintéticas; el resultado esperado se calcula ejecutando PHP, no aquí.
valid_roots = json.loads((ROOT / "tests/reference/validator/valid-roots.json").read_text())
assert len(valid_roots) == 16
cases.extend(valid_roots)

# Listas anidadas de varios elementos a partir de las raíces válidas: tres
# elementos válidos, y un elemento vacío entre dos válidos (índice 1).
for root in valid_roots:
    for field, items in root["model"]["fields"].items():
        if isinstance(items, list) and items and isinstance(items[0], dict) and "type" in items[0]:
            prefix = root["id"].rsplit("/", 2)[0]
            item = items[0]
            empty = {"type": item["type"], "fields": {}}
            for label, value in (("multi-valid", [item, item, item]), ("multi-mixed", [item, empty, item])):
                cases.append({**root, "id": f"{prefix}/{field}/{label}",
                              "model": {**root["model"], "fields": {**root["model"]["fields"], field: value}}})

# Límites de seguridad de Rust (perfil seguro, motivo «seguridad»): PHP no
# los impone. Las entradas grandes se describen con $repeat/$nest y las
# expanden ambos oráculos.
LIMITS = (
    ("Vehicle", "placa", "limit-bytes-1000001", {"$repeat": {"text": "A", "count": 1_000_001}}, True),
    ("Vehicle", "secundarios", "limit-nodes-100001", {"$repeat": {"value": 0, "count": 100_001}}, True),
    ("Vehicle", "secundarios", "limit-depth-33", {"$nest": {"depth": 33, "leaf": 0}}, True),
    ("Invoice", "details", "limit-violations-10012", {"$repeat": {"value": {"type": "SaleDetail", "fields": {}}, "count": 1000}}, False),
)
for model, field, label, value, raw in LIMITS:
    add("2.1", model, field, label, value, raw)

cases.sort(key=lambda case: case["id"])
ids = [case["id"] for case in cases]
if len(ids) != len(set(ids)):
    raise SystemExit("IDs de casos duplicados")
parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, default=ROOT / "tests/reference/validator/cases.json")
args = parser.parse_args()
args.output.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + "\n")
print(f"{len(cases)} casos")
