#!/usr/bin/env bash
# Gate de paridad de cpeforge-core contra la referencia PHP fijada.
#
# 1. Prepara la referencia PHP aislada (scripts/prepare_php_reference.sh).
# 2. Regenera inventario, casos y salidas PHP en un directorio temporal y exige
#    que coincidan byte a byte con los fixtures versionados: el oráculo debe ser
#    reproducible y nadie puede editar las expectativas a mano.
# 3. Verifica los hashes de tests/reference/core/SHA256SUMS.
# 4. Ejecuta las pruebas Rust de paridad, inventario, límites y API pública.
#
# Uso: scripts/core_parity.sh [--update]  (--update reescribe los fixtures y hashes)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ref_dir="$root/tests/reference/core"
update=false
[[ "${1:-}" == "--update" ]] && update=true

GREENTER_PHP_REFERENCE="$("$root/scripts/prepare_php_reference.sh")"
export GREENTER_PHP_REFERENCE
echo "referencia PHP: $GREENTER_PHP_REFERENCE ($(git -C "$GREENTER_PHP_REFERENCE" rev-parse HEAD))"
php --version | head -1

work="$(mktemp -d)"
source "$root/scripts/evidence.sh"
trap 'save_evidence core-regenerated "$work/inventory.json" "$work/cases" "$work/expected" "$work/int-cast"; rm -rf "$work"' EXIT
mkdir -p "$work/cases" "$work/expected" "$work/int-cast"

php "$root/oracle/php/core/inventory.php" > "$work/inventory.json"
python3 "$root/oracle/cases/core/generate_cases.py" "$work/cases"
for cases in "$work"/cases/*.json; do
    php "$root/oracle/php/core/run_cases.php" "$cases" > "$work/expected/$(basename "$cases")"
done
php "$root/oracle/php/core/int_cast.php" "$work/int-cast/inputs.json" > "$work/int-cast/expected.json"
# Identidad CPEForge (nombres de clase, firma, User-Agent): oracle/identidad.py.
python3 "$root/oracle/identidad.py" "$work"

if $update; then
    cp "$work/inventory.json" "$ref_dir/inventory.json"
    rm -f "$ref_dir"/cases/*.json "$ref_dir"/expected/*.json
    cp "$work"/cases/*.json "$ref_dir/cases/"
    cp "$work"/expected/*.json "$ref_dir/expected/"
    cp "$work"/int-cast/inputs.json "$work"/int-cast/expected.json "$ref_dir/int-cast/"
    (cd "$ref_dir" && find . -name '*.json' ! -name differences.json ! -name api-map.json | sort | xargs shasum -a 256 > SHA256SUMS)
    echo "fixtures actualizados; revisar el diff antes de aceptarlos"
fi

status=0
# Campos de entorno (versión de PHP, tzdb) no cuentan: scripts/fixture_diff.py.
python3 "$root/scripts/fixture_diff.py" "$ref_dir/inventory.json" "$work/inventory.json" || status=1
diff -ru "$ref_dir/cases" "$work/cases" || status=1
python3 "$root/scripts/fixture_diff.py" "$ref_dir/expected" "$work/expected" || status=1
diff -u "$ref_dir/int-cast/inputs.json" "$work/int-cast/inputs.json" || status=1
python3 "$root/scripts/fixture_diff.py" "$ref_dir/int-cast/expected.json" "$work/int-cast/expected.json" || status=1
(cd "$ref_dir" && shasum -a 256 -c SHA256SUMS --quiet) || status=1
if [[ $status -ne 0 ]]; then
    echo "FALLO: los fixtures versionados no se reproducen desde la referencia PHP" >&2
    exit 1
fi
echo "fixtures PHP reproducidos: OK"

cd "$root"
cargo test -p cpeforge-core --all-features --locked -- --nocapture
