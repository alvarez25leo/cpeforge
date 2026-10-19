# Evidencia de CI de los gates de paridad (docs/verificacion-paridad.md, §6).
#
# Uso: `source scripts/evidence.sh` y, en la trampa EXIT del script,
# `save_evidence <nombre> <ruta>...`: copia las salidas regeneradas en
# target/evidence/<nombre>/ antes de borrar el directorio temporal, también
# cuando el gate falla. Nunca copiar claves: los archivos con una clave privada
# PEM se eliminan de la copia por si una salida la incluyera.

save_evidence() {
  local dest
  dest="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/target/evidence/$1"
  shift
  rm -rf "$dest"
  mkdir -p "$dest"
  local path
  for path in "$@"; do
    if [[ -e "$path" ]]; then
      cp -R "$path" "$dest/"
    fi
  done
  # Corre en trampas EXIT con `set -e`: un fallo aquí cambiaría el estado del
  # gate, y grep sin coincidencias devuelve 1.
  grep -rl -e '-----BEGIN [A-Z ]*PRIVATE KEY-----' "$dest" 2>/dev/null \
    | while IFS= read -r file; do rm -f "$file"; done || true
  return 0
}
