#!/bin/bash
# Compara la VM de Rust (`zett run`, crate `mysql`) con el nativo (`std_mysql.titan`) contra el servidor
# de prueba servidor.py (mysql-mimic + SQLite: NO es MySQL, ver ESTADO.md).
#   COMPILER=selfhost/titanc3 bash selfhost/tests/mysql/comparar.sh [archivo.titan ...]
# Cada prueba declara en su primera línea de comentarios `// puerto: N` y, si hace falta,
# `// servidor: <argumentos de servidor.py>`. Necesita PYLIB (mysql_mimic + sqlglot) y `zett`.
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
DIR="$ROOT/selfhost/tests/mysql"
COMPILER="${COMPILER:-$ROOT/selfhost/titanc3}"
ZETT="${ZETT:-zett}"
TMP="${TMPDIR:-/tmp}/mysql_cmp"
mkdir -p "$TMP"
if [ $# -eq 0 ]; then set -- "$DIR"/*.titan; fi
fallos=0
for f in "$@"; do
  nombre="$(basename "$f" .titan)"
  puerto="$(grep -m1 '^// puerto:' "$f" | sed 's/.*: *//')"
  extra="$(grep -m1 '^// servidor:' "$f" | sed 's/^\/\/ servidor: *//')"
  arrancar() {
    python3 "$DIR/servidor.py" "$puerto" $extra > "$TMP/$nombre.srv.log" 2>&1 &
    srv=$!
    for _ in $(seq 1 50); do (echo > "/dev/tcp/127.0.0.1/$puerto") 2>/dev/null && break; sleep 0.2; done
  }
  parar() { kill $srv 2>/dev/null; wait $srv 2>/dev/null; }
  # un servidor nuevo para cada cliente: las cachés de autenticación y las tablas empiezan vacías
  arrancar
  "$ZETT" run "$f" > "$TMP/$nombre.vm.out" 2>&1
  parar
  if ! "$COMPILER" "$f" "$TMP/$nombre.bin" > "$TMP/$nombre.build.log" 2>&1; then
    echo "$nombre: NO COMPILA"; cat "$TMP/$nombre.build.log"; fallos=$((fallos+1)); continue
  fi
  arrancar
  "$TMP/$nombre.bin" > "$TMP/$nombre.nat.out" 2>&1
  parar
  if diff -q "$TMP/$nombre.vm.out" "$TMP/$nombre.nat.out" > /dev/null; then
    echo "$nombre: IGUAL ($(wc -l < "$TMP/$nombre.vm.out") líneas)"
  else
    echo "$nombre: DISTINTO"; diff "$TMP/$nombre.vm.out" "$TMP/$nombre.nat.out" | head -40; fallos=$((fallos+1))
  fi
done
echo "fallos: $fallos"
[ "$fallos" -eq 0 ]
