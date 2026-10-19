#!/usr/bin/env bash
# Demuestra con certificado efímero que PHP acepta TLS no confiable y Rust no.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
test "$(git -C target/reference/v5.3.0 rev-parse HEAD)" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
test -f target/reference/v5.3.0-suite/vendor/autoload.php
source scripts/evidence.sh
scratch="$(mktemp -d)"
server_pid=""
cleanup() {
  if test -n "$server_pid"; then kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; fi
  save_evidence ws-tls-probe "$scratch/php.json" "$scratch/requests"  # sin key.pem ni cert.pem
  rm -rf "$scratch"
}
trap cleanup EXIT
openssl req -x509 -nodes -newkey rsa:2048 -keyout "$scratch/key.pem" -out "$scratch/cert.pem" \
  -days 1 -subj '/CN=localhost' -addext 'subjectAltName=DNS:localhost' >/dev/null 2>&1
python3 oracle/ws/tls_server.py "$scratch/cert.pem" "$scratch/key.pem" "$scratch/port" "$scratch/requests" &
server_pid="$!"
for _ in {1..100}; do
  test -s "$scratch/port" && break
  kill -0 "$server_pid"
  sleep 0.05
done
test -s "$scratch/port"
export WS_TLS_URL="https://127.0.0.1:$(cat "$scratch/port")/"
php oracle/php/ws/tls_probe.php > "$scratch/php.json"
python3 - "$scratch/php.json" <<'PY'
import json, sys
assert json.load(open(sys.argv[1], encoding="utf-8")) == {"code": "98"}
PY
cargo test -p cpeforge-ws --test security untrusted_tls_is_rejected --locked -- --ignored
test "$(wc -l < "$scratch/requests" | tr -d ' ')" = 1
echo 'TLS loopback: PHP aceptó certificado no confiable; Rust lo rechazó'
