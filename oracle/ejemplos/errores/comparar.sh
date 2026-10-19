#!/usr/bin/env bash
# Genera y firma los ejemplos de rechazo en PHP y Rust; no llama a SUNAT.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
example="$(cd "$here/../../../example" && pwd)"
export EJEMPLO_FECHA="${EJEMPLO_FECHA:-2026-10-17 13:05:00}"
if ! test -f "$example/php/vendor/autoload.php"; then
  composer install --working-dir="$example/php" --no-interaction --no-progress
fi
rm -rf "$example/php/out/errores" "$example/rust/out/errores"
php "$example/php/emitir.php" --errores "$@"
cargo run -q --release --locked --manifest-path "$example/rust/Cargo.toml" -- --errores "$@"
python3 "$here/verificar.py" "$@"
