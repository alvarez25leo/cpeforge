#!/usr/bin/env bash
# Oráculo de cpeforge (fachadas See y Api): PHP real y Rust sobre entradas idénticas y red PHP bloqueada.
set -euo pipefail

cpeforge_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if command -v sandbox-exec >/dev/null; then
  cpeforge_php=(sandbox-exec -p '(version 1)(allow default)(deny network*)')
elif [[ "${CPEFORGE_NETWORK_BLOCKED:-}" == 1 ]]; then
  sudo -n iptables -C OUTPUT ! -o lo -j REJECT
  sudo -n ip6tables -C OUTPUT ! -o lo -j REJECT
  cpeforge_php=()
else
  echo 'Falta bloqueo de red verificable para PHP' >&2
  exit 2
fi
cpeforge_work="$(mktemp -d)"
source "$cpeforge_root/scripts/evidence.sh"
cleanup() {
  # Sin keys/: salida PHP, XML firmados por PHP, verificación y diferencias.
  save_evidence cpeforge-regenerated "$cpeforge_work/php.json" "$cpeforge_work/signed" "$cpeforge_work/verify.json" "$cpeforge_work/differences.json"
  python3 - "$cpeforge_work" <<'PY'
import shutil, sys
shutil.rmtree(sys.argv[1])
PY
}
trap cleanup EXIT
chmod 700 "$cpeforge_work"

cd "$cpeforge_root"
test "$(git -C target/reference/v5.3.0 rev-parse HEAD)" = 'f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f'
scripts/lite_upstream_safe.sh
scripts/cpeforge_local_contract.sh
if ! "${cpeforge_php[@]}" php -r 'exit(@fsockopen("192.0.2.1", 80, $n, $s, 1) ? 1 : 0);'; then
  echo 'El sandbox PHP permitió red; oráculo cancelado' >&2
  exit 2
fi

scripts/xmldsig_keys.sh "$cpeforge_work/keys"
"${cpeforge_php[@]}" \
  php -d log_errors=0 oracle/php/lite/run_cases.php "$cpeforge_work/keys" > "$cpeforge_work/php.json"
# Identidad CPEForge en los nombres de clase de los resultados: oracle/identidad.py.
python3 oracle/identidad.py "$cpeforge_work/php.json"

mkdir "$cpeforge_work/signed"
python3 - "$cpeforge_work/php.json" "$cpeforge_work/signed" <<'PY'
import json, pathlib, sys
cases = json.load(open(sys.argv[1]))
out = pathlib.Path(sys.argv[2])
(out / 'invoice.xml').write_text(cases['api_xml_after_send'])
(out / 'summary.xml').write_text(cases['see_signed_summary'])
PY

xsd='target/php-oracle-vendor/greenter/ubl-validator/src/xsd'
xmllint --noout --nonet --schema "$xsd/2.1/maindoc/UBL-Invoice-2.1.xsd" "$cpeforge_work/signed/invoice.xml"
xmllint --noout --nonet --schema "$xsd/2.0/maindoc/UBL-SummaryDocuments-2.0.xsd" "$cpeforge_work/signed/summary.xml"
python3 scripts/xmldsig_independent_verify.py "$cpeforge_work/signed" \
  "$cpeforge_work/keys/a.crt" "$cpeforge_work/keys/b.crt" "$cpeforge_work/verify.json"

export CPEFORGE_REFERENCE_OUT="$cpeforge_work/php.json" CPEFORGE_KEYS_DIR="$cpeforge_work/keys" CPEFORGE_DIFF_OUT="$cpeforge_work/differences.json"
set +e
cargo test -p cpeforge --test live_parity live_reference_parity -- --ignored --nocapture
cpeforge_result=$?
set -e
if [[ -f "$cpeforge_work/differences.json" ]]; then
  python3 - "$cpeforge_work/differences.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
print('Diferencias sin normalizar:', ', '.join(d) if d else 'ninguna')
PY
fi
test "$cpeforge_result" -eq 0
