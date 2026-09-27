#!/bin/bash
# Punto fijo del bootstrap (fase 5 del self-hosting).
#
#   etapa 1: la VM de Rust ejecuta el compilador escrito en Titan
#            (selfhost/build.titan) sobre su propio código  -> titanc1
#   etapa 2: titanc1 (ejecutable nativo, sin Rust ni VM) se compila -> titanc2
#   etapa 3: titanc2 se compila                               -> titanc3
#
# Las tres deben ser idénticas byte a byte. La etapa 2 y la 3 se ejecutan con
# un entorno vacío (sin PATH), así que no pueden llamar a `zett` ni a nada.
# Los ejecutables quedan en selfhost/ (junto a selfhost/native/runtime.titan,
# que es donde el compilador busca el runtime) y git los ignora.
#
# Uso: bash selfhost/verify_fixpoint.sh [zett]
set -u
ZETT="${1:-zett}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
rm -f selfhost/titanc1 selfhost/titanc2 selfhost/titanc3
echo "etapa 1: VM de Rust + compilador Titan -> selfhost/titanc1"
"$ZETT" run selfhost/build.titan selfhost/build.titan selfhost/titanc1 || exit 1
[ -x selfhost/titanc1 ] || { echo "falló la etapa 1"; exit 1; }
echo "etapa 2: titanc1 -> selfhost/titanc2 (sin PATH)"
env -i PATH=/nonexistent ./selfhost/titanc1 selfhost/build.titan selfhost/titanc2 || exit 1
echo "etapa 3: titanc2 -> selfhost/titanc3 (sin PATH)"
env -i PATH=/nonexistent ./selfhost/titanc2 selfhost/build.titan selfhost/titanc3 || exit 1
sha256sum selfhost/titanc1 selfhost/titanc2 selfhost/titanc3
if cmp -s selfhost/titanc1 selfhost/titanc2 && cmp -s selfhost/titanc2 selfhost/titanc3; then
  echo "PUNTO FIJO: etapa1 == etapa2 == etapa3 (byte a byte)"
else
  echo "DISTINTOS: no hay punto fijo"
  exit 1
fi
