#!/usr/bin/env bash
# Envía los casos de example/ con la referencia PHP y con CPEForge a los entornos de
# prueba (SUNAT BETA y la API GRE de GRE_API_URL) y compara al final los
# tiempos de cada fase, red incluida (benchmark.py).
#
#   bash oracle/ejemplos/enviar.sh                   # todos los casos
#   bash oracle/ejemplos/enviar.sh factura resumen   # solo esos
#
# Sin EJEMPLO_FECHA se usa la hora actual, como exige SUNAT. Entre envíos se
# esperan EJEMPLO_PAUSA segundos (5 por defecto) para evitar el 401 de BETA.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
ex="$(cd "$here/../../example" && pwd)"
if ! test -f "$ex/php/vendor/autoload.php"; then
  composer install --working-dir="$ex/php" --no-interaction --no-progress
fi
estado=0
echo "== PHP"
php "$ex/php/emitir.php" --enviar "$@" || estado=1
sleep "${EJEMPLO_PAUSA:-5}"
echo
echo "== Rust"
cargo run -q --release --locked --manifest-path "$ex/rust/Cargo.toml" -- --enviar "$@" || estado=1
echo
python3 "$here/benchmark.py" || estado=1
exit "$estado"
