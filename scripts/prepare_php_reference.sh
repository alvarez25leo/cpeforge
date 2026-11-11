#!/usr/bin/env bash
# Prepara una copia aislada de Greenter PHP en el commit de referencia fijado.
#
# No modifica `greenter-php/`: clona desde él (o desde el remoto oficial si no
# existe) a `target/reference/<tag>` y comprueba el commit exacto.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ref_json="$root/oracle/upstream/referencia.json"
tag="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["baseline_tag"])' "$ref_json")"
commit="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["baseline_commit"])' "$ref_json")"
dest="${PHP_REFERENCE_DIR:-$root/target/reference/$tag}"

if [[ -d "$dest/.git" ]] && [[ "$(git -C "$dest" rev-parse HEAD)" == "$commit" ]] \
    && [[ -z "$(git -C "$dest" status --porcelain --untracked-files=no)" ]]; then
    echo "$dest"
    exit 0
fi

source_repo="$root/greenter-php"
# El clon de consulta no se actualiza desde aquí: si no existe o no contiene
# el commit fijado (referencia más nueva que el clon), se usa el remoto oficial.
if [[ ! -d "$source_repo/.git" ]] \
    || ! git -C "$source_repo" cat-file -e "$commit^{commit}" 2>/dev/null; then
    source_repo="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["repository"])' "$ref_json")"
fi

rm -rf "$dest"
mkdir -p "$(dirname "$dest")"
git clone --quiet --no-checkout "$source_repo" "$dest"
# --force: tras `clone --no-checkout` el índice está vacío y, si HEAD ya
# apunta al commit fijado, un checkout normal no lo poblaría.
git -C "$dest" -c advice.detachedHead=false checkout --quiet --force "$commit"

actual="$(git -C "$dest" rev-parse HEAD)"
if [[ "$actual" != "$commit" ]]; then
    echo "commit inesperado: $actual (esperado $commit)" >&2
    exit 1
fi
echo "$dest"
