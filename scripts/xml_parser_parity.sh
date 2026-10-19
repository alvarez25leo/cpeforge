#!/usr/bin/env bash
# Puertas de paridad de cpeforge-xml-parser frente a greenter/xml-parser
# v5.3.0 (thegreenter/greenter f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f).
# Todo se ejecuta sin red (sandbox-exec en macOS o XML_PARSER_NETWORK_BLOCKED=1
# con un cortafuegos externo). Las evidencias quedan en target/evidence/.
set -euo pipefail
cd "$(dirname "$0")/.."

reference=target/reference/v5.3.0
suite=target/reference/v5.3.0-suite
commit=f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
test "$(git -C "$reference" rev-parse HEAD)" = "$commit"
test -z "$(git -C "$reference" status --porcelain)"
test -z "$(git -C greenter-php status --porcelain)"
test "$(git -C "$suite" rev-parse HEAD)" = "$commit"
test -f "$suite/vendor/bin/phpunit"
evidence=target/evidence
mkdir -p "$evidence"
profile='(version 1)(allow default)(deny network*)'
run_offline() {
  if [[ "${XML_PARSER_NETWORK_BLOCKED:-}" = 1 ]]; then
    "$@"
  elif command -v sandbox-exec >/dev/null 2>&1; then
    sandbox-exec -p "$profile" "$@"
  else
    echo 'Se requiere sandbox-exec o XML_PARSER_NETWORK_BLOCKED=1 con cortafuegos externo' >&2
    return 2
  fi
}

# 1. Auditoría de los 70 XML upstream ANTES de su suite: sin contenido, solo
#    agregados. Ninguno se copia a fixtures.
python3 -I scripts/audit_xml_parser_resources.py \
  "$suite/packages/xml-parser/tests/Resources" "$evidence/xml-parser-audit.json"
python3 -I - "$evidence/xml-parser-audit.json" <<'PY'
import json, sys
totals = json.load(open(sys.argv[1]))["totals"]
assert totals["files"] == 70, totals
assert totals.get("doctype", 0) == 0 and totals.get("entity_decl", 0) == 0, totals
PY

# 2. Suite upstream de xml-parser sin red, en el checkout aislado.
(cd "$suite" && run_offline php vendor/bin/phpunit packages/xml-parser/tests --no-coverage) \
  | tee "$evidence/xml-parser-suite-phpunit.log"

# 3. Casos sintéticos e inventario reproducibles.
python3 -I oracle/cases/xml_parser/generate_cases.py "$evidence/xml-parser-cases.json"
python3 -I oracle/identidad.py "$evidence/xml-parser-cases.json"
cmp tests/reference/xml-parser/cases.json "$evidence/xml-parser-cases.json"
python3 -I oracle/cases/xml_parser/inventory.py "$reference/packages/xml-parser/src" \
  "$evidence/xml-parser-inventory.json"
python3 -I oracle/identidad.py "$evidence/xml-parser-inventory.json"
cmp tests/reference/xml-parser/inventory.json "$evidence/xml-parser-inventory.json"
(cd tests/reference/xml-parser && shasum -a 256 -c SHA256SUMS)

# 4. Oráculo PHP regenerado sin red; se ignoran solo los valores del reloj.
run_offline php oracle/php/xml_parser/run_cases.php tests/reference/xml-parser/cases.json \
  > "$evidence/xml-parser-php-regenerated.json"
# Identidad CPEForge (nombres de clase): oracle/identidad.py.
python3 -I oracle/identidad.py "$evidence/xml-parser-php-regenerated.json"
python3 -I scripts/xml_parser_compare_oracle.py tests/reference/xml-parser/expected.json \
  "$evidence/xml-parser-php-regenerated.json"

# 5. Rust: paridad campo a campo, generación→parseo, hostiles, unitarias y doc.
XML_PARSER_RUST_OUT="$PWD/$evidence/xml-parser-rust.json" \
  run_offline cargo test -p cpeforge-xml-parser --all-features --locked --offline
run_offline cargo run -q -p cpeforge-xml-parser --example parse_invoice --locked --offline
run_offline cargo run -q -p cpeforge-xml-parser --example reader --locked --offline

# 6. Verificación independiente (expat) de raíz, namespaces y campos críticos.
python3 -I scripts/xml_parser_independent_verify.py tests/reference/xml-parser/cases.json \
  tests/reference/xml-parser/expected.json "$evidence/xml-parser-rust.json"
echo "xml-parser: puertas de paridad completadas"
