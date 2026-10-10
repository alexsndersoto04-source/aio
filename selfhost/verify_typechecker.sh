#!/bin/bash
# Verificación diferencial del typechecker escrito en Titan (fase 3 del self-hosting).
#
# Para cada archivo .titan del repositorio (y los casos límite de
# selfhost/tests/) compara, byte por byte:
#   - `zett typecheck ARCHIVO`                   → typechecker de Rust (oráculo)
#   - `zett run selfhost/check.titan ARCHIVO`   → typechecker escrito en Titan
#
# Uso: bash selfhost/verify_parser.sh [zett]
set -u
ZETT="${1:-zett}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

mapfile -t FILES < <(
  { git ls-files '*.titan'; ls selfhost/tests/lexer/*.titan selfhost/tests/parser/*.titan; find selfhost/tests/typechecker -name "*.titan"; } | sort -u
)

pass=0
fail=0
lines=0
failed=()
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Divergencias deliberadas con el oráculo congelado `zett` (2026-10-10): el
# typechecker escrito en Titan corrige defectos que el `zett` en Rust aún
# reproduce (asignación en posición de sentencia = (), if con ramas de tipos
# distintos en cola de un bucle, posiciones de errores que caían en el span de
# la función, y los casos de selfhost/tests/errores/ que documentan esas
# correcciones). En esos archivos la verdad es el SPEC, no el oráculo: se
# verifican con `bash selfhost/tests/arreglos/probar.sh` y
# `bash selfhost/tests/errores/probar.sh`. OJO: las posiciones corregidas
# pueden cambiar el volcado de CUALQUIER archivo del corpus que tenga errores
# antes emitidos sin span; si al reconstruir `zett` algo difiere en la
# posición pero el mensaje coincide, la corrección de Titan es la buena.
DELIBERADAS=" selfhost/prueba_arreglos.titan selfhost/tests/errores/posicion.titan selfhost/tests/errores/sin_duplicar.titan selfhost/tests/errores/proyecto/libreta.titan selfhost/tests/errores/proyecto/principal.titan "

for f in "${FILES[@]}"; do
  case "$DELIBERADAS" in
    *" $f "*) continue ;;
  esac
  "$ZETT" typecheck "$f" > "$tmp/rust.txt" 2>&1
  "$ZETT" run selfhost/check.titan "$f" > "$tmp/titan.txt" 2>&1
  if cmp -s "$tmp/rust.txt" "$tmp/titan.txt"; then
    pass=$((pass + 1))
    lines=$((lines + $(wc -l < "$tmp/rust.txt")))
  else
    fail=$((fail + 1))
    failed+=("$f")
    if [ "$fail" -le 3 ]; then
      echo "DIFERENCIA en $f:"
      diff "$tmp/rust.txt" "$tmp/titan.txt" | head -10
    fi
  fi
done
echo "archivos: ${#FILES[@]}  idénticos: $pass  distintos: $fail  (líneas idénticas: $lines)"
for f in "${failed[@]}"; do echo "  distinto: $f"; done
[ "$fail" -eq 0 ]
