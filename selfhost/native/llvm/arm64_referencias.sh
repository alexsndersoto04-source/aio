#!/bin/bash
# Crea las salidas de referencia con el compilador x86-64 de este mismo commit.
# El job ubuntu-24.04-arm ejecuta después los objetos AArch64 reales y compara
# contra estas salidas. No compila Titan con Cargo ni emula ARM64.
#
# Sin Rust: la referencia la produce el backend nativo del port (`titan run`;
# antes era `zett run`, la VM de Rust).
#
# Uso: bash selfhost/native/llvm/arm64_referencias.sh OBJETOS [archivos...]
set -u
TITAN="${TITAN:-}"
TIMEOUT="${TIMEOUT:-60}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
if [ -z "$TITAN" ]; then TITAN="$ROOT/selfhost/titan"; fi
if [ ! -x "$TITAN" ]; then
  echo "no se encuentra la CLI del port: $TITAN (TITAN=/ruta/a/titan)" >&2
  exit 1
fi
TITAN="$(realpath "$TITAN")"
out="$(realpath -m "$1")"; shift
cd "$ROOT"
source "$SCRIPT_DIR/arm64_entorno.sh"
arm64_prepare_environment "$ROOT"
if [ $# -gt 0 ]; then
  FILES=("$@")
else
  mapfile -t FILES < <(printf '%s\n' selfhost/tests/native/*.titan | LC_ALL=C sort)
fi
expected="$out/expected"
rm -rf "$expected"
mkdir -p "$expected"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

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

made=0; timed_out=0
for f in "${FILES[@]}"; do
  base="$(basename "$f" .titan)"
  case_dir="$(arm64_prepare_case_dir "$base")"
  args=("$f")
  case "$base" in
    image_gif|image_gif_validate) args+=("$case_dir/output") ;;
    image_webp|image_webp_validate) args+=("$case_dir/output") ;;
    image_webp_write) args+=("$case_dir/output") ;;
    image_io) args+=("$case_dir/output") ;;
  esac
  prefix="$expected/$base"
  run_limited "$tmp/$base.done" /usr/bin/env -i "${ARM64_TEST_ENV[@]}" \
    "$TITAN" run "${args[@]}" > "$prefix.stdout" 2> "$prefix.stderr"
  rc=$?
  if [ ! -e "$tmp/$base.done" ]; then
    timed_out=$((timed_out + 1))
    rm -f "$prefix.stdout" "$prefix.stderr"
    echo "TIEMPO AGOTADO al crear referencia VM para $f (límite ${TIMEOUT}s)"
    continue
  fi
  printf '%s\n' "$rc" > "$prefix.status"
  made=$((made + 1))
done
expected_count=${#FILES[@]}
echo "referencias x86-64 (backend nativo del port): $made  tiempos agotados: $timed_out"
[ "$timed_out" -eq 0 ] && [ "$made" -eq "$expected_count" ]
