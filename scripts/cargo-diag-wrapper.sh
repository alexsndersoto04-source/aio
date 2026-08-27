#!/bin/bash
# Wrapper de diagnostico para rustc en CI.
#
# Captura la salida de cada invocacion de rustc; si alguna FALLA y hay
# CI + GITHUB_TOKEN, publica el error por dos canales legibles sin acceso
# a las logs de Actions:
#   1. rama `tools-rustc-diag` (archivo diag.txt = ultima falla)
#   2. anotacion de GitHub `RUSTC_DIAG` (texto codificado %0A)
#
# En exito es transparente (solo repite la salida). Sin recursividad:
# cargo invoca al wrapper; el rustc real nunca invoca al wrapper.

OUT="$(mktemp /tmp/zett-rustc.XXXXXX)" || OUT="/tmp/zett-rustc.$$"
rustc "$@" > "$OUT" 2>&1
code=$?
if [ "$code" -ne 0 ] && [ -n "$CI" ] && [ -n "$GITHUB_TOKEN" ]; then
  TEXT=$(tail -n 80 "$OUT" | sed 's/%/%25/g; s/\r//g')
  ENC=$(printf '%s' "$TEXT" | awk 'NR>1{printf "%%0A"} {printf "%s", $0}')
  W=$(mktemp -d /tmp/zett-rustc-diag.XXXXXX) || W="/tmp/zett-rustc-diag.$$"
  git -C "$W" init -q 2>/dev/null
  printf '%s\n' "$TEXT" > "$W/diag.txt"
  git -C "$W" add -A 2>/dev/null
  git -C "$W" -c user.name=ci-diag -c user.email=ci@local commit -q -m diag 2>/dev/null
  git -C "$W" push -f -q "https://x-access-token:${GITHUB_TOKEN}@github.com/alexsndersoto04-source/aio.git" HEAD:refs/heads/tools-rustc-diag 2>/dev/null
  echo "::error title=RUSTC_DIAG::${ENC}"
fi
cat "$OUT"
rm -f "$OUT"
exit "$code"
