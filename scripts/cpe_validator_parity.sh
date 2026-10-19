#!/usr/bin/env bash
# Puertas de paridad de cpeforge-cpe-validator frente a greenter/cpe-validator
# v5.3.0 (thegreenter/greenter f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f).
# Todo se ejecuta sin red (sandbox-exec en macOS o CPE_VALIDATOR_NETWORK_BLOCKED=1
# con un cortafuegos externo). Las evidencias quedan en target/evidence/.
# Requiere php (ext-dom, ext-xsl), xsltproc, python3 y libxslt de desarrollo.
set -euo pipefail
cd "$(dirname "$0")/.."

reference=target/reference/v5.3.0
suite=target/reference/v5.3.0-suite
commit=f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f
ref=tests/reference/cpe-validator
test "$(git -C "$reference" rev-parse HEAD)" = "$commit"
test -z "$(git -C "$reference" status --porcelain)"
test -z "$(git -C greenter-php status --porcelain)"
test "$(git -C "$suite" rev-parse HEAD)" = "$commit"
test -f "$suite/vendor/bin/phpunit"
command -v xsltproc >/dev/null
php -r 'exit(extension_loaded("xsl") && extension_loaded("dom") ? 0 : 1);'
evidence=target/evidence
mkdir -p "$evidence"
profile='(version 1)(allow default)(deny network*)'
run_offline() {
  if [[ "${CPE_VALIDATOR_NETWORK_BLOCKED:-}" = 1 ]]; then
    "$@"
  elif command -v sandbox-exec >/dev/null 2>&1; then
    sandbox-exec -p "$profile" "$@"
  else
    echo 'Se requiere sandbox-exec o CPE_VALIDATOR_NETWORK_BLOCKED=1 con cortafuegos externo' >&2
    return 2
  fi
}

# 1. Entorno del oráculo.
php -r 'printf("PHP %s libxml %s libxslt %s libexslt %s PCRE %s pcre.jit=%s\n", PHP_VERSION, LIBXML_DOTTED_VERSION, LIBXSLT_DOTTED_VERSION, LIBEXSLT_DOTTED_VERSION, PCRE_VERSION, ini_get("pcre.jit"));' \
  | tee "$evidence/cpe-validator-env.txt"
xsltproc --version | head -1 | tee -a "$evidence/cpe-validator-env.txt"

# 2. Suite upstream de cpe-validator sin red, en el checkout aislado. Sus XML
#    de prueba (packages/xml-parser/tests/Resources) no se copian.
(cd "$suite" && run_offline php vendor/bin/phpunit packages/cpe-validator/tests --no-coverage) \
  | tee "$evidence/cpe-validator-suite-phpunit.log"

# 3. Casos sintéticos e inventario reproducibles (14 archivos, 10 rutas).
python3 -I oracle/cases/cpe_validator/generate_cases.py "$evidence/cpe-validator-cases.json"
python3 -I oracle/identidad.py "$evidence/cpe-validator-cases.json"
cmp "$ref/cases.json" "$evidence/cpe-validator-cases.json"
python3 -I oracle/cases/cpe_validator/inventory.py "$reference/packages/cpe-validator" \
  "$evidence/cpe-validator-inventory.json"
python3 -I oracle/identidad.py "$evidence/cpe-validator-inventory.json"
cmp "$ref/inventory.json" "$evidence/cpe-validator-inventory.json"
(cd "$ref" && shasum -a 256 -c SHA256SUMS)

# 4. Oráculo PHP regenerado sin red; se comparan todos los pasos. Los avisos
#    (texto de libxml/PHP) se conservan como artefacto y no se comparan.
#    Las hojas de los casos llaman a la función `matches` con el nombre de
#    CPEForge; la referencia la registra con el suyo (oracle/identidad.py
#    --inversa) y su salida vuelve a la identidad CPEForge.
cp "$ref/cases.json" "$evidence/cpe-validator-cases-reference.json"
python3 -I oracle/identidad.py --inversa "$evidence/cpe-validator-cases-reference.json"
run_offline php oracle/php/cpe_validator/run_cases.php "$evidence/cpe-validator-cases-reference.json" \
  > "$evidence/cpe-validator-php-regenerated.json"
python3 -I oracle/identidad.py "$evidence/cpe-validator-php-regenerated.json"
python3 -I - "$ref/expected.json" "$evidence/cpe-validator-php-regenerated.json" <<'PY'
import json, sys
fixture = json.load(open(sys.argv[1]))["results"]
fresh = json.load(open(sys.argv[2]))["results"]
assert fixture.keys() == fresh.keys(), "casos distintos"
diff = [cid for cid in fixture if fixture[cid].get("steps") != fresh[cid].get("steps") or "harness_error" in fresh[cid]]
for cid in diff:
    print("DIFIERE", cid, json.dumps(fixture[cid].get("steps"), ensure_ascii=False)[:300], "≠", json.dumps(fresh[cid].get("steps"), ensure_ascii=False)[:300])
print(f"oráculo regenerado: {len(fixture) - len(diff)}/{len(fixture)} casos idénticos")
sys.exit(1 if diff else 0)
PY

# 5. Rust: paridad paso a paso, hostiles, integración, unitarias y doc.
expect_limit=1
if [[ "$(uname)" = Darwin ]]; then expect_limit=0; fi
CPE_VALIDATOR_RUST_OUT="$PWD/$evidence/cpe-validator-rust.json" CPE_EXPECT_OP_LIMIT=$expect_limit \
  run_offline cargo test -p cpeforge-cpe-validator --all-features --locked --offline
if [[ "$(uname)" = Darwin ]]; then
  # La libxml2 de Apple solo aplica opLimit a binarios con objetivo >= 13.
  CPE_EXPECT_OP_LIMIT=1 MACOSX_DEPLOYMENT_TARGET=13.0 CARGO_TARGET_DIR=target/macos13 \
    run_offline cargo test -p cpeforge-cpe-validator --test hostile --locked --offline
fi
run_offline cargo run -q -p cpeforge-cpe-validator --example validate_rules --locked --offline

# 6. Verificación independiente: reglas evaluadas con Python (ElementTree + re)
#    y hojas ejecutadas con xsltproc.
run_offline python3 -I scripts/cpe_validator_independent_verify.py "$ref/cases.json" "$ref/expected.json" \
  "$evidence/cpe-validator-rust.json" "$evidence/cpe-validator-independent.json"
echo "cpe-validator: puertas de paridad completadas"
