#!/bin/bash
# Pruebas de las fallitas 4, 5 y 6 (ver selfhost/ESTADO.md):
#   #4 los errores dentro de `main` salen con la posición real, no 1:1;
#   #5 los errores de declaración citan el archivo donde están, no el de la
#      última función revisada;
#   #6 cada error se imprime UNA sola vez;
#   más los destinos de asignación (self.campo, campo, índice), cada uno con su mensaje.
#
#   bash selfhost/tests/errores/probar.sh
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
TITAN="${1:-${TITAN:-selfhost/titan}}"

if [ ! -x "$TITAN" ]; then
  echo "probar.sh: no se encuentra $TITAN (ejecuta antes: bash selfhost/bootstrap.sh)" >&2
  exit 2
fi

fail=0

echo "== fallita 4: posición real de los errores dentro de main =="
out=$("$TITAN" check selfhost/tests/errores/posicion.titan 2>&1) && {
  echo "FALLO: posicion.titan tenía que rechazarse" >&2
  fail=1
}
printf '%s\n' "$out" | grep -qF "unknown type 'TipoRaro'" || {
  echo "FALLO: falta el error «unknown type»" >&2
  fail=1
}
printf '%s\n' "$out" | grep -qF 'type mismatch: expected Named("TipoRaro"), found Int' || {
  echo "FALLO: falta el error «type mismatch»" >&2
  fail=1
}
if printf '%s\n' "$out" | grep -q "posicion.titan:1:1:"; then
  echo "FALLO: hay un error en 1:1 (sigue el defecto de la posición)" >&2
  printf '%s\n' "$out" | head -4 >&2
  fail=1
fi
let_line=$(grep -n "let e: TipoRaro" selfhost/tests/errores/posicion.titan | head -1 | cut -d: -f1)
printf '%s\n' "$out" | grep -q "posicion.titan:${let_line}:" || {
  echo "FALLO: los errores no citan la línea ${let_line} de posicion.titan (la del let)" >&2
  printf '%s\n' "$out" | head -4 >&2
  fail=1
}

echo "== fallita 5: los errores de declaración citan su archivo =="
out=$("$TITAN" check selfhost/tests/errores/proyecto/principal.titan 2>&1) && {
  echo "FALLO: el proyecto de prueba tenía que rechazarse" >&2
  fail=1
}
printf '%s\n' "$out" | grep "unknown type 'TipoInexistente'" | grep -q "libreta.titan:" || {
  echo "FALLO: «unknown type 'TipoInexistente'» no cita libreta.titan:" >&2
  printf '%s\n' "$out" | head -6 >&2
  fail=1
}

echo "== fallita 6: cada error se imprime una sola vez =="
out=$("$TITAN" check selfhost/tests/errores/sin_duplicar.titan 2>&1) && {
  echo "FALLO: sin_duplicar.titan tenía que rechazarse" >&2
  fail=1
}
# Sin las líneas ::error de GitHub Actions (protocolo de anotación, con el
# mensaje aplanado): lo que ve el usuario son las demás líneas.
out=$(printf '%s\n' "$out" | grep -v "^::error")
n=$(printf '%s\n' "$out" | grep -c "unknown type 'TipoRaro'")
if [ "$n" -ne 1 ]; then
  echo "FALLO: «unknown type» aparece $n veces (debe ser 1)" >&2
  printf '%s\n' "$out" | head -6 >&2
  fail=1
fi
n=$(printf '%s\n' "$out" | grep -c "type mismatch: expected")
if [ "$n" -ne 1 ]; then
  echo "FALLO: «type mismatch» aparece $n veces (debe ser 1)" >&2
  printf '%s\n' "$out" | head -6 >&2
  fail=1
fi

echo "== destinos de asignación: self.campo, campo e índice tienen su propio mensaje =="
out=$("$TITAN" check selfhost/tests/errores/asignacion.titan 2>&1) && {
  echo "FALLO: asignacion.titan tenía que rechazarse" >&2
  fail=1
}
out=$(printf '%s\n' "$out" | grep -v "^::error")
# 1. `self.x = …` dentro de un método: el mensaje tiene que nombrar self y explicar por qué.
printf '%s\n' "$out" | grep -qF "cannot assign to self.x: methods receive 'self' by value" || {
  echo "FALLO: falta el diagnóstico propio de self.campo" >&2
  printf '%s\n' "$out" | head -6 >&2
  fail=1
}
# 2. `p.x = …` sobre un valor normal.
printf '%s\n' "$out" | grep -qF "cannot assign to the field 'x'" || {
  echo "FALLO: falta el diagnóstico de asignación a un campo" >&2
  printf '%s\n' "$out" | head -6 >&2
  fail=1
}
# 3. `xs[0] = …` sobre un elemento.
printf '%s\n' "$out" | grep -qF "cannot assign to an element" || {
  echo "FALLO: falta el diagnóstico de asignación a un elemento" >&2
  printf '%s\n' "$out" | head -6 >&2
  fail=1
}
# Y el mensaje genérico de antes ya no aparece para estos tres destinos.
if printf '%s\n' "$out" | grep -qF "assignment target must be a mutable local variable"; then
  echo "FALLO: sigue saliendo el mensaje genérico en vez del específico" >&2
  printf '%s\n' "$out" | head -6 >&2
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "probar.sh: todo bien"
fi
exit "$fail"
