#!/usr/bin/env bash
# Envía solo los casos elegidos a SUNAT BETA o GRE de prueba y compara rechazos.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
example="$(cd "$here/../../../example" && pwd)"
export EJEMPLO_FECHA="${EJEMPLO_FECHA:-$(TZ=America/Lima date '+%Y-%m-%d %H:%M:%S')}"
if ! test -f "$example/php/vendor/autoload.php"; then
  composer install --working-dir="$example/php" --no-interaction --no-progress
fi
rm -rf "$example/php/out/errores" "$example/rust/out/errores"
php "$example/php/emitir.php" --errores --enviar "$@"
sleep "${EJEMPLO_PAUSA:-5}"
cargo run -q --release --locked --manifest-path "$example/rust/Cargo.toml" -- --errores --enviar "$@"
python3 "$here/verificar.py" --enviar "$@"
