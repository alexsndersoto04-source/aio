#!/bin/bash
# Verificación diferencial del generador de bytecode escrito en Titan
# (fase 4a del self-hosting), junto con el cargador de `import`.
#
# Para cada archivo .titan del repositorio (y los casos de selfhost/tests/)
# compara, byte por byte:
#   - `zett bytecode ARCHIVO`                      → cargador + typechecker + codegen de Rust
#   - `zett run selfhost/bytecode.titan ARCHIVO`   → los mismos, escritos en Titan
# Se comparan también los mensajes de error (léxicos, de sintaxis, de import,
# de tipos y del generador).
#
# Uso: bash selfhost/verify_codegen.sh [zett]
set -u
ZETT="${1:-zett}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

{ git ls-files '*.titan'; find selfhost/tests -name "*.titan"; } | sort -u > "$tmp/files"

# Un archivo por proceso, dos a la vez. Cada uno escribe "OK n ruta" o "DIF ruta".
export ZETT tmp
compare() {
  f="$1"
  key="$(printf '%s' "$f" | tr '/' '_')"
  "$ZETT" bytecode "$f" > "$tmp/$key.rust" 2>&1
  "$ZETT" run selfhost/bytecode.titan "$f" > "$tmp/$key.out" 2>&1
  if cmp -s "$tmp/$key.rust" "$tmp/$key.out"; then
    echo "OK $(wc -l < "$tmp/$key.rust") $f"
  else
    echo "DIF $f"
    diff "$tmp/$key.rust" "$tmp/$key.out" | head -10 > "$tmp/$key.diff"
  fi
}
export -f compare
xargs -P 2 -I{} bash -c 'compare "$1"' _ {} < "$tmp/files" > "$tmp/results"

total=$(wc -l < "$tmp/files")
pass=$(grep -c '^OK ' "$tmp/results")
fail=$(grep -c '^DIF ' "$tmp/results")
lines=$(awk '/^OK /{s+=$2} END{print s+0}' "$tmp/results")
shown=0
for f in $(grep '^DIF ' "$tmp/results" | cut -d' ' -f2); do
  if [ "$shown" -lt 3 ]; then
    echo "DIFERENCIA en $f:"
    cat "$tmp/$(printf '%s' "$f" | tr '/' '_').diff"
    shown=$((shown + 1))
  fi
done
echo "archivos: $total  idénticos: $pass  distintos: $fail  (líneas idénticas: $lines)"
grep '^DIF ' "$tmp/results" | sed 's/^DIF /  distinto: /'
[ "$fail" -eq 0 ] && [ "$pass" -eq "$total" ]
