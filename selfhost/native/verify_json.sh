#!/usr/bin/env bash
# Compara std::json::parse/stringify de la VM con el ejecutable nativo sobre
# todos los casos de tests/native/json_casos (incluidos los errores, con su
# línea y columna). El programa sonda se compila una vez a nativo.
set -u
cd "$(dirname "$0")/../.."
export PATH=$HOME/.local/bin:$PATH
dir=selfhost/tests/native/json_casos
tmp=$(mktemp -d)
zett run selfhost/build.titan $dir/sonda.titan $tmp/sonda >/dev/null || { echo "no compila la sonda"; exit 1; }
total=0; iguales=0
for f in $dir/*.json; do
  total=$((total+1))
  a=$(zett run $dir/sonda.titan $f 2>&1; echo "salida=$?")
  b=$($tmp/sonda $f 2>&1; echo "salida=$?")
  if [ "$a" == "$b" ]; then iguales=$((iguales+1)); else echo "DISTINTO $f"; echo " VM:     $a" | head -3; echo " nativo: $b" | head -3; fi
done
echo "casos JSON: $total  idénticos: $iguales"
rm -rf $tmp
