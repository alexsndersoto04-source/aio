#!/bin/bash
# Verificación diferencial del compilador nativo (fase 4b del self-hosting).
#
# Para cada programa: lo compila a ejecutable con `selfhost/build.titan`
# (todo en Titan), lo ejecuta y compara salida estándar, salida de errores y
# código de salida con `zett run` (la VM de Rust).
# Si el compilador nativo todavía no admite algo, lo dice ("no admitido") y
# se cuenta aparte: nunca como acierto.
#
# Uso: bash selfhost/native/verify_native.sh [archivos...]
#      (sin archivos: selfhost/tests/native/*.titan)
#      ZETT=/ruta/al/zett COMPILER=/ruta/a/titanc1 bash ...
# COMPILER permite reutilizar el compilador nativo ya creado en el bootstrap,
# evitando volver a interpretar build.titan con Rust para cada prueba.
set -u
ZETT="${ZETT:-zett}"
COMPILER="${COMPILER:-}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if [ $# -gt 0 ]; then FILES=("$@"); else FILES=(selfhost/tests/native/*.titan); fi
pass=0; fail=0; unsupported=0; current=0
total=${#FILES[@]}
for f in "${FILES[@]}"; do
  current=$((current + 1))
  echo "[$current/$total] $f"
  if [ -n "$COMPILER" ]; then
    "$COMPILER" "$f" "$tmp/prog" > "$tmp/build.txt" 2>&1
  else
    "$ZETT" run selfhost/build.titan "$f" "$tmp/prog" > "$tmp/build.txt" 2>&1
  fi
  if [ ! -x "$tmp/prog" ]; then
    unsupported=$((unsupported + 1))
    echo "no admitido: $f: $(head -c 300 "$tmp/build.txt" | tr '\n' ' ')"
    continue
  fi
  timeout "${TEST_TIMEOUT:-60}" "$ZETT" run "$f" > "$tmp/vm.out" 2> "$tmp/vm.err"; vm=$?
  timeout "${TEST_TIMEOUT:-60}" "$tmp/prog" > "$tmp/nat.out" 2> "$tmp/nat.err"; nat=$?
  rm -f "$tmp/prog"
  if cmp -s "$tmp/vm.out" "$tmp/nat.out" && cmp -s "$tmp/vm.err" "$tmp/nat.err" && [ "$vm" = "$nat" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "DIFERENCIA en $f (salida VM=$vm nativo=$nat):"
    diff "$tmp/vm.out" "$tmp/nat.out" | head -8
    diff "$tmp/vm.err" "$tmp/nat.err" | head -4
  fi
done
echo "programas: $total  idénticos: $pass  distintos: $fail  no admitidos: $unsupported"
[ "$fail" -eq 0 ] && [ "$unsupported" -eq 0 ]
