#!/usr/bin/env bash
# BillSender PHP (ext/soap real) y Rust contra el mismo servidor HTTP en
# loopback: estados >= 400 y CDR con entrada de directorio, como SUNAT BETA.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
reference="$(bash scripts/prepare_php_reference.sh)"
suite="$root/target/reference/v5.3.0-suite"
PHP_REFERENCE_DIR="$suite" bash scripts/prepare_php_reference.sh >/dev/null
test "$(git -C "$reference" rev-parse HEAD)" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
if ! test -f "$suite/vendor/autoload.php"; then
  cp oracle/php/ws/composer.lock "$suite/composer.lock"
  composer install --working-dir="$suite" --no-interaction --prefer-dist --no-progress --ignore-platform-req=php
fi
source scripts/evidence.sh
scratch="$(mktemp -d)"
server_pid=""
cleanup() {
  if test -n "$server_pid"; then kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; fi
  save_evidence ws-http-probe "$scratch/php.json" "$scratch/requests"
  rm -rf "$scratch"
}
trap cleanup EXIT
python3 oracle/ws/http_status_server.py tests/reference/ws/cases.json "$scratch/port" "$scratch/requests" &
server_pid="$!"
for _ in {1..100}; do
  test -s "$scratch/port" && break
  kill -0 "$server_pid"
  sleep 0.05
done
test -s "$scratch/port"
export WS_HTTP_URL="http://127.0.0.1:$(cat "$scratch/port")"
# Todos los casos del servidor, en su orden (sin espacios en los nombres).
cases=($(python3 -c '
import ast
tree = ast.parse(open("oracle/ws/http_status_server.py", encoding="utf-8").read())
for node in tree.body:
    if isinstance(node, ast.Assign) and getattr(node.targets[0], "id", "") == "RESPONSES":
        print("\n".join(key.value for key in node.value.keys))
'))
test "${#cases[@]}" -gt 0
php oracle/php/ws/http_probe.php "${cases[@]}" > "$scratch/php.json"
# Pila amplia para los hilos de test: el caso fault-detail-depth-256 está en el
# límite admitido y, en debug, roxmltree usa unos 17 KB de pila por nivel.
WS_HTTP_EXPECTED="$scratch/php.json" RUST_MIN_STACK=67108864 cargo test -p cpeforge-ws --test security \
  http_status_probe_matches_reference --locked -- --ignored --exact
# Cada caso llegó una vez desde PHP y otra desde Rust.
test "$(wc -l < "$scratch/requests" | tr -d ' ')" = "$((2 * ${#cases[@]}))"
echo "HTTP loopback ($(python3 -c 'import json,sys;d=json.load(open(sys.argv[1]));print("libxml2", d["libxml"], "CDATA", d["libxml_cdata"])' "$scratch/php.json")): ${#cases[@]} casos PHP (ext/soap) y Rust; iguales salvo las diferencias registradas del perfil seguro"
