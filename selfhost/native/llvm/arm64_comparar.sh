#!/bin/bash
# Backend LLVM para ARM64, parte 2: enlaza cada objeto de arm64_objetos.sh y
# lo ejecuta en una máquina ARM64 real. Compara la salida con referencias
# generadas por el Zett x86-64 precompilado del mismo commit; no usa el binario
# AArch64 publicado, que puede corresponder a una versión anterior.
#
# Uso: bash selfhost/native/llvm/arm64_comparar.sh OBJETOS [archivos...]
set -u
TIMEOUT="${TIMEOUT:-60}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
objs="$(realpath "$1")"; shift
cd "$ROOT"
source "$SCRIPT_DIR/arm64_entorno.sh"
arm64_prepare_environment "$ROOT"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if [ $# -gt 0 ]; then FILES=("$@"); else FILES=(selfhost/tests/native/*.titan); fi
echo "máquina: $(uname -m)"
pass=0; fail=0; unsupported=0; timed_out=0

run_limited() {
  local marker="$1"
  shift
  rm -f "$marker"
  timeout "$TIMEOUT" bash -c '
    marker=$1
    shift
    "$@"
    rc=$?
    : > "$marker"
    exit "$rc"
  ' _ "$marker" "$@"
  local rc=$?
  [ -e "$marker" ] || return 124
  return "$rc"
}

normalize_stdout() {
  local base="$1" file="$2"
  if [ "$base" = freestanding_textos ]; then
    # El nombre GNU ld del formato AArch64 varía entre hosts; normaliza solo
    # ese alias, manteniendo intacto el resto del script generado.
    sed -E 's/OUTPUT_FORMAT\("elf64-(little)?aarch64"\)/OUTPUT_FORMAT("elf64-<aarch64>")/g' "$file"
  else
    cat "$file"
  fi
}

for f in "${FILES[@]}"; do
  base="$(basename "$f" .titan)"
  if [ -f "$objs/$base.err" ]; then
    unsupported=$((unsupported + 1)); echo "no admitido: $f: $(head -c 300 "$objs/$base.err" | tr '\n' ' ')"; continue
  fi
  if [ ! -f "$objs/$base.o" ]; then
    fail=$((fail + 1)); echo "DIFERENCIA en $f: no hay objeto ARM64"; continue
  fi
  rm -f "$tmp/prog"
  if ! ld -static -e _start -o "$tmp/prog" "$objs/$base.o" > "$tmp/ld.txt" 2>&1; then
    fail=$((fail + 1)); echo "DIFERENCIA en $f: no enlaza: $(head -c 300 "$tmp/ld.txt" | tr '\n' ' ')"; continue
  fi
  case_dir="/tmp/aio-arm64-case/$base"
  rm -rf "$case_dir"
  mkdir -p "$case_dir"
  nat_args=("$tmp/prog")
  case "$base" in
    image_gif|image_gif_validate) nat_args+=("$case_dir/output") ;;
    image_webp|image_webp_validate) nat_args+=("$case_dir/output") ;;
    image_webp_write) nat_args+=("$case_dir/output") ;;
    image_io) nat_args+=("$case_dir/output") ;;
  esac
  run_limited "$tmp/$base.done" /usr/bin/env -i "${ARM64_TEST_ENV[@]}" \
    "${nat_args[@]}" > "$tmp/nat.stdout" 2> "$tmp/nat.stderr"
  nat=$?
  if [ ! -e "$tmp/$base.done" ]; then
    timed_out=$((timed_out + 1))
    echo "TIEMPO AGOTADO en $f (ARM64; límite ${TIMEOUT}s)"
    continue
  fi
  prefix="$objs/expected/$base"
  if [ ! -f "$prefix.stdout" ] || [ ! -f "$prefix.stderr" ] || [ ! -f "$prefix.status" ]; then
    fail=$((fail + 1)); echo "DIFERENCIA en $f: falta la referencia VM x86-64"; continue
  fi
  vm="$(cat "$prefix.status")"
  normalize_stdout "$base" "$prefix.stdout" > "$tmp/vm.norm"
  normalize_stdout "$base" "$tmp/nat.stdout" > "$tmp/nat.norm"
  if cmp -s "$tmp/vm.norm" "$tmp/nat.norm" && cmp -s "$prefix.stderr" "$tmp/nat.stderr" && [ "$vm" = "$nat" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "DIFERENCIA en $f (salida VM x86-64=$vm ARM64=$nat):"
    diff -u "$tmp/vm.norm" "$tmp/nat.norm" | head -8
    diff -u "$prefix.stderr" "$tmp/nat.stderr" | head -4
  fi
done
echo "programas: ${#FILES[@]}  idénticos: $pass  distintos: $fail  tiempos agotados: $timed_out  no admitidos: $unsupported  referencia: VM x86-64 precompilada"
[ "$fail" -eq 0 ] && [ "$timed_out" -eq 0 ] && [ "$unsupported" -eq 0 ]
