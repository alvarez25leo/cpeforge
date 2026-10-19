#!/usr/bin/env bash
# Gate de paridad de cpeforge-report contra greenter/report v5.3.0.
#
# 1. Prepara la referencia PHP aislada (commit fijado) y las dependencias del
#    oráculo (oracle/php/report/composer.lock: Twig 3.30.0, Bacon QR 3.1.1).
# 2. Comprueba que las plantillas de crates/report son las de PHP más las
#    adaptaciones documentadas (scripts/report_template_adaptations.py).
# 3. Ejecuta la suite PHPUnit upstream de packages/report (con greenter/data
#    de la suite fijada) si está disponible.
# 4. Regenera casos y salidas PHP en un directorio temporal y exige igualdad
#    con tests/reference/report (salvo el instante `now` de date(null)).
# 5. Verifica tests/reference/report/SHA256SUMS.
# 6. Ejecuta las pruebas Rust de report (paridad, seguridad, unitarias) y la
#    verificación independiente (HTML con html.parser, SVG con ElementTree y
#    decodificación QR con rqrr en scripts/report_qr_verify).
#
# Uso: scripts/report_parity.sh [--update]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ref_dir="$root/tests/reference/report"
update=false
[[ "${1:-}" == "--update" ]] && update=true

GREENTER_PHP_REFERENCE="$("$root/scripts/prepare_php_reference.sh")"
export GREENTER_PHP_REFERENCE
(cd "$root/oracle/php/report" && composer install --no-interaction --no-progress --quiet)
echo "referencia PHP: $(git -C "$GREENTER_PHP_REFERENCE" rev-parse HEAD)"
php -n --version | head -1
echo "composer.lock del oráculo: $(shasum -a 256 "$root/oracle/php/report/composer.lock" | cut -d' ' -f1)"

python3 "$root/scripts/report_template_adaptations.py" check \
    "$GREENTER_PHP_REFERENCE/packages/report/src/Report/Templates" "$root/crates/report/templates"

suite="$root/target/reference/v5.3.0-suite"
if [[ -x "$suite/vendor/bin/phpunit" ]]; then
    echo "suite PHPUnit upstream de packages/report:"
    (cd "$suite" && php vendor/bin/phpunit packages/report/tests --no-coverage)
else
    echo "AVISO: no existe $suite/vendor/bin/phpunit; suite upstream no ejecutada" >&2
    [[ "${REPORT_REQUIRE_UPSTREAM_SUITE:-0}" == "1" ]] && exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
python3 "$root/oracle/cases/report/generate_cases.py" "$work"
mkdir -p "$work/expected"
oracle="$root/oracle/php/report/run_cases.php"
php -n "$oracle" html "$work/cases/html.json" "$work/cases" "$work/expected/html" > "$work/expected/html.json"
php -n "$oracle" qr "$work/cases/qr.json" "$work/expected/qr" > "$work/expected/qr.json"
php -n "$oracle" xml "$work/cases/xml.json" "$work/expected/xml" > "$work/expected/xml.json"
php -n "$oracle" resolver "$work/cases/resolver.json" > "$work/expected/resolver.json"
php -n "$oracle" catalog > "$work/expected/catalog.json"
rmdir "$work/expected/xml" 2>/dev/null || true
# Identidad CPEForge (nombres de clase, firma): oracle/identidad.py.
python3 "$root/oracle/identidad.py" "$work"

if $update; then
    rm -rf "$ref_dir/cases" "$ref_dir/expected"
    mkdir -p "$ref_dir"
    cp -R "$work/cases" "$work/expected" "$ref_dir/"
    (cd "$ref_dir" && find cases expected -type f | LC_ALL=C sort | xargs shasum -a 256 > SHA256SUMS)
    echo "fixtures actualizados; revisar el diff antes de aceptarlos"
fi

# Compara la ejecución nueva con los fixtures: metadatos del entorno y `now`
# se excluyen; en el HTML se sustituye la fecha de Lima del `now` de cada
# ejecución (date(null) imprime la fecha actual).
python3 - "$ref_dir" "$work" <<'PY'
import json, sys
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

ref, new = Path(sys.argv[1]), Path(sys.argv[2])
failed = False

def load(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    for field in ("reference_version", "timezone_db", "libxml"):
        data.pop(field, None)
    return data

for family in ("html", "qr", "xml", "resolver", "catalog"):
    a, b = load(ref / "expected" / f"{family}.json"), load(new / "expected" / f"{family}.json")
    nows = {}
    if family == "html":
        for side, data in (("ref", a), ("new", b)):
            for case_id, result in data["results"].items():
                nows[(side, case_id)] = result.pop("now")
                result.pop("sha256", None)
    if a != b:
        print(f"{family}.json distinto"); failed = True
    if family == "html":
        for path in sorted((new / "expected" / "html").glob("*.html")):
            old = ref / "expected" / "html" / path.name
            def norm(text, now):
                stamp = datetime.fromtimestamp(now, ZoneInfo("America/Lima")).strftime("%d/%m/%Y")
                return text.replace(stamp, "NOW-DATE")
            case_id = path.stem
            if not old.exists() or norm(old.read_text(encoding="utf-8"), nows[("ref", case_id)]) != norm(path.read_text(encoding="utf-8"), nows[("new", case_id)]):
                print(f"HTML distinto: {case_id}"); failed = True
for sub in ("qr",):
    for path in sorted((new / "expected" / sub).glob("*")):
        if (ref / "expected" / sub / path.name).read_bytes() != path.read_bytes():
            print(f"{sub}/{path.name} distinto"); failed = True
for path in sorted((new / "cases").rglob("*")):
    if path.is_file() and (ref / path.relative_to(new)).read_bytes() != path.read_bytes():
        print(f"caso distinto: {path.relative_to(new)}"); failed = True
sys.exit(1 if failed else 0)
PY
(cd "$ref_dir" && shasum -a 256 -c SHA256SUMS --quiet)
echo "fixtures PHP reproducidos: OK"

cd "$root"
mkdir -p "$work/rust"
REPORT_RUST_OUT="$work/rust" cargo test -p cpeforge-report --all-features --locked -- --nocapture
python3 "$root/scripts/report_independent_verify.py" "$ref_dir" "$work/rust"
