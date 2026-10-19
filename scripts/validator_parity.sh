#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

reference=target/reference/v5.3.0
commit=$(git -C "$reference" rev-parse HEAD)
test "$commit" = f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
test -z "$(git -C greenter-php status --porcelain)"
mkdir -p target/evidence
profile='(version 1)(allow default)(deny network*)'
run_php() {
  if [[ "${VALIDATOR_NETWORK_BLOCKED:-}" = 1 ]]; then
    php "$@"
  elif command -v sandbox-exec >/dev/null 2>&1; then
    sandbox-exec -p "$profile" php "$@"
  else
    echo 'Se requiere sandbox-exec o VALIDATOR_NETWORK_BLOCKED=1 con firewall externo' >&2
    return 2
  fi
}
run_offline() {
  if [[ "${VALIDATOR_NETWORK_BLOCKED:-}" = 1 ]]; then
    "$@"
  elif command -v sandbox-exec >/dev/null 2>&1; then
    sandbox-exec -p "$profile" "$@"
  else
    echo 'Se requiere sandbox-exec o VALIDATOR_NETWORK_BLOCKED=1 con firewall externo' >&2
    return 2
  fi
}

run_php oracle/php/validator/inventory.php > target/evidence/validator-inventory-regenerated.json
# Identidad CPEForge (nombres de clase) en cada salida PHP: oracle/identidad.py.
python3 oracle/identidad.py target/evidence/validator-inventory-regenerated.json
python3 -m json.tool target/evidence/validator-inventory-regenerated.json > /dev/null
python3 scripts/verify_validator_snapshot.py tests/reference/validator/inventory.json target/evidence/validator-inventory-regenerated.json

python3 scripts/generate_validator_cases.py --output target/evidence/validator-cases-regenerated.json
python3 oracle/identidad.py target/evidence/validator-cases-regenerated.json
python3 scripts/verify_validator_snapshot.py tests/reference/validator/cases.json target/evidence/validator-cases-regenerated.json
python3 scripts/build_validator_rules.py --output target/evidence/validator-rules-regenerated.json
python3 scripts/verify_validator_snapshot.py crates/validator/src/rules.json target/evidence/validator-rules-regenerated.json
python3 scripts/build_validator_setters.py --output target/evidence/validator-setters-regenerated.json
python3 scripts/verify_validator_snapshot.py crates/validator/src/setters.json target/evidence/validator-setters-regenerated.json

run_php oracle/php/validator/run_cases.php tests/reference/validator/cases.json > target/evidence/validator-php-regenerated.json
python3 oracle/identidad.py target/evidence/validator-php-regenerated.json
python3 scripts/verify_validator_snapshot.py tests/reference/validator/reference-results.json target/evidence/validator-php-regenerated.json
run_php -d 'error_reporting=E_ALL&~E_DEPRECATED' oracle/php/validator/probe_api.php > target/evidence/validator-api-regenerated.json
python3 oracle/identidad.py target/evidence/validator-api-regenerated.json
python3 scripts/verify_validator_snapshot.py tests/reference/validator/api-probe.json target/evidence/validator-api-regenerated.json

python3 scripts/check_validator_rules.py
python3 scripts/build_validator_matrix.py --output target/evidence/validator-matrix-regenerated.md
cmp oracle/evidencia/validator-0.1.0-matriz.md target/evidence/validator-matrix-regenerated.md
cargo build --locked -q -p cpeforge-validator --bin validator_oracle
run_offline target/debug/validator_oracle tests/reference/validator/cases.json > target/evidence/validator-rust.json
python3 scripts/compare_validator.py tests/reference/validator/cases.json target/evidence/validator-php-regenerated.json target/evidence/validator-rust.json
