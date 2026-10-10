#!/bin/bash
# Verificación diferencial de `titan build` / `titan exec` frente a `zett`:
#   1. `titan build` y `zett build` producen el MISMO .tbc (cmp byte a byte) y el
#      mismo resumen por la salida estándar, para cada programa de programs/.
#   2. `titan exec` de un .tbc hecho por zett da la misma salida y código que
#      `zett exec` (los programas que el backend nativo aún no admite se cuentan
#      aparte, nunca como aciertos).
#   3. Los artefactos inválidos de gen_bad.py se rechazan con el mismo mensaje y
#      código de salida (los errores de serde -JSON truncado, variante
#      desconocida- se comparan solo por la etiqueta INVALID ARTIFACT).
# Uso: TITAN=selfhost/titan ZETT=zett bash selfhost/tests/tbc_diff/run.sh
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
TITAN="${TITAN:-$HERE/../../titan}"
ZETT="${ZETT:-zett}"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
mkdir -p "$W/z" "$W/t" "$W/bad"
fail=0
for f in "$HERE"/programs/*.titan; do
  n=$(basename "$f" .titan)
  if ! "$ZETT" build "$f" -o "$W/z/$n.tbc" >"$W/z/$n.out" 2>&1; then
    echo "programa inválido: $n"; head -2 "$W/z/$n.out"; fail=1; continue
  fi
  "$TITAN" build "$f" -o "$W/t/$n.tbc" >"$W/t/$n.out" 2>&1
  sed -i "s#$W/z/##" "$W/z/$n.out"; sed -i "s#$W/t/##" "$W/t/$n.out"
  if cmp -s "$W/z/$n.tbc" "$W/t/$n.tbc" && cmp -s "$W/z/$n.out" "$W/t/$n.out"; then
    echo "build OK     $n"
  else
    echo "build DIFF   $n"; fail=1
  fi
  "$ZETT" exec "$W/z/$n.tbc" >"$W/z/$n.so" 2>"$W/z/$n.se"; zr=$?
  "$TITAN" exec "$W/z/$n.tbc" >"$W/t/$n.so" 2>"$W/t/$n.se"; tr=$?
  if grep -q "not supported by the native backend" "$W/t/$n.se"; then
    echo "exec UNSUP   $n ($(head -1 "$W/t/$n.se"))"
  elif cmp -s "$W/z/$n.so" "$W/t/$n.so" && cmp -s "$W/z/$n.se" "$W/t/$n.se" && [ "$zr" = "$tr" ]; then
    echo "exec OK      $n"
  else
    echo "exec DIFF    $n ($zr vs $tr)"; fail=1
  fi
done
python3 "$HERE/gen_bad.py" "$W/z/a_hello.tbc" "$W/bad"
for f in "$W"/bad/*.tbc; do
  n=$(basename "$f" .tbc)
  "$ZETT" exec "$f" >"$W/z/$n.bo" 2>"$W/z/$n.be"; zr=$?
  "$TITAN" exec "$f" >"$W/t/$n.bo" 2>"$W/t/$n.be"; tr=$?
  case "$n" in
    trunc|garbage) same=$(head -c 22 "$W/z/$n.be" | cmp -s - <(head -c 22 "$W/t/$n.be") && echo 1) ;;
    *) same=$(cmp -s "$W/z/$n.be" "$W/t/$n.be" && echo 1) ;;
  esac
  if [ "$same" = 1 ] && [ "$zr" = "$tr" ] && [ "$zr" != 0 ]; then
    echo "bad OK       $n"
  else
    echo "bad DIFF     $n ($zr vs $tr): $(head -1 "$W/z/$n.be") | $(head -1 "$W/t/$n.be")"; fail=1
  fi
done
if [ "$fail" = 0 ]; then echo "TBC: TODO OK"; else echo "TBC: HAY DIFERENCIAS"; exit 1; fi
