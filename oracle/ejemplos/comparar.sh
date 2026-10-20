#!/usr/bin/env bash
# Genera todos los casos de example/ con la referencia PHP y con CPEForge, con
# la misma fecha base, y exige XML idéntico salvo la firma y la identidad
# (normalizar.py). Además valida cada XML de Rust
# contra su XSD (UBL 2.1 para comprobantes y guías; 2.0 de SUNAT para
# resúmenes, bajas, percepciones y retenciones) si el oráculo instaló los
# esquemas. Al final compara los tiempos de cada fase (benchmark.py).
# No envía nada (para eso: enviar.sh).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
ex="$root/example"
export EJEMPLO_FECHA="${EJEMPLO_FECHA:-2026-10-17 13:05:00}"

if ! test -f "$ex/php/vendor/autoload.php"; then
  composer install --working-dir="$ex/php" --no-interaction --no-progress
fi
rm -rf "$ex/php/out" "$ex/rust/out"
php "$ex/php/emitir.php" > /dev/null
# Release: el benchmark compara con código optimizado.
cargo run -q --release --locked --manifest-path "$ex/rust/Cargo.toml" > /dev/null

xsd="$root/target/reference/ubl-validator-v2.3.0/src/xsd"
php_files=$(cd "$ex/php/out" && ls ./*.xml | sort)
# Casos que solo existen en Rust (solo-rust.txt): se validan aparte.
solo_rust=$(grep -v -e '^#' -e '^$' "$here/solo-rust.txt" | sed 's#^#./#' || true)
rust_files=$(cd "$ex/rust/out" && ls ./*.xml | sort | grep -v -x -F -e "${solo_rust:-/}" || true)
if test "$php_files" != "$rust_files"; then
  echo "PHP y Rust generaron archivos distintos:"
  diff <(echo "$php_files") <(echo "$rust_files") || true
  exit 1
fi

fallos=0
total=0
for php_xml in "$ex"/php/out/*.xml; do
  name="$(basename "$php_xml")"
  rust_xml="$ex/rust/out/$name"
  total=$((total + 1))
  if cmp -s <(python3 "$here/normalizar.py" "$php_xml") <(python3 "$here/normalizar.py" "$rust_xml"); then
    estado="IGUAL  "
  else
    estado="DIFIERE"
    fallos=$((fallos + 1))
  fi
  esquema=""
  if test -d "$xsd"; then
    raiz="$(xmllint --xpath 'local-name(/*)' "$rust_xml")"
    case "$raiz" in
      Invoice | CreditNote | DebitNote | DespatchAdvice) version=2.1 ;;
      *) version=2.0 ;;
    esac
    if xmllint --noout --nonet --schema "$xsd/$version/maindoc/UBL-$raiz-$version.xsd" "$rust_xml" 2> /dev/null; then
      esquema="XSD $raiz $version válido"
    else
      esquema="XSD $raiz $version INVÁLIDO"
      fallos=$((fallos + 1))
    fi
  fi
  echo "$estado $name  $esquema"
  if test "$estado" = DIFIERE; then
    diff <(python3 "$here/normalizar.py" "$php_xml" | xmllint --format -) <(python3 "$here/normalizar.py" "$rust_xml" | xmllint --format -) | head -20 || true
  fi
done
for name in $solo_rust; do
  rust_xml="$ex/rust/out/${name#./}"
  total=$((total + 1))
  if ! test -f "$rust_xml"; then
    echo "SOLO RUST ${name#./}: no se generó"
    fallos=$((fallos + 1))
    continue
  fi
  esquema="sin XSD"
  if test -d "$xsd"; then
    raiz="$(xmllint --xpath 'local-name(/*)' "$rust_xml")"
    if xmllint --noout --nonet --schema "$xsd/2.1/maindoc/UBL-$raiz-2.1.xsd" "$rust_xml" 2> /dev/null; then
      esquema="XSD $raiz 2.1 válido"
    else
      esquema="XSD $raiz 2.1 INVÁLIDO"
      fallos=$((fallos + 1))
    fi
  fi
  echo "SOLO RUST ${name#./}  $esquema"
done
test -d "$xsd" || echo "XSD no instalados: ejecutar scripts/prepare_ubl_validator_reference.sh para validar contra los esquemas"
test "$fallos" -eq 0
echo "PHP y Rust: XML equivalente (sin firma ni identidad) en $((total - $(echo "$solo_rust" | grep -c .))) casos; $(echo "$solo_rust" | grep -c .) solo en Rust"
echo
python3 "$here/benchmark.py"
