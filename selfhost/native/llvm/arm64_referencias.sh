#!/bin/bash
# Crea las salidas de referencia con el Zett x86-64 precompilado y actual.
# El job ubuntu-24.04-arm ejecuta después los objetos AArch64 reales y compara
# contra estas salidas. No compila Titan con Cargo ni emula ARM64.
#
# Uso: bash selfhost/native/llvm/arm64_referencias.sh OBJETOS [archivos...]
set -u
ZETT="${ZETT:-zett}"
ZETT_BIN="$(command -v "$ZETT" 2>/dev/null || true)"
if [ -z "$ZETT_BIN" ]; then
  echo "no se encuentra el Zett precompilado: $ZETT" >&2
  exit 1
fi
ZETT_BIN="$(realpath "$ZETT_BIN")"
TIMEOUT="${TIMEOUT:-60}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
out="$(realpath -m "$1")"; shift
cd "$ROOT"
source "$SCRIPT_DIR/arm64_entorno.sh"
arm64_prepare_environment "$ROOT"
if [ $# -gt 0 ]; then FILES=("$@"); else FILES=(selfhost/tests/native/*.titan); fi
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

made=0; timed_out=0; native_only=0
for f in "${FILES[@]}"; do
  base="$(basename "$f" .titan)"
  if [ "$base" = image_webp_unsupported ]; then
    # Este caso contrasta deliberadamente el soporte del backend nativo con la
    # VM; su salida de referencia es el fixture del rechazo nativo.
    native_only=$((native_only + 1))
    continue
  fi
  case_dir="/tmp/aio-arm64-case/$base"
  rm -rf "$case_dir"
  mkdir -p "$case_dir"
  args=("$f")
  case "$base" in
    image_gif|image_gif_validate) args+=("$case_dir/output") ;;
    image_webp|image_webp_validate) args+=("$case_dir/output") ;;
    image_webp_write) args+=("$case_dir/output") ;;
    image_io) args+=("$case_dir/output") ;;
  esac
  prefix="$expected/$base"
  run_limited "$tmp/$base.done" /usr/bin/env -i "${ARM64_TEST_ENV[@]}" \
    "$ZETT_BIN" run "${args[@]}" > "$prefix.stdout" 2> "$prefix.stderr"
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
expected_count=$((${#FILES[@]} - native_only))
echo "referencias VM x86-64: $made  casos nativos especiales: $native_only  tiempos agotados: $timed_out"
[ "$timed_out" -eq 0 ] && [ "$made" -eq "$expected_count" ]
