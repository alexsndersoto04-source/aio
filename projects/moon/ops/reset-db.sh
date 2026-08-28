#!/bin/bash
# Vacía la BD local de moon (drop de todas las tablas).
# Uso: bash ops/reset-db.sh
OPS="$(cd "$(dirname "$0")" && pwd)"
NATIVE="$OPS/pg/node_modules/@embedded-postgres-linux-x64/native"
[ -x "$NATIVE/bin/psql" ] || { echo "Falta Postgres: ejecuta bash ops/setup-local.sh"; exit 1; }
"$NATIVE/bin/psql" -h 127.0.0.1 -p 5432 -U moon -d moon -v ON_ERROR_STOP=1 <<'SQL'
DO $$
DECLARE r record;
BEGIN
  FOR r IN SELECT tablename FROM pg_tables WHERE schemaname = 'public'
  LOOP
    EXECUTE 'DROP TABLE IF EXISTS public.' || quote_ident(r.tablename) || ' CASCADE';
  END LOOP;
END $$;
SQL
echo "BD vaciada."
