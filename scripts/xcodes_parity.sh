#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

reference=target/reference/v5.3.0
suite=target/reference/v5.3.0-suite
test "$(git -C "$reference" rev-parse HEAD)" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
test -z "$(git -C greenter-php status --porcelain)"
test "$(git -C "$suite" rev-parse HEAD)" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
test -f "$suite/vendor/bin/phpunit"
mkdir -p target/evidence
profile='(version 1)(allow default)(deny network*)'
run_offline() {
  if [[ "${XCODES_NETWORK_BLOCKED:-}" = 1 ]]; then
    "$@"
  elif command -v sandbox-exec >/dev/null 2>&1; then
    sandbox-exec -p "$profile" "$@"
  else
    echo 'Se requiere sandbox-exec o XCODES_NETWORK_BLOCKED=1 con firewall externo' >&2
    return 2
  fi
}

# La única clase de prueba xcodes no realiza IO de red: constructor, getAll y
# 14 consultas conocidas/desconocidas. Se ejecuta en el checkout aislado.
(cd "$suite" && run_offline php vendor/bin/phpunit packages/xcodes/tests --no-coverage)
run_offline php oracle/php/xcodes/oracle.php > target/evidence/xcodes-php-regenerated.json
# Identidad CPEForge en los textos del oráculo: oracle/identidad.py.
python3 oracle/identidad.py target/evidence/xcodes-php-regenerated.json
cmp tests/reference/xcodes/expected.json target/evidence/xcodes-php-regenerated.json
python3 scripts/check_xcodes_xml.py
run_offline cargo test -p cpeforge-xcodes --all-features --locked --offline
run_offline cargo test -p cpeforge --features xcodes --test xcodes --locked --offline
