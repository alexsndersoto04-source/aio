#!/bin/bash
# Diferencial x86-64 contra ARM 32 bits: el mismo programa se compila para las
# dos arquitecturas y se compara lo que escupe cada uno (stdout, stderr y código
# de salida).
#
#   - x86-64: el backend propio del compilador, con `titan run` (la referencia).
#   - ARM32: `titan compile --target armv7` (backend LLVM + clang/lld) y el
#     ejecutable se corre con el procesador ARMv7 de
#     selfhost/native/arm32/correr.py, que sirve las llamadas al sistema desde
#     Python. No es un Linux: es el mismo ejecutable que correría en una
#     Raspberry o un Android viejo de 32 bits, con el núcleo puesto desde aquí.
#
#   bash selfhost/tests/arm32_diff.sh [programas.titan...]
#
# Sin programas usa una muestra fija (y con TITAN_ARM32_MUESTRA=1, la muestra
# amplia: uno de cada cuatro ficheros, sin los que necesitan red, pantalla,
# audio, servidores, procesos o terminal). El trabajo se guarda en
# TITAN_ARM32_DIFF_DIR (por defecto /tmp/arm32_diff); las referencias x86-64 se
# reaprovechan y solo se repiten con TITAN_ARM32_REFRESCAR=1.
#
# Un programa que el backend LLVM rechaza («not supported by the native backend
# yet», p. ej. std::runtime::*) se cuenta aparte como NO ADMITIDO, y uno que no
# cabe en el tiempo que se le da (TITAN_ARM32_TIEMPO, por defecto 900 s, y 1800
# para los que llevan el fichero `.lento` al lado, como archive_zip64) como
# LENTO: nunca como aciertos. Si lo mata una señal (137: casi siempre quedarse
# sin memoria en la máquina que emula) se cuenta como fallo, no como lento. El
# ejecutor va a unos 30 millones de instrucciones por segundo, así que un
# programa pesado (comprimir 6 MB, por ejemplo) tarda minutos: para comprobar
# resultados da igual, pero no es una máquina para medir rendimiento.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
W="${TITAN_ARM32_DIFF_DIR:-/tmp/arm32_diff}"
refrescar="${TITAN_ARM32_REFRESCAR:-0}"
tiempo="${TITAN_ARM32_TIEMPO:-900}"

# Muestra corta: números (coma flotante incluida), cadenas, colecciones,
# ficheros, directorios, entorno, fechas y todo lo que el ejecutor sirve.
corta() {
  echo selfhost/tests/native/basico.titan
  echo selfhost/tests/native/error_division.titan
  echo selfhost/tests/native/closures.titan
  echo selfhost/tests/native/colecciones.titan
  echo selfhost/tests/native/coll_std.titan
  echo selfhost/tests/native/decimales.titan
  echo selfhost/tests/native/math_libm.titan
  echo selfhost/tests/native/texto_std.titan
  echo selfhost/tests/native/vec_std.titan
  echo selfhost/tests/native/hash_std.titan
  echo selfhost/tests/native/memoria.titan
  echo selfhost/tests/native/random_std.titan
  echo selfhost/tests/native/json_std.titan
  echo selfhost/tests/native/regex_basico.titan
  echo selfhost/tests/native/archivos_bytes.titan
  echo selfhost/tests/native/texto_sistema.titan
  echo selfhost/tests/native/dirs_std.titan
  echo selfhost/tests/native/dt_std.titan
  echo selfhost/tests/native/procfs_std.titan
  echo selfhost/tests/native/misc_std.titan
}

