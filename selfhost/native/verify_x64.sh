#!/bin/bash
# Comprueba el ensamblador de Titan (selfhost/native/x64.titan) contra el
# desensamblador de binutils: los bytes que genera Titan, leídos por objdump,
# deben dar exactamente las instrucciones esperadas.
set -eu
ZETT="${1:-zett}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$ROOT/selfhost/native"
"$ZETT" run x64_test.titan "$tmp/code.bin" "$tmp/want.txt"
objdump -D -b binary -m i386:x86-64 -M intel "$tmp/code.bin" \
  | sed -n 's/^ *[0-9a-f]*:\t[0-9a-f ]*\t//p' | sed 's/ \+/ /g; s/ *$//' > "$tmp/got.txt"
if diff "$tmp/want.txt" "$tmp/got.txt" > "$tmp/diff.txt"; then
  echo "x64: $(wc -l < "$tmp/want.txt") instrucciones, todas idénticas a objdump"
else
  head -20 "$tmp/diff.txt"
  echo "x64: DIFERENCIAS"
  exit 1
fi
