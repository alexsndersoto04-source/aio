#!/usr/bin/env bash
# Regenera selfhost/natives.titan a partir del registro de nativas del
# compilador actual (`titan natives`). Es DATO, no lógica: la tabla de firmas
# (nombre, parámetros, resultado) con la que el typechecker comprueba las
# llamadas a std::*. Temporal hasta la fase de la stdlib en Titan, cuando la
# tabla se generará desde las propias nativas escritas en Titan.
# `selfhost/natives_titan.txt` lista las nativas que existen solo en Titan (no en el registro de
# Rust): se añaden al final de la tabla.
# Uso: bash selfhost/gen_natives.sh [--check]
set -euo pipefail
cd "$(dirname "$0")/.."
out=selfhost/natives.titan
tmp=$(mktemp)
{
  echo "// GENERADO por selfhost/gen_natives.sh desde \`titan natives\`. No editar a mano."
  echo "// Una línea por nativa: \"nombre Param,Param -> Resultado\"."
  echo "fn native_table() -> [string] {"
  echo "    ["
  # `std::audio::cloud_*` (Telegram/MTProto) se eliminó: no se puede verificar de verdad sin servicio real.
  { titan natives | grep -v '^std::audio::cloud_'; cat selfhost/natives_titan.txt; } | sed 's/\\/\\\\/g; s/"/\\"/g; s/.*/        "&",/'
  echo "    ]"
  echo "}"
} > "$tmp"
if [ "${1:-}" = "--check" ]; then
  if cmp -s "$tmp" "$out"; then echo "natives.titan: sincronizado"; rm -f "$tmp"; exit 0; fi
  echo "natives.titan: DESINCRONIZADO (ejecuta bash selfhost/gen_natives.sh)"; rm -f "$tmp"; exit 1
fi
mv "$tmp" "$out"
echo "escrito $out ($(grep -c '^        "' "$out") nativas)"
