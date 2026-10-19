#!/usr/bin/env bash
# Referencia v5.3.0 y SOAP confinados a 127.0.0.1. No ejecuta PHPUnit.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
reference="$(bash scripts/prepare_php_reference.sh)"
suite="$root/target/reference/v5.3.0-suite"
PHP_REFERENCE_DIR="$suite" bash scripts/prepare_php_reference.sh >/dev/null
test "$(git -C "$reference" rev-parse HEAD)" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
test "$(git -C "$reference" rev-parse v5.3.0)" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
test "$(git -C "$suite" rev-parse HEAD)" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
if ! test -f "$suite/vendor/autoload.php"; then
  cp oracle/php/ws/composer.lock "$suite/composer.lock"
  composer install --working-dir="$suite" --no-interaction --prefer-dist --no-progress --ignore-platform-req=php
fi

source scripts/evidence.sh
scratch="$(mktemp -d)"
server_pid=""
cleanup() {
  if test -n "$server_pid"; then kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; fi
  save_evidence ws-oracle "$scratch/expected.json" "$scratch/contract.json" "$scratch/soap.jsonl"
  rm -rf "$scratch"
}
trap cleanup EXIT

python3 oracle/cases/ws/generate_cases.py "$scratch/cases.json"
python3 oracle/identidad.py "$scratch/cases.json"
cmp "$scratch/cases.json" tests/reference/ws/cases.json
python3 oracle/ws/mock_soap_server.py tests/reference/ws/cases.json \
  "$reference/packages/ws/src/Resources/wsdl" "$scratch/port" "$scratch/soap.jsonl" &
server_pid="$!"
for _ in {1..100}; do
  test -s "$scratch/port" && break
  kill -0 "$server_pid"
  sleep 0.05
done
test -s "$scratch/port"
WS_PHP_REFERENCE="$reference" WS_SOAP_URL="http://127.0.0.1:$(cat "$scratch/port")" \
  php oracle/php/ws/run_cases.php tests/reference/ws/cases.json > "$scratch/expected.json"
# Identidad CPEForge (nombres de clase): oracle/identidad.py.
python3 oracle/identidad.py "$scratch/expected.json"
python3 - "$scratch/soap.jsonl" "$scratch/contract.json" <<'PY'
import json, sys
records = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
with open(sys.argv[2], "w", encoding="utf-8") as file:
    json.dump(records, file, ensure_ascii=False, indent=2)
    file.write("\n")
if len(records) != 5:
    raise SystemExit(f"se esperaban cinco solicitudes SOAP locales, se observaron {len(records)}")
for record in records:
    if record["observations"]:
        raise SystemExit(f"contrato SOAP inválido en {record['id']}: {record['observations']}")
PY
if test "${WS_UPDATE:-0}" = 1; then
  cp "$scratch/expected.json" tests/reference/ws/expected.json
  cp "$scratch/contract.json" tests/reference/ws/contract.json
else
  # Campos de entorno (versión de PHP) no cuentan: scripts/fixture_diff.py.
  python3 scripts/fixture_diff.py tests/reference/ws/expected.json "$scratch/expected.json"
  cmp "$scratch/contract.json" tests/reference/ws/contract.json
fi
python3 oracle/ws/verify_independent.py tests/reference/ws/cases.json \
  tests/reference/ws/expected.json "$reference/packages/ws/src/Resources/wsdl"
echo 'PHP ws, WSDL/XSD y SOAP loopback: reproducidos'
if test "${WS_RUN_RUST:-0}" = 1; then
  WS_SOAP_URL="http://127.0.0.1:$(cat "$scratch/port")" WS_SOAP_LOG="$scratch/soap.jsonl" \
    cargo test -p cpeforge-ws --test parity live_parity --locked -- --ignored --nocapture
fi
