#!/usr/bin/env bash
# Oráculo PHP fijado y servidor local independiente; no llama a SUNAT.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

reference="$(bash scripts/prepare_gre_api_reference.sh)"
expected_commit="81627a3511b98cba7243ee3a60cc66ff4a688f51"
test "$(git -C "$reference" rev-parse HEAD)" = "$expected_commit"
test "$(git -C "$reference" rev-parse v1.0.2)" = "$expected_commit"
if ! test -f "$root/target/php-oracle-vendor/autoload.php"; then
  composer install --working-dir=oracle/php --no-interaction --prefer-dist --no-progress
fi

source scripts/evidence.sh
scratch="$(mktemp -d)"
server_pid=""
cleanup() {
  if test -n "$server_pid"; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  save_evidence gre-api-regenerated "$scratch/cases.json" "$scratch/php.json"
  rm -rf "$scratch"
}
trap cleanup EXIT

python3 oracle/cases/gre_api/generate_cases.py "$scratch/cases.json"
python3 oracle/identidad.py "$scratch/cases.json"
cmp "$scratch/cases.json" tests/reference/gre-api/cases.json
ruby -ryaml -rjson -e 'puts JSON.generate(YAML.load_file(ARGV.fetch(0)))' \
  "$reference/openapi.yaml" > "$scratch/openapi.json"
python3 oracle/gre_api/mock_server.py \
  tests/reference/gre-api/cases.json "$scratch/openapi.json" "$scratch/port" &
server_pid="$!"
for _ in {1..100}; do
  test -s "$scratch/port" && break
  kill -0 "$server_pid"
  sleep 0.05
done
test -s "$scratch/port"
base="http://127.0.0.1:$(cat "$scratch/port")"

GRE_API_PHP_REFERENCE="$reference" php oracle/php/gre_api/run_cases.php \
  tests/reference/gre-api/cases.json "$base" > "$scratch/php.json"
# Identidad CPEForge (User-Agent, nombres de clase): oracle/identidad.py.
python3 oracle/identidad.py "$scratch/php.json"
python3 - "$scratch/openapi.json" "$scratch/php.json" <<'PY'
import json
import sys

openapi = json.load(open(sys.argv[1], encoding="utf-8"))
php = json.load(open(sys.argv[2], encoding="utf-8"))["results"]["config-defaults"]["result"]
openapi_cpe = openapi["servers"][1]["url"]
php_cpe = php["host_settings"][1]["url"]
assert openapi_cpe == "https://api.sunat.gob.pe/v1"
assert php_cpe == "https://api-cpe.sunat.gob.pe/v1"
assert openapi_cpe != php_cpe
assert php["cpe_api_host"] == php["host"] == "https://api-seguridad.sunat.gob.pe/v1"
print("hosts: OpenAPI CPE difiere de Configuration PHP; CpeApi sin configurar usa host auth")
PY
GRE_API_MOCK_URL="$base" GRE_API_REFERENCE_OUT="$scratch/php.json" \
  cargo test -p cpeforge-gre-api --test parity live_parity --locked -- --ignored --nocapture
