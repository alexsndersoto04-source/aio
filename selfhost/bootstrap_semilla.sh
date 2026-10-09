#!/bin/bash
# Semilla mínima para máquinas con poca memoria (4 GB).
#
# La VM de Rust no puede interpretar el compilador completo si el runtime incluye MySQL (pasa de 4 GB).
# Este script construye un compilador nativo "delgado" (selfhost/titanc0): igual que el completo pero
# con un sustituto vacío de std_mysql. Después se usa como semilla del punto fijo:
#
#   bash selfhost/bootstrap_semilla.sh            # necesita `zett` en el PATH; deja selfhost/titanc0
#   SEMILLA=selfhost/titanc0 bash selfhost/verify_fixpoint.sh
#
# El titanc0 NO es el compilador final (no sabe hablar con MySQL): el punto fijo lo reemplaza.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
RT=selfhost/native/runtime.titan
STUB=selfhost/native/std_mysql_stub.titan
cp "$RT" "$RT.orig"
trap 'mv -f "$RT.orig" "$RT"; rm -f "$STUB"' EXIT
python3 - "$STUB" <<'PY'
import sys
sig = {"connect": "url", "execute": "db: any, sql: any, params", "query": "db: any, sql: any, params",
       "begin": "db", "commit": "db", "rollback": "db", "migrate": "db: any, migs", "last_id": "db",
       "close": "db", "ping": "db", "pool_new": "url: any, maximum", "pool_acquire": "pool: any, timeout",
       "pool_stats": "pool", "pool_health": "pool: any, timeout", "pool_close": "pool"}
out = ["// Sustituto vacío de std_mysql: solo lo usa bootstrap_semilla.sh\n"]
for n, a in sig.items():
    a = a if ":" in a else a + ": any"
    out.append('fn rt_my_%s(%s) -> any {\n    rt_fatal("MySQL no disponible en la semilla")\n}\n' % (n, a))
out.append('fn my_db_exec(db: any, sql: any, params: any, mode: int) -> any {\n    rt_fatal("MySQL no disponible en la semilla")\n}\n')
out.append('fn my_db_migrate(db: any, migs: any) -> any {\n    rt_fatal("MySQL no disponible en la semilla")\n}\n')
open(sys.argv[1], "w").write("".join(out))
PY
sed -i 's/^import std_mysql$/import std_mysql_stub/' "$RT"
zett run selfhost/build.titan selfhost/build.titan selfhost/titanc0
echo "listo: selfhost/titanc0 (semilla delgada)"
