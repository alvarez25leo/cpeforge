#!/usr/bin/env bash
# Prepara greenter/gre-api v1.0.2 en target; nunca toca greenter-php/.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="${GRE_API_PHP_REFERENCE:-$root/target/reference/gre-api-v1.0.2}"
commit="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["baseline_commit"])' "$root/oracle/upstream/referencia-gre-api.json")"

if test -d "$dest/.git"; then
  test "$(git -C "$dest" rev-parse HEAD)" = "$commit"
  test -z "$(git -C "$dest" status --porcelain --untracked-files=no)"
else
  test ! -e "$dest"
  mkdir -p "$(dirname "$dest")"
  git clone --quiet --no-checkout https://github.com/thegreenter/gre-api.git "$dest"
  git -C "$dest" -c advice.detachedHead=false checkout --quiet "$commit"
fi
test "$(git -C "$dest" rev-parse v1.0.2)" = "$commit"
echo "$dest"
