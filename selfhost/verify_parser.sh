#!/bin/bash
# Verificación diferencial del parser escrito en Titan (fase 2 del self-hosting).
#
# Para cada archivo .titan del repositorio (y los casos límite de
# selfhost/tests/) compara, byte por byte:
#   - `zett ast ARCHIVO`                       → parser de Rust (oráculo)
#   - `zett run selfhost/ast.titan ARCHIVO`   → parser escrito en Titan
#
# Uso: bash selfhost/verify_parser.sh [zett]
set -u
ZETT="${1:-zett}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

mapfile -t FILES < <(
  { git ls-files '*.titan'; ls selfhost/tests/lexer/*.titan selfhost/tests/parser/*.titan; } | sort -u
)

pass=0
fail=0
lines=0
failed=()
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
for f in "${FILES[@]}"; do
  "$ZETT" ast "$f" > "$tmp/rust.txt" 2>&1
  "$ZETT" run selfhost/ast.titan "$f" > "$tmp/titan.txt" 2>&1
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
echo "archivos: ${#FILES[@]}  idénticos: $pass  distintos: $fail  (líneas de AST idénticas: $lines)"
for f in "${failed[@]}"; do echo "  distinto: $f"; done
[ "$fail" -eq 0 ]