# Muestra amplia: uno de cada cuatro ficheros, sin los que necesitan red,
# pantalla, audio, servidores, procesos del sistema ni terminal (el ejecutor no
# tiene núcleo: ni redes, ni señales, ni fork/exec) — y sin el compilador mismo,
# que es mucho más pesado que el resto.
amplia() {
  ls selfhost/tests/native/*.titan \
    | grep -v "redis\|postgres\|mysql\|sqlite\|onnx\|audio\|gui_std\|window\|clipboard\|mobile\|termux" \
    | grep -v "http\|server\|servidor\|router\|tcp_\|ws_\|tls_\|wifi\|dns\|net_\|url_\|web_\|email_\|jwt\|pdf" \
    | grep -v "process\|proceso\|spawn\|senal\|signals\|fswatch\|term_\|input_\|teclas\|kv_\|game_\|plot" \
    | grep -v "optimizador\|runtime_info\|freestanding\|diag_\|clipboard" \
    | grep -v "tareas_estres" \
    | awk 'NR%4==0 || /tarea|canal|coleccion/'
}

# tareas_estres queda fuera de la muestra a propósito: levanta 256 tareas a la
# vez y cada tarea reserva 32 MB de pila (TK_STACK), o sea 8 GB. Un proceso de
# 32 bits solo ve 3 GB, así que el programa no cabe en ningún ARM de 32 bits
# —ni emulado ni de verdad—; no es un fallo del port. Medido aquí: el ejecutor
# coloca 2 reservas iniciales de 64 MB + 84 pilas de 32 MB = los 2816 MB de su
# espacio de usuario y la pila 85 recibe ENOMEM, y el runtime entonces dice
# «out of memory». El resto de las pruebas de tareas (basico, canales, select,
# error_tarea_*) sí entran en la muestra.

if ! python3 -c "import unicorn, capstone" > /dev/null 2>&1; then
  echo "faltan los módulos de Python unicorn y capstone (los necesita el ejecutor ARM32)" >&2
  exit 2
fi

# El compilador se construye si no está (no se versiona).
if [ ! -x selfhost/titan ]; then
  echo "compilando selfhost/titan..."
  env -i PATH=/nonexistent ./selfhost/titanc3 selfhost/titan.titan selfhost/titan || exit 1
fi

if [ "$#" -gt 0 ]; then
  files="$*"
elif [ "${TITAN_ARM32_MUESTRA:-0}" = "1" ]; then
  files="$(amplia)"
else
  files="$(corta)"
fi

mkdir -p "$W/bin" "$W/ref" "$W/cc"
iguales=0
distintos=0
noadm=0
lentos=0
errores=0
for f in $files; do
  if [ ! -f "$f" ]; then echo "no existe: $f" >&2; exit 2; fi
  base="$(basename "$f" .titan)"
  # 1. Referencia x86-64 (se reaprovecha entre pasadas).
  if [ "$refrescar" = "1" ] || [ ! -f "$W/ref/$base.code" ]; then
    timeout 900 ./selfhost/titan run "$f" < /dev/null > "$W/ref/$base.out" 2> "$W/ref/$base.err"
    echo $? > "$W/ref/$base.code"
  fi
  # 2. El mismo programa para ARM 32 bits, por el camino de verdad:
  #    `titan compile --target armv7` (que llama a clang/lld o al clang de zig).
  rm -f "$W/bin/$base.arm"
  timeout 900 ./selfhost/titan compile "$f" -o "$W/bin/$base.arm" --target armv7 \
    > "$W/cc/$base.out" 2> "$W/cc/$base.err"
  if [ ! -x "$W/bin/$base.arm" ]; then
    if grep -q "not supported by the native backend yet" "$W/cc/$base.err" "$W/cc/$base.out" 2> /dev/null; then
      echo "NO ADMITIDO $base: $(grep -m1 -ho 'instruction [A-Za-z0-9_]* is not supported' "$W/cc/$base.err" "$W/cc/$base.out" | head -1)"
      noadm=$((noadm + 1))
    else
      echo "COMPILA FALLA $base: $(head -2 "$W/cc/$base.err" | tr '\n' ' ')$(head -1 "$W/cc/$base.out")"
      errores=$((errores + 1))
    fi
    continue
  fi
  # 3. El mismo ejecutable en el procesador ARMv7, con las llamadas al sistema
  #    puestas desde Python.
  limite="$tiempo"
  if [ -f "${f%.titan}.lento" ]; then limite=1800; fi
  timeout "$limite" python3 selfhost/native/arm32/correr.py "$W/bin/$base.arm" < /dev/null > "$W/bin/$base.out" 2> "$W/bin/$base.err"
  a=$?
  b="$(cat "$W/ref/$base.code")"
  if [ "$a" = "124" ]; then
    echo "LENTO    $base  (no acabó en ${limite}s; salida nativa $b)"
    lentos=$((lentos + 1))
  elif [ "$a" = "137" ]; then
    echo "MUERTO   $base  (lo mató una señal —¿memoria?—; salida nativa $b)"
    errores=$((errores + 1))
  elif [ "$a" = "$b" ] && cmp -s "$W/ref/$base.out" "$W/bin/$base.out" && cmp -s "$W/ref/$base.err" "$W/bin/$base.err"; then
    echo "IGUAL    $base  (codigo $a, $(wc -l < "$W/bin/$base.out") lineas)"
    iguales=$((iguales + 1))
  else
    echo "DISTINTO $base  codigo emul=$a nativo=$b"
    for x in out err; do
      cmp -s "$W/ref/$base.$x" "$W/bin/$base.$x" || {
        echo "   --- $x (x86-64 | ARM32)"
        diff <(head -c 500 "$W/ref/$base.$x") <(head -c 500 "$W/bin/$base.$x") | head -6
      }
    done
    distintos=$((distintos + 1))
  fi
done

echo "programas: $((iguales + distintos + noadm + lentos + errores))  identicos: $iguales  distintos: $distintos  no admitidos: $noadm  lentos: $lentos  sin compilar: $errores"
[ "$distintos" -eq 0 ] && [ "$errores" -eq 0 ]
