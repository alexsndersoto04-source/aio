#!/bin/bash
# Compara std::json::parse/stringify sobre todos los casos de
# tests/native/json_casos (incluidos los errores, con su línea y columna)
# entre las dos formas de ejecutar un programa en este compilador:
#
#   - nativo: la sonda compilada con `titan compile` y ejecutada tal cual;
#   - VM:     la sonda compilada a bytecode (`titan build`) y ejecutada por
#             la VM del port (`titan exec`), que la vuelve a bajar a nativo.
#
# Antes esta comparación se hacía contra la VM de Rust (`zett`); al borrarse
# el Rust el guion se quedó sin poder correr. Ahora no depende de nada externo.
#
#   bash selfhost/native/verify_json.sh
#
# El trabajo se guarda en TITAN_JSON_DIR (por defecto /tmp/verify_json) y cada
# caso tiene TITAN_JSON_TIEMPO segundos (por defecto 60).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
dir=selfhost/tests/native/json_casos
W="${TITAN_JSON_DIR:-/tmp/verify_json}"
tiempo="${TITAN_JSON_TIEMPO:-60}"
mkdir -p "$W"

# 1. La sonda, una vez por cada motor.
if ! timeout "$tiempo" ./selfhost/titan compile "$dir/sonda.titan" -o "$W/sonda" > "$W/compile.out" 2>&1; then
  echo "no compila la sonda nativa:"
  tail -3 "$W/compile.out"
  exit 1
fi
if ! timeout "$tiempo" ./selfhost/titan build "$dir/sonda.titan" -o "$W/sonda.tbc" > "$W/build.out" 2>&1; then
  echo "no compila la sonda a bytecode:"
  tail -3 "$W/build.out"
  exit 1
fi

total=0
iguales=0
distintos=0
for f in "$dir"/*.json; do
  total=$((total + 1))
  nombre="$(basename "$f")"
  # 2. Las dos corridas, con la salida y el error juntos y el código al final
  #    (los casos de error se distinguen por el mensaje y por el código).
  a=$(timeout "$tiempo" "$W/sonda" "$f" 2>&1; echo "salida=$?")
  b=$(timeout "$tiempo" ./selfhost/titan exec "$W/sonda.tbc" "$f" 2>&1; echo "salida=$?")
  if [ "$a" == "$b" ]; then
    iguales=$((iguales + 1))
  else
    echo "DISTINTO $nombre"
    echo " nativo: $(echo "$a" | head -3 | tr '\n' '|')" | cut -c1-160
    echo " VM:     $(echo "$b" | head -3 | tr '\n' '|')" | cut -c1-160
    distintos=$((distintos + 1))
  fi
done
echo "casos JSON: $total  idénticos: $iguales  distintos: $distintos"
[ "$distintos" -eq 0 ]
