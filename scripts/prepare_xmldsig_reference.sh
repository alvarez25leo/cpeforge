#!/usr/bin/env bash
# Prepara una copia aislada de thegreenter/xmldsig en el commit fijado por
# oracle/upstream/referencia-xmldsig.json (tag v5.1.0) y genera su autoload.
#
# No toca `greenter-php/`. Comprueba que el tag apunte al commit esperado y que
# el árbol quede limpio. Imprime la ruta del checkout.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ref_json="$root/oracle/upstream/referencia-xmldsig.json"
read_ref() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$ref_json" "$1"; }
tag="$(read_ref baseline_tag)"
commit="$(read_ref baseline_commit)"
repo="$(read_ref repository)"
dest="${XMLDSIG_PHP_REFERENCE:-$root/target/reference/xmldsig-$tag}"

fresh=false
if [[ ! -d "$dest/.git" ]]; then
    rm -rf "$dest"
    mkdir -p "$(dirname "$dest")"
    git clone --quiet --no-checkout "$repo" "$dest"
    fresh=true
fi
tag_commit="$(git -C "$dest" rev-parse "refs/tags/$tag^{commit}")"
if [[ "$tag_commit" != "$commit" ]]; then
    echo "el tag $tag apunta a $tag_commit (esperado $commit)" >&2
    exit 1
fi
# Tras `clone --no-checkout` el índice está vacío aunque HEAD ya apunte al
# commit fijado (cuando es la última versión del remoto): hay que poblarlo.
if $fresh || [[ "$(git -C "$dest" rev-parse HEAD 2>/dev/null || true)" != "$commit" ]]; then
    git -C "$dest" -c advice.detachedHead=false checkout --quiet --force "$commit"
fi
if [[ -n "$(git -C "$dest" status --porcelain --untracked-files=no)" ]]; then
    echo "el checkout $dest tiene cambios locales" >&2
    exit 1
fi
if [[ ! -f "$dest/vendor/autoload.php" ]]; then
    (cd "$dest" && composer dump-autoload --quiet --no-interaction)
fi
echo "$dest"
