#!/bin/bash
# Verificación diferencial del lexer escrito en Titan (fase 1 del self-hosting).
#
# Para cada archivo .titan del repositorio (y los casos límite de
# selfhost/tests/lexer/) compara, byte por byte:
#   - `zett tokens ARCHIVO`                       → lexer de Rust (oráculo)
#   - `zett run selfhost/tokens.titan ARCHIVO`   → lexer escrito en Titan
#
# Uso: bash selfhost/verify_lexer.sh [zett]
set -u
ZETT="${1:-zett}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

mapfile -t FILES < <(
  { git ls-files '*.titan'; ls selfhost/tests/lexer/*.titan; } | sort -u
)

pass=0
fail=0
failed=()
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
for f in "${FILES[@]}"; do
  "$ZETT" tokens "$f" > "$tmp/rust.txt" 2>&1
  "$ZETT" run selfhost/tokens.titan "$f" > "$tmp/titan.txt" 2>&1
  if cmp -s "$tmp/rust.txt" "$tmp/titan.txt"; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    failed+=("$f")
    if [ "$fail" -le 3 ]; then
      echo "DIFERENCIA en $f:"
      diff "$tmp/rust.txt" "$tmp/titan.txt" | head -10
    fi
  fi
done
echo "archivos: ${#FILES[@]}  idénticos: $pass  distintos: $fail"
for f in "${failed[@]}"; do echo "  distinto: $f"; done
[ "$fail" -eq 0 ]
