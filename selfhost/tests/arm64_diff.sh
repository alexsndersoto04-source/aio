#!/bin/bash
# Diferencial x86-64 (backend propio) contra ARM64 (backend LLVM): el mismo programa se compila y ejecuta con
# `titan run` en cada arquitectura y se comparan stdout, stderr y código de salida.
#
#   bash selfhost/tests/arm64_diff.sh generar <titan> <directorio> [programas...]   (en cada máquina)
#   bash selfhost/tests/arm64_diff.sh comparar <dir-x86> <dir-arm64>
#
# Sin programas, usa una muestra determinista de selfhost/tests/native (1 de cada 4, sin los que necesitan
# servidores, pantalla, audio real o red) más todas las pruebas de tareas y canales. Un programa que el backend LLVM rechaza («not supported by the native
# backend yet», p. ej. spawn/canales) se cuenta aparte como NO ADMITIDO: nunca como acierto.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
modo="${1:-}"

muestra() {
  ls selfhost/tests/native/*.titan | grep -v "redis\|postgres\|mysql\|audio_player\|gui_std\|sqlite_conc\|window_x11\|clipboard\|http_serve\|servidor_\|tcp_\|ws_\|tls_\|wifi\|termux\|fswatch" | awk 'NR%4==0 || /tarea|canal|coleccion|window_state|server_std/'
}

case "$modo" in
generar)
  titan="$(readlink -f "$2")"
  out="$3"
  shift 3
  mkdir -p "$out"
  files="$*"
  [ -z "$files" ] && files="$(muestra)"
  n=0
  for f in $files; do
    n=$((n + 1))
    base="$(basename "$f" .titan)"
    timeout 600 "$titan" run "$f" > "$out/$base.out" 2> "$out/$base.err"
    echo $? > "$out/$base.code"
    echo "[$n] $base -> $(cat "$out/$base.code")"
  done
  ;;
comparar)
  a="$2"
  b="$3"
  iguales=0
  distintos=0
  noadm=0
  for c in "$a"/*.code; do
    base="$(basename "$c" .code)"
    [ -f "$b/$base.code" ] || { echo "falta en ARM64: $base"; distintos=$((distintos + 1)); continue; }
    if grep -q "not supported by the native backend yet" "$b/$base.err" 2> /dev/null; then
      echo "NO ADMITIDO en ARM64: $base ($(grep -o 'instruction [A-Za-z]* is not supported' "$b/$base.err" | head -1))"
      noadm=$((noadm + 1))
      continue
    fi
    if cmp -s "$a/$base.out" "$b/$base.out" && cmp -s "$a/$base.err" "$b/$base.err" && cmp -s "$a/$base.code" "$b/$base.code"; then
      iguales=$((iguales + 1))
    else
      echo "DISTINTO: $base"
      for x in out err code; do
        cmp -s "$a/$base.$x" "$b/$base.$x" || { echo "  --- $x (x86-64 | ARM64)"; diff <(head -c 600 "$a/$base.$x") <(head -c 600 "$b/$base.$x") | head -8; }
      done
      distintos=$((distintos + 1))
    fi
  done
  echo "programas: $((iguales + distintos + noadm))  idénticos: $iguales  distintos: $distintos  no admitidos en ARM64: $noadm"
  [ "$distintos" -eq 0 ]
  ;;
*)
  echo "uso: $0 generar <titan> <dir> [programas...] | comparar <dir-x86> <dir-arm64>" >&2
  exit 2
  ;;
esac
