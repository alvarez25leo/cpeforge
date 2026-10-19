#!/usr/bin/env bash
# Gate de paridad de cpeforge-xmldsig contra la referencia PHP xmldsig v5.1.0.
#
# 1. Checkout aislado de la referencia (tag v5.1.0 = commit fijado) y suite
#    PHPUnit de la referencia.
# 2. Claves y certificados EFÍMEROS en un directorio temporal (se borra al
#    salir; nunca se imprimen ni se versionan).
# 3. Casos sintéticos regenerados == tests/reference/xmldsig/cases.
# 4. Oráculo PHP con esas claves y comparación en vivo con Rust (mismas
#    claves: firmas, digest, C14N y XML firmado byte a byte); los fixtures
#    redactados deben reproducirse.
# 5. Verificación cruzada: PHP firma → Rust verifica (dentro de 4) y Rust
#    firma → PHP verifica.
# 6. Verificación independiente (xmllint del sistema + LibreSSL).
# 7. Pruebas Rust del crate (incluida la comparación offline redactada) y
#    SHA256SUMS de los fixtures.
#
# Uso: scripts/xmldsig_parity.sh [--update]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ref_dir="$root/tests/reference/xmldsig"
update=false
[[ "${1:-}" == "--update" ]] && update=true
# openssl-sys usa pkg-config; en macOS con Homebrew se indica el prefijo.
if [[ -z "${OPENSSL_DIR:-}" && -d /opt/homebrew/opt/openssl@3 ]]; then
    export OPENSSL_DIR=/opt/homebrew/opt/openssl@3
fi

# Asignación separada: con `export "$(…)"` un fallo de la preparación no
# detendría el script (set -e no lo ve).
# `php -n` aísla el oráculo de la configuración local, pero solo carga las
# extensiones compiladas dentro de PHP. En Ubuntu (setup-php) dom, openssl y
# mbstring son extensiones compartidas que se cargan desde php.ini: allí se
# usa `php` sin -n.
php_cmd=(php -n)
if ! php -n -r 'exit(class_exists("DOMDocument") && extension_loaded("openssl") ? 0 : 1);' 2>/dev/null; then
    php_cmd=(php)
fi
XMLDSIG_PHP_REFERENCE="$("$root/scripts/prepare_xmldsig_reference.sh")"
export XMLDSIG_PHP_REFERENCE
(cd "$root/oracle/php" && composer install --no-interaction --no-progress --quiet)
echo "referencia PHP: $(git -C "$XMLDSIG_PHP_REFERENCE" describe --tags) $(git -C "$XMLDSIG_PHP_REFERENCE" rev-parse HEAD)"
"${php_cmd[@]}" -r 'echo "PHP ", PHP_VERSION, " | ", OPENSSL_VERSION_TEXT, " | libxml2 ", LIBXML_DOTTED_VERSION, "\n";'

work="$(mktemp -d)"
source "$root/scripts/evidence.sh"
# Sin keys/ ni los XML firmados: solo salidas, veredictos y logs.
trap 'save_evidence xmldsig-regenerated "$work/phpunit.log" "$work/php" "$work/live.log" "$work/cross/reference-verdicts.json" "$work/independent.json"; rm -rf "$work"' EXIT
chmod 700 "$work"

# 1. Suite PHPUnit de la referencia (sin detenerse en el primer fallo).
sed -e 's/stopOnFailure="true"/stopOnFailure="false"/' \
    -e "s#<directory suffix=\".phpt\">tests#<directory suffix=\".phpt\">$XMLDSIG_PHP_REFERENCE/tests#" \
    "$XMLDSIG_PHP_REFERENCE/phpunit.xml" > "$work/phpunit.xml"
