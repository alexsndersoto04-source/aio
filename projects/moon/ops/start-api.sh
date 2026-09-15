#!/bin/bash
# Arranca la API de moon con la BD local (setup-local.sh) y el binario
# zett descargado en projects/moon/bin/zett.
# Uso: bash ops/start-api.sh   (desde projects/moon o desde donde sea)
set -e
OPS="$(cd "$(dirname "$0")" && pwd)"
MOON="$OPS/.."
ZETT="$MOON/bin/zett"
if [ ! -x "$ZETT" ]; then
  echo "Falta el binario: $ZETT"
  echo "(Se descarga de la rama tools-zett-x86_64 del repo, ver LOCAL.md)"
  exit 1
fi
cd "$MOON"
export DATABASE_URL="${DATABASE_URL:-postgres://moon@127.0.0.1:5432/moon}"
export PORT="${PORT:-3000}"
export CORS_ORIGIN="${CORS_ORIGIN:-http://localhost:5173,http://127.0.0.1:5173}"
export PUBLIC_BASE_URL="${PUBLIC_BASE_URL:-http://127.0.0.1:3000}"
# JWT_SECRET: se toma del entorno o se genera (64 hex) si no existe.
if [ -z "$JWT_SECRET" ]; then
  if [ -f "$OPS/.jwt-secret" ]; then
    JWT_SECRET="$(cat "$OPS/.jwt-secret")"
  else
    JWT_SECRET=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
    printf '%s' "$JWT_SECRET" > "$OPS/.jwt-secret"
    chmod 600 "$OPS/.jwt-secret"
  fi
  export JWT_SECRET
fi
exec "$ZETT" run src/main.titan
