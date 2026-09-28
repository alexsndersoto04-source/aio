#!/bin/bash
# Backend LLVM para ARM64, parte 1 (en una máquina x86-64): el compilador en
# Titan escribe el LLVM IR de cada programa para AArch64 y LLVM de verdad lo
# compila a código objeto ARM64 (.o). La parte 2 (arm64_comparar.sh) enlaza y
# ejecuta en una máquina ARM64 real.
#
# Uso: bash selfhost/native/llvm/arm64_objetos.sh SALIDA [archivos...]
set -u
ZETT="${ZETT:-zett}"
OPT="${OPT:--O2}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
out="$(realpath -m "$1")"; shift
cd "$ROOT"
if [ $# -gt 0 ]; then FILES=("$@"); else FILES=(selfhost/tests/native/*.titan); fi
mkdir -p "$out"
if [ ! -x selfhost/build_llvm ] || [ -n "$(find selfhost -name '*.titan' -newer selfhost/build_llvm | head -1)" ]; then
  (cd selfhost && "$ZETT" run build.titan build_llvm.titan build_llvm) || exit 1
fi
if command -v clang >/dev/null 2>&1; then CC=(clang); else CC=(python3 -m ziglang cc); fi
echo "LLVM: $("${CC[@]}" --version | head -1) ($OPT, aarch64)"
./selfhost/build_llvm --lote-aarch64 "$out" "${FILES[@]}"
n=0; bad=0
for f in "${FILES[@]}"; do
  base="$(basename "$f" .titan)"
  [ -f "$out/$base.ll" ] || continue
  if "${CC[@]}" --target=aarch64-linux-gnu $OPT -c "$out/$base.ll" -o "$out/$base.o" 2> "$out/$base.cc.txt"; then
    n=$((n + 1)); rm -f "$out/$base.cc.txt"
  else
    bad=$((bad + 1)); echo "LLVM rechazó $f: $(head -c 300 "$out/$base.cc.txt" | tr '\n' ' ')"
  fi
  rm -f "$out/$base.ll"
done
echo "objetos ARM64: $n  rechazados: $bad"
[ "$bad" -eq 0 ]
