#!/bin/bash
# Compara `titan debug` con `zett debug` (salida, errores y código de salida) con guiones de
# comandos sobre los programas de esta carpeta (y los que se pasen como argumentos 3 en adelante).
# `zett debug` repite "unknown debugger command" sin fin si la entrada se acaba antes de que el
# programa termine, así que todos los guiones acaban con muchas `c`.
# Uso: bash probar.sh [titan] [zett] [programa.titan...]
cd "$(dirname "$0")" || exit 1
TITAN=${1:-$PWD/../../titan}
ZETT=${2:-zett}
d=$(mktemp -d)
n=0; mal=0
rep() {  # rep N cmd [cmd...]: la secuencia, N veces, una por línea
  local k=$1; shift
  local i; for ((i = 0; i < k; i++)); do printf '%s\n' "$@"; done
}
comparar() {  # nombre, guion (con saltos de línea reales), args...
  local nombre=$1 guion=$2; shift 2
  n=$((n+1))
  { printf '%s\n' "$guion"; rep 80 c; } > "$d/in"
  timeout 600 "$TITAN" debug "$@" < "$d/in" > "$d/ot" 2> "$d/et"; ct=$?
  timeout 600 "$ZETT" debug "$@" < "$d/in" > "$d/oz" 2> "$d/ez"; cz=$?
  if cmp -s "$d/ot" "$d/oz" && cmp -s "$d/et" "$d/ez" && [ $ct = $cz ]; then
    echo "ok   $nombre ($(wc -l < "$d/ot") líneas)"
  else
    mal=$((mal+1)); echo "FALLA $nombre"
    diff "$d/ot" "$d/oz" | head -8; diff "$d/et" "$d/ez" | head -4; echo "códigos: $ct vs $cz"
  fi
}
for f in *.titan "${@:3}"; do
  [ -f "$f" ] || continue
  comparar "$f pasos" "$(rep 600 s)" "$f"
  comparar "$f next" "$(rep 200 n)" "$f"
  comparar "$f paso+p" "$(rep 300 s p)" "$f"
  comparar "$f mezcla" "$(printf '%s\n' s p n p s s o p s n o x '')" "$f"
  comparar "$f salir" "$(printf '%s\n' s s q)" "$f"
done
# Puntos de interrupción por línea (cada parada: p y continuar)
comparar "bucles bp" "$(rep 12 p c)" -b bucles.titan:3 -b bucles.titan:12 bucles.titan
comparar "metodos bp" "$(rep 3 p c)" -b metodos.titan:6 metodos.titan
comparar "valores bp" "$(printf '%s\n' p s p c p c)" --breakpoints valores.titan:17 valores.titan
comparar "valores bp -b pegado" "$(rep 5 p c)" -bvalores.titan:18 valores.titan
comparar "cierres bp" "$(rep 6 p c)" -b cierres.titan:2 -b cierres.titan:5 cierres.titan
# Errores de la línea de comandos
comparar "bp sin linea" "q" -b valores.titan valores.titan
comparar "bp linea cero" "q" -b valores.titan:0 valores.titan
comparar "bp linea negativa" "q" -b valores.titan:-3 valores.titan
comparar "bp archivo no existe" "q" -b noexiste.titan:3 valores.titan
comparar "entrada no existe" "q" noexiste.titan
echo "casos: $n, fallan: $mal"
rm -rf "$d"
[ $mal = 0 ]
