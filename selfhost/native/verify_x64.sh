#!/bin/bash
# Comprueba el ensamblador de Titan (selfhost/native/x64.titan) contra el
# desensamblador de binutils: los bytes que genera Titan, leídos por objdump,
# deben dar exactamente las instrucciones esperadas. El programa de prueba se
# ejecuta con la CLI del port (antes se ejecutaba con la VM de Rust).
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# La CLI del port (el codificador está en Titan; el oráculo es objdump, así que
# no hace falta nada más). Se puede cambiar con TITAN= o con el primer argumento.
TITAN="${1:-${TITAN:-$ROOT/selfhost/titan}}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$ROOT/selfhost/native"
"$TITAN" run x64_test.titan "$tmp/code.bin" "$tmp/want.txt"
objdump -D -b binary -m i386:x86-64 -M intel "$tmp/code.bin" \
  | sed -n 's/^ *[0-9a-f]*:\t[0-9a-f ]*\t//p' | sed 's/ \+/ /g; s/ *$//' > "$tmp/got.txt"
if diff "$tmp/want.txt" "$tmp/got.txt" > "$tmp/diff.txt"; then
  echo "x64: $(wc -l < "$tmp/want.txt") instrucciones, todas idénticas a objdump"
else
  head -20 "$tmp/diff.txt"
  echo "x64: DIFERENCIAS"
  exit 1
fi
