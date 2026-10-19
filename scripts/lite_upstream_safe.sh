#!/usr/bin/env bash
# Ejecuta SOLO FeFactoryXmlTest, tras sustituir su PEM upstream en una copia
# temporal por un PEM efímero. Toda la red del proceso PHPUnit está denegada.
set -euo pipefail
lite_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if command -v sandbox-exec >/dev/null; then
  lite_php=(sandbox-exec -p '(version 1)(allow default)(deny network*)')
# Lo invoca scripts/cpeforge_parity.sh, que comparte el bloqueo de red del job.
elif [[ "${CPEFORGE_NETWORK_BLOCKED:-}" == 1 ]]; then
  sudo -n iptables -C OUTPUT ! -o lo -j REJECT
  sudo -n ip6tables -C OUTPUT ! -o lo -j REJECT
  lite_php=()
else
  echo 'Falta bloqueo de red verificable para PHPUnit' >&2
  exit 2
fi
lite_tmp="$(mktemp -d)"
cleanup() {
  python3 - "$lite_tmp" <<'PY'
import shutil, sys
shutil.rmtree(sys.argv[1])
PY
}
trap cleanup EXIT
chmod 700 "$lite_tmp"
cd "$lite_root"
test "$(git -C target/reference/v5.3.0 rev-parse HEAD)" = 'f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f'
scripts/xmldsig_keys.sh "$lite_tmp/keys"
mkdir "$lite_tmp/tests"
python3 - "$lite_tmp/tests" <<'PY'
from pathlib import Path
import sys
root = Path.cwd()
src = root / 'target/reference/v5.3.0/packages/lite/tests/Greenter/Factory'
dst = Path(sys.argv[1])
base = (src / 'FeFactoryBase.php').read_text()
old = "__DIR__.'/../../Resources/SFSCert.pem'"
assert base.count(old) == 1, 'referencia de PEM cambió; auditar nuevamente'
(dst / 'FeFactoryBase.php').write_text(base.replace(old, "getenv('LITE_KEYS_DIR').'/a.pem'"))
(dst / 'FeFactoryXmlTest.php').write_bytes((src / 'FeFactoryXmlTest.php').read_bytes())
PY
cat > "$lite_tmp/bootstrap.php" <<'PHP'
<?php
$root = getcwd();
$ref = $root.'/target/reference/v5.3.0';
require $root.'/target/reference/v5.3.0-suite/vendor/autoload.php';
spl_autoload_register(static function (string $class) use ($ref): void {
    if (!str_starts_with($class, 'Greenter\\')) return;
    $relative = str_replace('\\', '/', substr($class, 9)).'.php';
    foreach (['lite/src/Greenter', 'xml/src', 'ws/src', 'core/src/Core'] as $part) {
        $file = "$ref/packages/$part/$relative";
        if (is_file($file)) { require $file; return; }
    }
}, true, true);
require getenv('LITE_SAFE_TEST_DIR').'/FeFactoryBase.php';
PHP
export LITE_KEYS_DIR="$lite_tmp/keys" LITE_SAFE_TEST_DIR="$lite_tmp/tests"
"${lite_php[@]}" php \
  -d log_errors=0 target/reference/v5.3.0-suite/vendor/bin/phpunit \
  --bootstrap "$lite_tmp/bootstrap.php" "$lite_tmp/tests/FeFactoryXmlTest.php"
