#!/bin/bash
# Diferencial entre las dos formas de ejecutar un programa en este compilador:
# el código nativo (backend propio, `titan run`) y la VM de bytecode del port
# (`titan build` + `titan exec`). Las dos están escritas en Titan aquí dentro.
#
#   - nativo: `titan run PROGRAMA.titan`  (compila a x86-64 y lo ejecuta).
#   - VM:     `titan build PROGRAMA.titan -o X.tbc` y `titan exec X.tbc`.
#
# Es la regresión del corpus que antes se hacía contra la VM de Rust (`zett`):
# al borrarse el Rust se quedó sin oráculo, y este arnés la recupera sin
# depender de nada externo. Antes de comparar, el programa se corre dos veces
# en nativo: si las dos salidas no coinciden es que el programa no es
# determinista (fechas, azar, pid) y se cuenta aparte, nunca como acierto.
#
#   bash selfhost/tests/vm_diff.sh [programas.titan...]
#
# Sin programas usa una muestra corta; con TITAN_VM_MUESTRA=1, la muestra
# amplia (uno de cada cuatro ficheros, sin los que necesitan red, pantalla,
# audio, servidores, procesos del sistema, terminal ni servicios externos).
# El trabajo se guarda en TITAN_VM_DIFF_DIR (por defecto /tmp/vm_diff).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
W="${TITAN_VM_DIFF_DIR:-/tmp/vm_diff}"
tiempo="${TITAN_VM_TIEMPO:-300}"

# Muestra corta: números y coma flotante, cadenas, colecciones, ficheros,
# errores y tareas.
corta() {
  echo selfhost/tests/native/basico.titan
  echo selfhost/tests/native/error_division.titan
  echo selfhost/tests/native/closures.titan
  echo selfhost/tests/native/colecciones.titan
  echo selfhost/tests/native/decimales.titan
  echo selfhost/tests/native/texto_std.titan
  echo selfhost/tests/native/vec_std.titan
  echo selfhost/tests/native/hash_std.titan
  echo selfhost/tests/native/json_std.titan
  echo selfhost/tests/native/regex_basico.titan
  echo selfhost/tests/native/archivos_bytes.titan
  echo selfhost/tests/native/dirs_std.titan
  echo selfhost/tests/native/tareas_basico.titan
  echo selfhost/tests/native/error_canal_limite.titan
}

# Muestra amplia: uno de cada cuatro ficheros, sin los que necesitan red,
# pantalla, audio, servidores, procesos del sistema, terminal o servicios
# externos (la VM tampoco los tiene: los dos lados fallarían, pero por cosas
# distintas y sin interés), y sin el compilador mismo.
amplia() {
  ls selfhost/tests/native/*.titan \
    | grep -v "redis\|postgres\|mysql\|sqlite\|onnx\|audio\|gui_std\|window\|clipboard\|mobile\|termux" \
    | grep -v "http\|server\|servidor\|router\|tcp_\|ws_\|tls_\|wifi\|dns\|net_\|url_\|web_\|email_\|jwt\|pdf" \
    | grep -v "process\|proceso\|spawn\|senal\|signals\|fswatch\|term_\|input_\|teclas\|kv_\|game_\|plot" \
    | grep -v "optimizador\|runtime_info\|freestanding\|diag_\|clipboard" \
    | awk 'NR%4==0'
}

if [ "$#" -gt 0 ]; then
  files="$*"
elif [ "${TITAN_VM_MUESTRA:-0}" = "1" ]; then
  files="$(amplia)"
else
  files="$(corta)"
fi

mkdir -p "$W"
iguales=0
distintos=0
nodet=0
noadm=0
lentos=0
errores=0
for f in $files; do
  if [ ! -f "$f" ]; then echo "no existe: $f" >&2; exit 2; fi
  base="$(basename "$f" .titan)"
  # 1. Nativo, dos veces: la segunda dice si el programa es determinista.
  timeout "$tiempo" ./selfhost/titan run "$f" < /dev/null > "$W/$base.run1.out" 2> "$W/$base.run1.err"
  a1=$?
  timeout "$tiempo" ./selfhost/titan run "$f" < /dev/null > "$W/$base.run2.out" 2> "$W/$base.run2.err"
  a2=$?
  if [ "$a1" = "124" ] || [ "$a2" = "124" ]; then
    echo "LENTO    $base  (el nativo no acabó en ${tiempo}s)"
    lentos=$((lentos + 1))
    continue
  fi
  determinista="si"
  if [ "$a1" != "$a2" ] || ! cmp -s "$W/$base.run1.out" "$W/$base.run2.out" || ! cmp -s "$W/$base.run1.err" "$W/$base.run2.err"; then
    determinista="no"
  fi
  # 2. La VM del port: su propio bytecode.
  rm -f "$W/$base.tbc"
  timeout "$tiempo" ./selfhost/titan build "$f" -o "$W/$base.tbc" < /dev/null > "$W/$base.build.out" 2> "$W/$base.build.err"
  ab=$?
  if [ "$ab" = "124" ]; then
    echo "LENTO    $base  (la VM no acabó de compilar en ${tiempo}s)"
    lentos=$((lentos + 1))
    continue
  fi
  if [ ! -f "$W/$base.tbc" ]; then
    echo "NO ADMITIDO $base: $(head -1 "$W/$base.build.err" | cut -c1-90)"
    noadm=$((noadm + 1))
    continue
  fi
  timeout "$tiempo" ./selfhost/titan exec "$W/$base.tbc" < /dev/null > "$W/$base.vm.out" 2> "$W/$base.vm.err"
  av=$?
  if [ "$av" = "124" ]; then
    echo "LENTO    $base  (la VM no acabó en ${tiempo}s)"
    lentos=$((lentos + 1))
    continue
  fi
  # 3. Comparación: código de salida, stdout y stderr.
  if [ "$determinista" = "no" ]; then
    # El programa cambia entre dos corridas del MISMO lado (fechas, azar, pid):
    # comparar byte a byte no diría nada. Se cuenta aparte.
    echo "NO DET.  $base  (dos corridas nativas distintas; código $a1)"
    nodet=$((nodet + 1))
    continue
  fi
  if [ "$a1" = "$av" ] && cmp -s "$W/$base.run1.out" "$W/$base.vm.out" && cmp -s "$W/$base.run1.err" "$W/$base.vm.err"; then
    echo "IGUAL    $base  (codigo $a1, $(wc -l < "$W/$base.run1.out") lineas)"
    iguales=$((iguales + 1))
  else
    echo "DISTINTO $base  codigo nativo=$a1 vm=$av"
    for x in out err; do
      cmp -s "$W/$base.run1.$x" "$W/$base.vm.$x" || {
        echo "   --- $x (nativo | VM)"
        diff <(head -c 500 "$W/$base.run1.$x") <(head -c 500 "$W/$base.vm.$x") | head -6
      }
    done
    distintos=$((distintos + 1))
  fi
done

echo "programas: $((iguales + distintos + nodet + noadm + lentos))  identicos: $iguales  distintos: $distintos  no deterministas: $nodet  no admitidos: $noadm  lentos: $lentos"
[ "$distintos" -eq 0 ] && [ "$noadm" -eq 0 ] && [ "$lentos" -eq 0 ]
