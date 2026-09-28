#!/bin/bash
# Punto fijo por LLVM.
#   1. El compilador en Titan (build.titan) y el backend LLVM (build_llvm.titan)
#      se traducen a LLVM IR con build_llvm, y LLVM de verdad los compila.
#   2. El compilador hecho por LLVM compila build.titan: el resultado debe ser
#      idéntico, byte a byte, al del compilador normal (titanc1 del punto fijo).
#   3. El backend LLVM hecho por LLVM traduce build.titan: el IR debe ser
#      idéntico al que escribe el build_llvm normal.
# Uso: bash selfhost/native/llvm/punto_fijo.sh   (desde cualquier sitio)
set -eu
ZETT="${ZETT:-zett}"
OPT="${OPT:--O2}"
cd "$(dirname "$0")/../.."          # selfhost/
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; rm -f titanc_llvm build_llvm_llvm' EXIT
if command -v clang >/dev/null 2>&1; then CC=(clang); else CC=(python3 -m ziglang cc -target x86_64-linux-none); fi
echo "LLVM: $("${CC[@]}" --version | head -1) ($OPT)"
[ -x build_llvm ] || "$ZETT" run build.titan build_llvm.titan build_llvm
echo "referencia: VM de Rust + build.titan -> titanc_ref"
"$ZETT" run build.titan build.titan "$tmp/titanc_ref"
echo "1. build.titan y build_llvm.titan -> LLVM IR -> ejecutables"
./build_llvm --lote "$tmp" build.titan build_llvm.titan
for p in build build_llvm; do [ -f "$tmp/$p.ll" ] || { cat "$tmp/$p.err"; exit 1; }; done
"${CC[@]}" $OPT -nostdlib -static -fno-pie -Wl,-e,_start "$tmp/build.ll" -o titanc_llvm 2>/dev/null
"${CC[@]}" $OPT -nostdlib -static -fno-pie -Wl,-e,_start "$tmp/build_llvm.ll" -o build_llvm_llvm 2>/dev/null
echo "2. compilador hecho por LLVM: build.titan -> titanc_por_llvm"
./titanc_llvm build.titan "$tmp/titanc_por_llvm"
mkdir -p "$tmp/otra"
echo "3. backend LLVM hecho por LLVM: build.titan -> build.ll"
./build_llvm_llvm --lote "$tmp/otra" build.titan
sha256sum "$tmp/titanc_ref" "$tmp/titanc_por_llvm" "$tmp/build.ll" "$tmp/otra/build.ll"
cmp "$tmp/titanc_ref" "$tmp/titanc_por_llvm"
cmp "$tmp/build.ll" "$tmp/otra/build.ll"
echo "PUNTO FIJO POR LLVM: mismo compilador y mismo IR (byte a byte)"
