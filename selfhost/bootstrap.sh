#!/bin/bash
# Construye Titan desde cero sin Rust, a partir de la semilla de selfhost/semilla/.
#
#   bash selfhost/bootstrap.sh
#
# 1. desempaqueta la semilla (selfhost/titanc0) y comprueba su SHA-256;
# 2. punto fijo: titanc0 -> titanc1 -> titanc2 -> titanc3 (selfhost/verify_fixpoint.sh);
# 3. compila la CLI `titan` (selfhost/titan.titan) con el compilador resultante -> selfhost/titan.
#
# Resultado: selfhost/titan (la CLI: new, check, run, test, compile, wasm, build, exec, add, fetch, lsp,
# dap, repl, debug...). El ejecutable busca el runtime en selfhost/native/ (junto a él), así que se
# usa desde esta carpeta o junto a una copia de native/.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
SEED=selfhost/semilla/titanc-linux-x86_64
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) ;;
  *) echo "la semilla es un ejecutable Linux x86-64; esta máquina es $(uname -s)-$(uname -m)" >&2; exit 2 ;;
esac
gzip -dc "$SEED.gz" > selfhost/titanc0
chmod +x selfhost/titanc0
want="$(cat "$SEED.sha256")"
got="$(sha256sum selfhost/titanc0 | cut -d' ' -f1)"
[ "$want" = "$got" ] || { echo "la semilla no coincide con su SHA-256 ($got != $want)" >&2; exit 1; }
bash selfhost/verify_fixpoint.sh
env -i PATH=/nonexistent ./selfhost/titanc3 selfhost/titan.titan selfhost/titan
echo "listo: selfhost/titan"
