#!/usr/bin/env bash
# Contrato HTTP independiente de cpeforge; solo loopback, ningún destino externo.
set -euo pipefail
cpeforge_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$cpeforge_root"
if command -v sandbox-exec >/dev/null; then
  cpeforge_run=(sandbox-exec -p '(version 1)(allow default)(deny network*)(allow network-outbound (remote ip "localhost:*"))')
elif [[ "${CPEFORGE_NETWORK_BLOCKED:-}" == 1 ]]; then
  sudo -n iptables -C OUTPUT ! -o lo -j REJECT
  sudo -n ip6tables -C OUTPUT ! -o lo -j REJECT
  cpeforge_run=()
else
  echo 'Falta bloqueo de egress verificable' >&2
  exit 2
fi
cpeforge_work="$(mktemp -d)"
cpeforge_server_pid=''
cleanup() {
  if [[ -n "$cpeforge_server_pid" ]]; then
    kill "$cpeforge_server_pid" 2>/dev/null || true
    wait "$cpeforge_server_pid" 2>/dev/null || true
  fi
  python3 - "$cpeforge_work" <<'PY'
import shutil, sys
shutil.rmtree(sys.argv[1])
PY
}
trap cleanup EXIT
chmod 700 "$cpeforge_work"
python3 oracle/cpeforge/mock_http_server.py "$cpeforge_work/port" "$cpeforge_work/requests.jsonl" \
  target/reference/v5.3.0/packages/ws/src/Resources/wsdl/billService.wsdl &
cpeforge_server_pid="$!"
for _ in {1..100}; do
  [[ -s "$cpeforge_work/port" ]] && break
  kill -0 "$cpeforge_server_pid"
  sleep 0.05
done
test -s "$cpeforge_work/port"
export CPEFORGE_LOCAL_URL="http://127.0.0.1:$(cat "$cpeforge_work/port")"
"${cpeforge_run[@]}" cargo test -p cpeforge --test local_contract --offline --locked \
  soap_and_gre_local_contract -- --ignored --nocapture
python3 - "$cpeforge_work/requests.jsonl" <<'PY'
import json, sys
records = [json.loads(line) for line in open(sys.argv[1])]
kinds = [r['kind'] for r in records]
expected = ['soap-sendSummary', 'soap-getStatus', 'gre-auth', 'gre-send', 'gre-status']
if kinds != expected:
    raise SystemExit(f'operaciones HTTP inesperadas: {kinds!r}')
bad = [r for r in records if r['observations']]
if bad:
    raise SystemExit(f'contrato HTTP inválido: {bad!r}')
print('servidor independiente: 5/5 solicitudes SOAP/GRE válidas contra WSDL, ZIP y JSON')
PY
