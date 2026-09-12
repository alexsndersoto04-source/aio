#!/bin/bash
# Vacía la BD local de moon (drop de todas las tablas de public,
# incluyendo la tabla de migraciones: la API recrea todo al arrancar).
OPS="$(cd "$(dirname "$0")" && pwd)"
[ -d "$OPS/node_modules/pg" ] || { echo "Falta el cliente: bash ops/setup-local.sh"; exit 1; }
node "$OPS/db.mjs" reset