set +e
(cd "$XMLDSIG_PHP_REFERENCE" && php "$root/target/php-oracle-vendor/bin/phpunit" -c "$work/phpunit.xml") > "$work/phpunit.log" 2>&1
set -e
git -C "$XMLDSIG_PHP_REFERENCE" checkout --quiet -- tests 2>/dev/null || true
rm -f "$XMLDSIG_PHP_REFERENCE/.phpunit.result.cache"
grep -E '^(OK|Tests:)' "$work/phpunit.log" | sed 's/^/PHPUnit referencia: /'
python3 - "$work/phpunit.log" <<'PY'
import re, sys
log = open(sys.argv[1]).read()
fails = re.split(r"\n\d+\) ", log.split("There were", 1)[1])[1:] if "There were" in log else []
other = [f.splitlines()[0] for f in fails if "Implicitly marking parameter $doc as nullable is deprecated" not in f]
print(f"PHPUnit referencia: {len(fails)} fallos, {len(fails) - len(other)} solo por el aviso de deprecación de PHP 8.4")
sys.exit(1 if other else 0)
PY

# 2. Material efímero.
"$root/scripts/xmldsig_keys.sh" "$work/keys"

# 3. Casos.
python3 "$root/oracle/cases/xmldsig/generate_cases.py" "$work/cases"
# Identidad CPEForge en las entradas (URI de la firma): oracle/identidad.py.
python3 "$root/oracle/identidad.py" "$work/cases"
diff -ru "$ref_dir/cases" "$work/cases"

# 4. Oráculo PHP y comparación en vivo.
mkdir -p "$work/php" "$work/cross"
for cases in "$work"/cases/*.json; do
    family="$(basename "$cases" .json)"
    "${php_cmd[@]}" -d memory_limit=1G "$root/oracle/php/xmldsig/run_cases.php" "$cases" "$work/keys" > "$work/php/$family.json"
done
# El Id de la firma de la referencia (fuera de SignedInfo) pasa a CPEForgeSign.
python3 "$root/oracle/identidad.py" "$work/php"
export XMLDSIG_KEYS_DIR="$work/keys" XMLDSIG_REFERENCE_OUT="$work/php" XMLDSIG_CROSS_DIR="$work/cross"
$update && export XMLDSIG_UPDATE=1
# Si falla, se muestra el detalle completo del comparador (casos y primer
# campo distinto), no solo el resumen.
if ! cargo test -q -p cpeforge-xmldsig --test parity live_parity -- --ignored --nocapture > "$work/live.log" 2>&1; then
    head -c 60000 "$work/live.log"
    exit 1
fi
grep -E 'live|verificó|test result' "$work/live.log"

# 5. Rust firma → PHP verifica.
"${php_cmd[@]}" "$root/oracle/php/xmldsig/run_cases.php" "$work/cross/cases.json" "$work/keys" "$work/cross/rust-signed" > "$work/cross/reference-verdicts.json"
python3 "$root/oracle/identidad.py" "$work/cross/reference-verdicts.json"
cargo test -q -p cpeforge-xmldsig --test parity live_reference_verdicts -- --ignored --nocapture 2>&1 | grep -E 'verificó|test result'

# 6. Verificación independiente.
python3 -I "$root/scripts/xmldsig_independent_verify.py" "$work/cross/rust-signed" "$work/keys/a.crt" "$work/keys/b.crt" "$work/independent.json"

# 7. Pruebas del crate y sumas de los fixtures.
cargo test -q -p cpeforge-xmldsig 2>&1 | grep -E 'offline|test result'
if $update; then
    (cd "$ref_dir" && find cases expected differences.json -type f | sort | xargs shasum -a 256 > SHA256SUMS)
    echo "fixtures actualizados; revisar el diff antes de aceptarlos"
fi
(cd "$ref_dir" && shasum -a 256 -c --quiet SHA256SUMS)
# Ningún fixture puede contener claves ni certificados reales (los casos solo
# llevan bloques PEM sintéticos de pocos bytes).
python3 - "$ref_dir" <<'PY'
import re, sys
from pathlib import Path
bad = []
for path in Path(sys.argv[1]).rglob("*.json"):
    text = path.read_text()
    # Un bloque PEM dentro de una misma cadena JSON; los sintéticos de los
    # casos tienen cuerpos de pocos bytes.
    for m in re.finditer(r"-----BEGIN ([A-Z ]+)-----([^\"]*?)-----END", text):
        if len(re.sub(r"\\[nr]|\s", "", m.group(2))) > 40:
            bad.append(f"{path.name}: {m.group(1)}")
if bad:
    sys.exit("material PEM en fixtures: " + ", ".join(sorted(set(bad))))
PY
echo "xmldsig: paridad, verificación cruzada e independiente completas"
