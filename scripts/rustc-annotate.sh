#!/usr/bin/env bash
# Diagnostico TEMPORAL: envuelve a rustc y, si falla, publica su error como
# anotacion de GitHub Actions (un canal que se puede leer sin acceso a los
# logs del workflow). Se activa con `rustc-wrapper` en .cargo/config.toml.
set -uo pipefail

OUT="$(mktemp)"
rustc "$@" > "$OUT" 2>&1
CODE=$?
cat "$OUT"
if [ "$CODE" -ne 0 ] && [ -n "${GITHUB_ACTIONS:-}" ]; then
  FLAT="$(sed 's/%/%25/g' "$OUT" | tr '\n' '|' | tr -d '\r' | cut -c1-3000)"
  echo "::error title=RUSTC_DIAG::${FLAT}"
fi
rm -f "$OUT"
exit "$CODE"
