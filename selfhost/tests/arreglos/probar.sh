#!/bin/bash
# Pruebas de los arreglos del 2026-10-10 (ver selfhost/ESTADO.md). Sin
# dependencias: solo hace falta el compilador selfhost/titan construido.
#
#   bash selfhost/tests/arreglos/probar.sh
#
# 1. selfhost/prueba_arreglos.titan tiene que compilar y ejecutarse con código 0
#    (tipos de bucles, asignación en posición de sentencia, asignación como
#    valor y la decisión de arquitectura).
# 2. Los archivos de rechazo tienen que seguir fallando con el mensaje exacto.
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
TITAN="${TITAN:-selfhost/titan}"

if [ ! -x "$TITAN" ]; then
  echo "probar.sh: no se encuentra $TITAN (ejecuta antes: bash selfhost/bootstrap.sh)" >&2
  exit 2
fi

fail=0

echo "== compilación de la prueba =="
if ! "$TITAN" check selfhost/prueba_arreglos.titan; then
  echo "FALLO: selfhost/prueba_arreglos.titan no compila" >&2
  fail=1
fi

echo "== rechazos que deben seguir fallando =="
check_reject() {
  local file="$1" expect="$2" out
  if out=$("$TITAN" check "$file" 2>&1); then
    echo "FALLO: $file se aceptó y tenía que rechazarse" >&2
    fail=1
    return
  fi
  if ! printf '%s' "$out" | grep -qF "$expect"; then
    echo "FALLO: $file rechazó, pero sin el mensaje esperado [$expect]:" >&2
    printf '%s\n' "$out" | head -3 >&2
    fail=1
    return
  fi
  echo "ok: $file → $expect"
}
check_reject selfhost/tests/arreglos/rechazo_ramas_valor.titan "type mismatch: expected Int, found String"
check_reject selfhost/tests/arreglos/rechazo_cola_fn_ramas.titan "type mismatch: expected Int, found String"
check_reject selfhost/tests/arreglos/rechazo_cola_fn_asignacion.titan "type mismatch: expected Nil, found Array(Int)"
check_reject selfhost/tests/arreglos/rechazo_asignacion_anotada.titan "type mismatch: expected String, found Int"

echo "== ejecución de la prueba =="
if ! out=$("$TITAN" run selfhost/prueba_arreglos.titan 2>&1); then
  echo "FALLO: selfhost/prueba_arreglos.titan falló al ejecutarse:" >&2
  printf '%s\n' "$out" | head -20 >&2
  fail=1
elif [ "$out" != "arreglos: todo bien" ]; then
  echo "FALLO: salida inesperada de selfhost/prueba_arreglos.titan:" >&2
  printf '%s\n' "$out" | head -20 >&2
  fail=1
else
  echo "ok: arreglos: todo bien"
fi

if [ "$fail" -eq 0 ]; then
  echo "probar.sh: todo bien"
fi
exit "$fail"
