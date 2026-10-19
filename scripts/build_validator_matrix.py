#!/usr/bin/env python3
"""Construye la matriz auditable desde la reflexión PHP y sus casos ejecutados."""

import json
import argparse
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "tests/reference/validator"
inventory = json.loads((BASE / "inventory.json").read_text())
cases = json.loads((BASE / "cases.json").read_text())
results = json.loads((BASE / "reference-results.json").read_text())
assert [case["id"] for case in cases] == [result["id"] for result in results]
stats = Counter()
errors = Counter()
for case, result in zip(cases, results):
    profile, model = case["id"].split("/", 2)[:2]
    stats[(profile, model)] += 1
    errors[(profile, model)] += "error" in result

consumer = {
    "Address": "Client, Company", "Charge": "Invoice, SaleDetail",
    "Client": "Invoice, Note, Despatch, Perception, Retention",
    "Company": "Invoice, Note, Despatch, Summary, Voided, Retention, Perception",
    "Cuota": "Invoice, Note", "DetailAttribute": "SaleDetail",
    "DespatchDetail": "Despatch", "Direction": "Shipment, EmbededDespatch",
    "Document": "Invoice, Note, Despatch, SummaryDetail",
    "EmbededDespatch": "Invoice", "Exchange": "RetentionDetail, PerceptionDetail",
    "Legend": "Invoice, Note", "Payment": "RetentionDetail, PerceptionDetail",
    "PerceptionDetail": "Perception", "Prepayment": "Invoice",
    "RetentionDetail": "Retention", "SaleDetail": "Invoice, Note",
    "SalePerception": "Invoice, Note", "Shipment": "Despatch",
    "SummaryDetail": "Summary", "SummaryPerception": "SummaryDetail",
    "Transportist": "Shipment", "VoidedDetail": "Voided, Reversion",
}

lines = [
    "# Matriz de validator 0.1.0 frente a Greenter PHP v5.3.0",
    "",
    "Fuente fijada: `thegreenter/greenter` "
    "`f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`; Symfony Validator 6.4.48. "
    "Generar con `python3 scripts/build_validator_matrix.py`. "
    "El inventario JSON guarda el SHA-256 de cada fuente, campos, orden, opciones y versiones; "
    "`cases.json` y `reference-results.json` conservan cada entrada, salida y error por ID.",
    "",
    "## API y metadatos públicos",
    "",
    "| Fuente PHP | Entrada y comportamiento | Salida/error | Equivalente/consumidor |",
    "| --- | --- | --- | --- |",
    "| `SymfonyValidator.php` | `validate(DocumentInterface)`, `setVersion`, `getValidator`, `getMetadatFactory`; proveedor opcional | Violaciones ordenadas; error de tipo PHP | `Validator::validate_document`, `validate_model`, `set_version`, `metadata_for`; consumidores de `core` |",
    "| `MessageTranslator.php` | `trans(id, parámetros, dominio, locale)`; proveedor/recurso G001/fallback | Mensaje o ID; fallo del proveedor | `MessageTranslator`, `Validator::translate`; mensajes de restricciones y `xcodes` futuro |",
    "| `Metadata/CustomMetadataFactory.php` | Clase, versión, listener | Metadata efectiva, `hasMetadataFor`, listener tras carga | `metadata_for`, `has_metadata_for`, `RuleExtension`; todos los loaders |",
    "| `Metadata/LoaderMetadataInterface.php` | Objeto de clase registrada | Restricciones de clase y propiedad | Tabla `rules.json`; `Validator` |",
    "| `Metadata/LoaderListenerInterface.php` | Metadata recién cargada | Reglas añadidas y callbacks | `RuleExtension`; consumidor `Validator` |",
    "",
    "## Los 40 loaders (33 base y siete UBL 2.1)",
    "",
    "Cada caso usa el modelo `cpeforge-core` correspondiente, salvo los casos `raw` que "
    "ejercitan Symfony sin pasar por setters. La salida de éxito es una lista de "
    "violaciones con ruta, mensaje, plantilla, código, parámetros y valor rechazado; "
    "el fallo del setter es `TypeError`. Los contadores de error señalan las "
    "diferencias de mensaje entre PHP y Rust. `rules.json` conserva todas las opciones.",
    "",
    "| Fuente PHP | Perfil efectivo | Modelo y consumidor | Campos/reglas | Restricciones | Casos PHP / TypeError |",
    "| --- | --- | --- | ---: | --- | ---: |",
]
for path in sorted(inventory["files"]):
    if not path.startswith("Loader/"):
        continue
    variant = path.startswith("Loader/v21/")
    name = Path(path).stem.removesuffix("Loader")
    profile = "2.1" if variant else "default"
    meta = inventory["profiles"][profile][name]
    rule_names = sorted({rule["name"] for items in meta["fields"].values() for rule in items})
    rule_count = sum(map(len, meta["fields"].values()))
    profiles = [profile] if variant else ["default"] + ([] if name in ["Charge", "DetailAttribute", "Detraction", "Invoice", "Note", "SaleDetail", "SalePerception"] else ["2.1"])
    counts = ", ".join(f"{p}: {stats[(p, name)]}/{errors[(p, name)]}" for p in profiles)
    lines.append(
        f"| `{path}` | {profile} | `{name}` → {consumer.get(name, 'raíz o `Validator::validate_model`')} "
        f"| {len(meta['fields'])}/{rule_count} | {', '.join(rule_names)} | {counts} |"
    )
lines += [
    "",
    "En el perfil base, `Charge` y `DetailAttribute` no tienen loader: "
    "`hasMetadataFor` es falso y la validación devuelve cero violaciones. "
    "La versión desconocida `9.9` recae en el perfil base. Los casos de "
    "ausencia y cambio de versión están en `cases.json` y `api-probe.json`.",
    "",
    "## Suite PHP auditada",
    "",
    "Doce clases `*Test.php` y cuatro archivos de apoyo; ejecutadas con salida de red "
    "denegada por `sandbox-exec`. Los nombres se toman del checkout fijado:",
    "",
]
test_files = sorted((ROOT / "target/reference/v5.3.0/packages/validator/tests").rglob("*.php"))
assert len(test_files) == 16
for file in test_files:
    kind = "clase de prueba" if file.name.endswith("Test.php") else "apoyo"
    lines.append(f"- `{file.relative_to(ROOT / 'target/reference/v5.3.0/packages/validator/tests')}` — {kind}.")
lines += [
    "",
    f"Total de casos: {len(cases)}, con {sum(errors.values())} `TypeError` PHP. "
    "El runner/comparador recorre todos los IDs en orden; "
    "`scripts/check_validator_rules.py` comprueba opciones completas, cobertura por campo "
    "y bordes independientes.",
    "",
]
parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, default=ROOT / "oracle/evidencia/validator-0.1.0-matriz.md")
args = parser.parse_args()
out = args.output
out.write_text("\n".join(lines))
print(f"{len(inventory['files'])} fuentes, {len(cases)} casos → {out}")
