#!/bin/bash
# Backend LLVM para ARM64, parte 2 (en una máquina ARM64 real): enlaza cada
# objeto de arm64_objetos.sh, lo ejecuta y compara salida estándar, salida de
# errores y código de salida con la VM de Rust (compilada para ARM64).
#
# Uso: bash selfhost/native/llvm/arm64_comparar.sh OBJETOS [archivos...]
set -u
ZETT="${ZETT:-zett}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
objs="$(realpath "$1")"; shift
cd "$ROOT"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if [ $# -gt 0 ]; then FILES=("$@"); else FILES=(selfhost/tests/native/*.titan); fi
echo "máquina: $(uname -m)"
pass=0; fail=0; unsupported=0
for f in "${FILES[@]}"; do
  base="$(basename "$f" .titan)"
  if [ -f "$objs/$base.err" ]; then
    unsupported=$((unsupported + 1)); echo "no admitido: $f: $(head -c 300 "$objs/$base.err" | tr '\n' ' ')"; continue
  fi
  if [ ! -f "$objs/$base.o" ]; then
    fail=$((fail + 1)); echo "DIFERENCIA en $f: no hay objeto ARM64"; continue
  fi
  rm -f "$tmp/prog"
  if ! ld -static -e _start -o "$tmp/prog" "$objs/$base.o" > "$tmp/ld.txt" 2>&1; then
    fail=$((fail + 1)); echo "DIFERENCIA en $f: no enlaza: $(head -c 300 "$tmp/ld.txt" | tr '\n' ' ')"; continue
  fi
  timeout 60 "$ZETT" run "$f" > "$tmp/vm.out" 2> "$tmp/vm.err"; vm=$?
  timeout 60 "$tmp/prog" > "$tmp/nat.out" 2> "$tmp/nat.err"; nat=$?
  if cmp -s "$tmp/vm.out" "$tmp/nat.out" && cmp -s "$tmp/vm.err" "$tmp/nat.err" && [ "$vm" = "$nat" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "DIFERENCIA en $f (salida VM=$vm ARM64=$nat):"
    diff "$tmp/vm.out" "$tmp/nat.out" | head -8
    diff "$tmp/vm.err" "$tmp/nat.err" | head -4
  fi
done
echo "programas: ${#FILES[@]}  idénticos: $pass  distintos: $fail  no admitidos: $unsupported"
[ "$fail" -eq 0 ] && [ "$unsupported" -eq 0 ]
