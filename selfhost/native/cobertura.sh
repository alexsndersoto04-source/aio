#!/bin/bash
# Cobertura de la biblioteca estándar en el runtime nativo (fase 6).
# Compara la lista completa de nativas (selfhost/natives.titan, 816) con las
# funciones `std__modulo__nombre` que ya existen en native/runtime.titan (y
# los módulos que importa). Uso: bash selfhost/native/cobertura.sh [-v]
#   -v: lista también las que faltan, por módulo.
set -u
cd "$(dirname "$0")/.."
all=$(grep -o '"std::[a-z_0-9]*::[a-z_0-9]*' natives.titan | tr -d '"' | sort -u)
have=$(cat native/*.titan | grep -o '^fn std__[a-z0-9_]*' | sed 's/^fn //' | sort -u)
# Nativas que el compilador convierte en una instrucción propia (no en una
# función del runtime): cuentan si los DOS backends traducen esa instrucción.
for pair in std__try__catch:TryCall; do
  f=${pair%%:*}; op=${pair#*:}
  if grep -q "k == \"$op\"" native/backend.titan && grep -q "k == \"$op\"" native/llvm.titan; then
    have=$(printf '%s\n%s\n' "$have" "$f" | sort -u)
  fi
done
total=0; done=0
declare -A mt md
for n in $all; do
  m=${n#std::}; m=${m%%::*}
  f=$(echo "$n" | sed 's/::/__/g')
  total=$((total+1)); mt[$m]=$(( ${mt[$m]:-0} + 1 ))
  if echo "$have" | grep -qx "$f"; then done=$((done+1)); md[$m]=$(( ${md[$m]:-0} + 1 )); fi
done
for m in $(printf '%s\n' "${!mt[@]}" | sort); do
  printf '%-22s %3d / %3d\n' "$m" "${md[$m]:-0}" "${mt[$m]}"
done
echo "TOTAL: $done / $total nativas escritas en Titan"
if [ "${1:-}" = "-v" ]; then
  echo "--- faltan:"
  for n in $all; do f=$(echo "$n" | sed 's/::/__/g'); echo "$have" | grep -qx "$f" || echo "  $n"; done
fi
