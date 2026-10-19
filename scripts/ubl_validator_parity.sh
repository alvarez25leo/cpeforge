#!/usr/bin/env bash
# Oráculo PHP y Rust sin salida a Internet; dependencias y referencia se preparan antes.
set -euo pipefail
cd "$(dirname "$0")/.."
reference=target/reference/ubl-validator-v2.3.0
commit=980bad9412eab51516e4879b06d8d2e723c62f0d
test "$(git -C "$reference" rev-parse HEAD)" = "$commit"
test "$(git -C "$reference" rev-parse v2.3.0)" = "$commit"
test -z "$(git -C "$reference" status --porcelain)"
test -z "$(git -C greenter-php status --porcelain)"
php -r 'exit(extension_loaded("dom") && extension_loaded("libxml") ? 0 : 1);'
mkdir -p target/evidence
profile='(version 1)(allow default)(deny network*)'
run_offline() {
  if [[ "${UBL_VALIDATOR_NETWORK_BLOCKED:-}" = 1 ]]; then
    "$@"
  elif command -v sandbox-exec >/dev/null 2>&1; then
    sandbox-exec -p "$profile" "$@"
  else
    echo 'Se requiere sandbox-exec o UBL_VALIDATOR_NETWORK_BLOCKED=1 con cortafuegos externo' >&2
    return 2
  fi
}
python3 scripts/ubl_validator_inventory.py target/evidence/ubl-validator-inventory.json
cmp crates/ubl-validator/schema-manifest.json target/evidence/ubl-validator-inventory.json
run_offline php oracle/php/ubl_validator/run_cases.php > target/evidence/ubl-validator-php-current.json
run_offline php oracle/php/ubl_validator/run_security.php > target/evidence/ubl-validator-php-security-current.json
# Identidad CPEForge en los textos del oráculo: oracle/identidad.py.
python3 oracle/identidad.py target/evidence/ubl-validator-php-current.json target/evidence/ubl-validator-php-security-current.json
python3 -I - <<'PY'
import json, os
p='target/evidence/ubl-validator-php-current.json'
s='target/evidence/ubl-validator-php-security-current.json'
# Los oráculos escriben `{workspace}/` en vez del prefijo absoluto del checkout,
# solo en `error` y `errors[].message`; cualquier otra aparición de la ruta es
# un campo sin normalizar y haría el fixture dependiente del checkout.
for path in (p, s):
    text=open(path, encoding='utf-8').read()
    for prefix in {os.getcwd(), os.path.realpath('.')}:
        assert prefix + '/' not in text, f'{path}: ruta absoluta del workspace fuera de {{workspace}}'
j=json.load(open(p))
c=json.load(open('tests/reference/ubl-validator/cases.json'))['cases']
baseline=json.load(open('tests/reference/ubl-validator/reference-expected.json'))
security_baseline=json.load(open('tests/reference/ubl-validator/reference-security-expected.json'))
security_current=json.load(open(s))
assert j['reference']=='980bad9412eab51516e4879b06d8d2e723c62f0d'
assert len(j['results'])==len(c) and len(c)>=48
assert [x['id'] for x in j['results']]==[x['id'] for x in c]
assert all('valid' in x or 'exception' in x for x in j['results'])
if (j['runtime'],j['libxml'])==(baseline['runtime'],baseline['libxml']):
    assert j==baseline, 'oráculo PHP cambió con el mismo runtime fijado'
    assert security_current==security_baseline, 'oráculo de seguridad cambió con el mismo runtime'
else:
    print('Runtime PHP/libxml distinto del fixture; comparación PHP↔Rust en vivo sin normalizar diagnósticos')
security=security_current['results']
registered=json.load(open('tests/reference/ubl-validator/differences.json'))['differences']
assert set(security)=={x['id'] for x in registered}, 'diferencia sin fixture o registro obsoleto'
assert security['ubl/dtd-internal']['valid'] is False
assert security['ubl/version-traversal']['valid'] is False
# Import ausente: primero el fallo de E/S y después 3084 (schema no localizado).
# libxml2 >= 2.13 lo informa como XML_IO_ENOENT (1524); 2.9–2.12, como
# XML_IO_LOAD_ERROR (1549). Ambos significan que el archivo no se pudo cargar.
io_code, schema_code = [x['code'] for x in security['ubl/xades-missing-import']['errors']][:2]
assert (io_code, schema_code) in {(1524, 3084), (1549, 3084)}, (io_code, schema_code)
assert security['ubl/schema-outside-manifest']=={'valid': True, 'error_count': 0}
assert security['ubl/deep-nesting']=={'file': 'security-deep-nesting.xml', 'valid': True, 'error': ''}
print('PHP:', len(c), 'casos',j['runtime'],'libxml',j['libxml'])
PY
UBL_REFERENCE_EXPECTED="$PWD/target/evidence/ubl-validator-php-current.json" run_offline cargo test --offline --locked -p cpeforge-ubl-validator
python_bin="${UBL_XMLSCHEMA_PYTHON:-target/ubl-independent-venv312/bin/python}"
run_offline "$python_bin" scripts/ubl_validator_independent.py
