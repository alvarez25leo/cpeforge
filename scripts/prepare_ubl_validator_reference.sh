#!/usr/bin/env bash
# Prepara la referencia externa en target, sin tocar greenter-php ni el checkout principal.
#
# Un clon existente solo se reutiliza si está en el commit fijado, tiene el tag
# y está limpio. Si no (por ejemplo, restaurado a medias por una caché de CI),
# se descarta y se vuelve a clonar.
set -euo pipefail
cd "$(dirname "$0")/.."
reference=target/reference/ubl-validator-v2.3.0
commit=980bad9412eab51516e4879b06d8d2e723c62f0d

valid_reference() {
  [[ -d "$reference/.git" ]] \
    && [[ "$(git -C "$reference" rev-parse HEAD 2>/dev/null)" == "$commit" ]] \
    && [[ "$(git -C "$reference" rev-parse -q --verify 'v2.3.0^{commit}' 2>/dev/null)" == "$commit" ]] \
    && [[ -z "$(git -C "$reference" status --porcelain 2>/dev/null)" ]]
}

if [[ -e "$reference" ]] && ! valid_reference; then
  echo "referencia ubl-validator incompleta o modificada: se vuelve a clonar" >&2
  rm -rf "$reference"
fi
if [[ ! -d "$reference/.git" ]]; then
  mkdir -p target/reference
  git clone --filter=blob:none --no-checkout https://github.com/thegreenter/ubl-validator.git "$reference"
  git -C "$reference" -c advice.detachedHead=false checkout --detach "$commit"
fi
test "$(git -C "$reference" rev-parse HEAD)" = "$commit"
test "$(git -C "$reference" rev-parse v2.3.0)" = "$commit"
test -z "$(git -C "$reference" status --porcelain)"
echo "ubl-validator v2.3.0 $commit"
