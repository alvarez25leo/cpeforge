#!/usr/bin/env bash
# Reejecuta las cuatro clases PHP originales sobre el checkout fijo. Solo se
# adapta en copias ignoradas la firma setUp(): void que exige PHPUnit 9.
set -euo pipefail
cd "$(dirname "$0")/.."
reference=target/reference/ubl-validator-v2.3.0
test "$(git -C "$reference" rev-parse HEAD)" = 980bad9412eab51516e4879b06d8d2e723c62f0d
python3 - <<'PY'
from pathlib import Path
import shutil
source=Path('target/reference/ubl-validator-v2.3.0')
base=Path('target/ubl-upstream-phpunit9')
if base.exists(): shutil.rmtree(base)
for p in (source/'tests/Ubl').rglob('*.php'):
    q=base/'tests'/p.relative_to(source/'tests')
    q.parent.mkdir(parents=True,exist_ok=True)
    q.write_text(p.read_text().replace('protected function setUp()', 'protected function setUp(): void'))
(base/'tests/Resources').symlink_to((source/'tests/Resources').resolve())
(base/'src').symlink_to((source/'src').resolve())
PY
cat > target/ubl-upstream-bootstrap.php <<'PHP'
<?php
spl_autoload_register(static function ($class) {
    if (strncmp($class, 'Greenter\\', 9) !== 0) return;
    $path = __DIR__ . '/reference/ubl-validator-v2.3.0/src/' . str_replace('\\', '/', substr($class, 9)) . '.php';
    if (is_file($path)) require $path;
}, true, true);
class_alias(PHPUnit\Framework\TestCase::class, 'PHPUnit_Framework_TestCase');
PHP
phpunit_bin="${PHPUNIT_BIN:-target/php-oracle-vendor/bin/phpunit}"
profile='(version 1)(allow default)(deny network*)'
mkdir -p target/evidence
log=target/evidence/ubl-validator-upstream-suite.log
suite=(--bootstrap target/ubl-upstream-bootstrap.php target/ubl-upstream-phpunit9/tests/Ubl --no-coverage)
status=0
if [[ "${UBL_VALIDATOR_NETWORK_BLOCKED:-}" = 1 ]]; then
  "$phpunit_bin" "${suite[@]}" > "$log" 2>&1 || status=$?
elif command -v sandbox-exec >/dev/null 2>&1; then
  sandbox-exec -p "$profile" "$phpunit_bin" "${suite[@]}" > "$log" 2>&1 || status=$?
else
  echo 'Se requiere sandbox-exec o UBL_VALIDATOR_NETWORK_BLOCKED=1 con cortafuegos externo' >&2
  exit 2
fi
cat "$log"
# La suite upstream no es compatible con PHP 8: testEmptyXML y testInvalidXml
# llaman a DOMDocument::loadXML('') y reciben ValueError (registrado en
# docs/evidencia/ubl-validator-0.1.0.md). Se exige exactamente ese resultado:
# cualquier otro fallo, o que dejen de fallar, rompe el paso.
python3 -I - "$log" "$status" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
status = int(sys.argv[2])
if status == 0:
    sys.exit("la suite upstream ya pasa: actualizar este script y la evidencia")
assert "Tests: 24, Assertions: 29, Errors: 2." in text, "resumen distinto del registrado"
failing = re.findall(r"^\d+\) (\S+)$", text, re.M)
assert failing == [
    "Tests\\Greenter\\Ubl\\Resolver\\UblVersionResolverTest::testEmptyXML",
    "Tests\\Greenter\\Ubl\\UblValidatorTest::testInvalidXml",
], failing
assert text.count("ValueError: DOMDocument::loadXML(): Argument #1 ($source) must not be empty") == 2
print("suite upstream: solo los 2 errores conocidos de PHP 8 (loadXML('')), sin otros fallos")
PY
