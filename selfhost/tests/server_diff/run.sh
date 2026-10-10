#!/bin/bash
# Prueba diferencial de std::server: el mismo guion de cliente (client.py)
# contra la VM de Rust (el oráculo) y contra el binario nativo.
#
# Uso: bash selfhost/tests/server_diff/run.sh
#      ZETT=/ruta/zett  NATIVE=/ruta/binario-ya-compilado  bash ...
# Sin NATIVE, compila server.titan con selfhost/build.titan (varios minutos).
# Deja las salidas en $OUT (por omisión /tmp/server_diff): vm.server, vm.client,
# nat.server, nat.client; sale con 0 solo si las cuatro coinciden por pares.
set -u
ZETT="${ZETT:-zett}"
OUT="${OUT:-/tmp/server_diff}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
mkdir -p "$OUT"
NATIVE="${NATIVE:-$OUT/server}"
if [ ! -x "$NATIVE" ]; then
  "$ZETT" run selfhost/build.titan selfhost/tests/server_diff/server.titan "$NATIVE" || exit 2
fi
run_one() { # nombre, comando...
  local name="$1"; shift
  : > "$OUT/$name.server"
  "$@" > "$OUT/$name.server" 2>&1 &
  local pid=$!
  python3 selfhost/tests/server_diff/client.py "$OUT/$name.server" > "$OUT/$name.client" 2>&1
  local rc=$?
  for _ in $(seq 1 100); do kill -0 $pid 2>/dev/null || break; sleep 0.1; done
  kill $pid 2>/dev/null
  wait $pid 2>/dev/null
  return $rc
}
run_one vm "$ZETT" run selfhost/tests/server_diff/server.titan || echo "cliente (vm) terminó con error"
run_one nat "$NATIVE" || echo "cliente (nativo) terminó con error"
status=0
for k in server client; do
  if diff "$OUT/vm.$k" "$OUT/nat.$k" > "$OUT/$k.diff"; then
    echo "$k: idéntico ($(wc -l < "$OUT/vm.$k") líneas)"
  else
    echo "$k: DIFERENTE (ver $OUT/$k.diff)"; status=1
  fi
done
exit $status
