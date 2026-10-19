#!/usr/bin/env bash
# Gate de paridad de cpeforge-xml contra Greenter PHP fijado.
#
# 1. Prepara la referencia PHP aislada y las dependencias fijadas del oráculo
#    (oracle/php/composer.lock: Twig 3.30.0 y XSD de greenter/ubl-validator 2.3.0).
# 2. Regenera casos y salidas PHP (un proceso por caso) en un directorio
#    temporal y exige igualdad byte a byte con tests/reference/xml.
# 3. Verifica tests/reference/xml/SHA256SUMS.
# 4. Ejecuta las pruebas Rust de xml: paridad, XSD (xmllint), selección y límites.
#
# Uso: scripts/xml_parity.sh [--update]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ref_dir="$root/tests/reference/xml"
update=false
[[ "${1:-}" == "--update" ]] && update=true

command -v xmllint >/dev/null || { echo "falta xmllint (libxml2)" >&2; exit 1; }
GREENTER_PHP_REFERENCE="$("$root/scripts/prepare_php_reference.sh")"
export GREENTER_PHP_REFERENCE
(cd "$root/oracle/php" && composer install --no-interaction --no-progress --quiet)
echo "referencia PHP: $(git -C "$GREENTER_PHP_REFERENCE" rev-parse HEAD)"
php -n --version | head -1
xmllint --version 2>&1 | head -1

work="$(mktemp -d)"
source "$root/scripts/evidence.sh"
trap 'save_evidence xml-regenerated "$work/cases" "$work/expected" "$work/filters"; rm -rf "$work"' EXIT
python3 "$root/oracle/cases/xml/generate_cases.py" "$work"
mkdir -p "$work/expected"
for cases in "$work"/cases/*.json; do
    family="$(basename "$cases" .json)"
    mkdir -p "$work/expected/$family"
    php -n "$root/oracle/php/xml/run_cases.php" "$cases" "$work/expected/$family" > "$work/expected/$family.json"
done
php -n "$root/oracle/php/xml/filters.php" "$work/filters/inputs.json" > "$work/filters/expected.json"
# Identidad CPEForge (URI de la firma, nombres de clase): oracle/identidad.py;
# después se recalcula el sha256 de cada XML traducido.
python3 "$root/oracle/identidad.py" "$work"
python3 "$root/oracle/identidad.py" --sha256 "$work/expected"

# El instante de render (`now`) depende del reloj. Para comprobar que el
# oráculo es reproducible se compara todo salvo ese campo, y en los XML se
# sustituyen solo la fecha y hora de Lima derivadas del `now` de cada ejecución
# (las plantillas imprimen la hora actual cuando una fecha es nula). La
# comparación PHP↔Rust de crates/xml/tests/parity.rs usa el `now` exacto.
normalize_now() {
    python3 - "$1" <<'PY'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
# Metadatos del entorno (versión de PHP…), como scripts/fixture_diff.py.
for field in ("reference_version", "timezone_db", "openssl", "libxml"):
    if field in data:
        data[field] = "<entorno>"
for result in data["results"].values():
    result["now"] = 0
    result.pop("sha256", None)  # el XML se compara aparte, normalizado
print(json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True))
PY
}

if $update; then
    rm -rf "$ref_dir/cases" "$ref_dir/expected" "$ref_dir/filters"
    cp -R "$work/cases" "$work/expected" "$work/filters" "$ref_dir/"
    (cd "$ref_dir" && find cases expected filters -type f | sort | xargs shasum -a 256 > SHA256SUMS)
    echo "fixtures actualizados; revisar el diff antes de aceptarlos"
fi

status=0
diff -ru "$ref_dir/cases" "$work/cases" || status=1
diff -u "$ref_dir/filters/inputs.json" "$work/filters/inputs.json" || status=1
python3 "$root/scripts/fixture_diff.py" "$ref_dir/filters/expected.json" "$work/filters/expected.json" || status=1
for family_json in "$work"/expected/*.json; do
    family="$(basename "$family_json" .json)"
    diff -u <(normalize_now "$ref_dir/expected/$family.json") <(normalize_now "$family_json") || status=1
    python3 - "$ref_dir/expected/$family" "$ref_dir/expected/$family.json" "$work/expected/$family" "$family_json" <<'PY' || status=1
import json, sys
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

def stamps(now):
    lima = datetime.fromtimestamp(now, ZoneInfo("America/Lima"))
    return lima.strftime(">%Y-%m-%d<"), lima.strftime(">%H:%M:%S<")

def normalized(directory, summary, case_id):
    text = (Path(directory) / f"{case_id}.xml").read_bytes().decode("utf-8", "surrogateescape")
    date, time = stamps(json.load(open(summary))["results"][case_id]["now"])
    return text.replace(date, ">NOW-DATE<").replace(time, ">NOW-TIME<")

ref_dir, ref_json, new_dir, new_json = sys.argv[1:]
ref_ids = sorted(p.stem for p in Path(ref_dir).glob("*.xml"))
new_ids = sorted(p.stem for p in Path(new_dir).glob("*.xml"))
failed = ref_ids != new_ids
for case_id in new_ids:
    if case_id in ref_ids and normalized(ref_dir, ref_json, case_id) != normalized(new_dir, new_json, case_id):
        print(f"XML distinto: {new_dir}/{case_id}.xml")
        failed = True
sys.exit(1 if failed else 0)
PY
done
(cd "$ref_dir" && shasum -a 256 -c SHA256SUMS --quiet) || status=1
if [[ $status -ne 0 ]]; then
    echo "FALLO: los fixtures versionados no se reproducen desde la referencia PHP" >&2
    exit 1
fi
echo "fixtures PHP reproducidos: OK"

cd "$root"
# tests/schema.rs valida contra los XSD UBL 2.3.0; aquí usa los que instaló
# composer para el oráculo (mismo contenido que target/reference/ubl-validator-v2.3.0).
export CPEFORGE_XSD_DIR="${CPEFORGE_XSD_DIR:-$root/target/php-oracle-vendor/greenter/ubl-validator/src/xsd}"
cargo test -p cpeforge-xml --all-features --locked -- --nocapture
