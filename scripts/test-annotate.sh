#!/usr/bin/env bash
# Diagnostico TEMPORAL: ejecuta un binario de pruebas y, si falla, publica el
# final de su salida (donde libtest lista las pruebas rotas) como anotacion de
# GitHub Actions, que si se puede leer sin acceso a los logs del workflow.
# Se activa con `runner` para el target en .cargo/config.toml.
set -uo pipefail

OUT="$(mktemp)"
"$@" > "$OUT" 2>&1
CODE=$?
cat "$OUT"
if [ "$CODE" -ne 0 ] && [ -n "${GITHUB_ACTIONS:-}" ]; then
  FLAT="$(tail -c 2600 "$OUT" | sed 's/%/%25/g' | tr '\n' '|' | tr -d '\r')"
  echo "::error title=TEST_DIAG::${FLAT}"
fi
rm -f "$OUT"
exit "$CODE"
