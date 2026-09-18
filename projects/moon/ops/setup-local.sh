#!/bin/bash
# Postgres embebido local para moon: instala el paquete (si falta),
# crea el cluster y la base de datos. Idempotente.
set -e
OPS="$(cd "$(dirname "$0")" && pwd)"
PGDIR="$OPS/pg"
mkdir -p "$PGDIR"
cd "$PGDIR"
if [ ! -d node_modules/@embedded-postgres/linux-x64 ]; then
  npm install --no-fund --no-audit @embedded-postgres/linux-x64
fi
NATIVE="$PGDIR/node_modules/@embedded-postgres/linux-x64/native"
if [ ! -f data/PG_VERSION ]; then
  "$NATIVE/bin/initdb" -D data --username=moon --auth=trust -E UTF8 >/dev/null
fi
if ! "$NATIVE/bin/pg_ctl" -D data -l pg.log status >/dev/null 2>&1; then
  "$NATIVE/bin/pg_ctl" -D data -l pg.log start
  sleep 1
fi
# Cliente Node (para db.mjs) en ops/node_modules (gitignoreado).
if [ ! -d "$OPS/node_modules/pg" ]; then
  (cd "$OPS" && npm install --no-fund --no-audit pg)
fi
# Base de datos moon (si no existe).
node "$OPS/db.mjs" create-db
echo "DATABASE_URL=postgres://moon@127.0.0.1:5432/moon"
