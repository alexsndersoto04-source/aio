#!/bin/bash
# Verificación diferencial del compilador nativo (fase 4b del self-hosting).
#
# Para cada programa: lo compila a ejecutable con `selfhost/build.titan`
# (todo en Titan) y compara stdout, stderr y código de salida con `zett run`
# (la VM de Rust). En las pruebas de imagen también compara los archivos
# generados; sus casos productor/validador deben estar en el mismo shard.
# Las compilaciones no admitidas y los timeouts se cuentan aparte; nunca como aciertos.
#
# Uso: bash selfhost/native/verify_native.sh [archivos...]
#      (sin archivos: selfhost/tests/native/*.titan)
#      ZETT=/ruta/al/zett COMPILER=/ruta/a/titanc1 bash ...
#      TEST_TIMEOUT=60 BUILD_TIMEOUT=300 ajustan los límites en segundos.
# COMPILER permite reutilizar el compilador nativo ya creado en el bootstrap,
# evitando volver a interpretar build.titan con Rust para cada prueba.
set -u
ZETT="${ZETT:-zett}"
COMPILER="${COMPILER:-}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
if ! command -v "$ZETT" >/dev/null 2>&1; then
  echo "error: no se encuentra el Zett precompilado: $ZETT" >&2
  exit 2
fi
if [ -n "$COMPILER" ]; then
  compiler_path="$(command -v "$COMPILER")"
  if [ -z "$compiler_path" ] || [ ! -x "$compiler_path" ]; then
    echo "error: no se encuentra el compilador nativo ejecutable: $COMPILER" >&2
    exit 2
  fi
  compiler_dir="$(dirname "$(readlink -f "$compiler_path")")"
  if [ ! -f "$compiler_dir/native/runtime.titan" ]; then
    echo "error: falta native/runtime.titan junto al compilador: $compiler_dir/native/runtime.titan" >&2
    exit 2
  fi
  COMPILER="$compiler_path"
fi
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if [ $# -gt 0 ]; then FILES=("$@"); else FILES=(selfhost/tests/native/*.titan); fi
pass=0; fail=0; unsupported=0; build_timeouts=0; run_timeouts=0; current=0
BUILD_TIMEOUT="${BUILD_TIMEOUT:-300}"
TEST_TIMEOUT="${TEST_TIMEOUT:-60}"
total=${#FILES[@]}
details="$tmp/native-details.txt"
: > "$details"

# Workflow annotations remain queryable through the Checks API, unlike the
# runner's raw log archive in this environment.
report_error_annotation() {
  local file="$1"
  local title="$2"
  local message="$3"
  if [ -n "${GITHUB_ACTIONS:-}" ]; then
    message=${message//'%'/'%25'}
    message=${message//$'\r'/'%0D'}
    message=${message//$'\n'/'%0A'}
    printf '::error file=%s,title=%s::%s\n' "$file" "$title" "$message"
  fi
}

# Marker distinguishes a command that naturally exits 124 from one that the
# timeout program terminates. Never count two hung programs as a match.
run_limited() {
  local seconds="$1"
  local marker="$2"
  shift 2
  rm -f "$marker"
  timeout "$seconds" bash -c '
    marker=$1
    shift
    "$@"
    status=$?
    : > "$marker"
    exit "$status"
  ' _ "$marker" "$@"
  local status=$?
  [ -e "$marker" ] || return 124
  return "$status"
}

for f in "${FILES[@]}"; do
  current=$((current + 1))
  echo "[$current/$total] $f"
  rm -f "$tmp/prog" "$tmp/build.done"
  if [ -n "$COMPILER" ]; then
    run_limited "$BUILD_TIMEOUT" "$tmp/build.done" "$COMPILER" "$f" "$tmp/prog" > "$tmp/build.txt" 2>&1
    build_status=$?
  else
    run_limited "$BUILD_TIMEOUT" "$tmp/build.done" "$ZETT" run selfhost/build.titan "$f" "$tmp/prog" > "$tmp/build.txt" 2>&1
    build_status=$?
  fi
  if [ ! -e "$tmp/build.done" ]; then
    build_timeouts=$((build_timeouts + 1))
    message="tiempo agotado al compilar $f (límite ${BUILD_TIMEOUT}s)"
    echo "$message"
    printf '%s\n' "$message" >> "$details"
    report_error_annotation "$f" "Native compilation timeout" "$message"
    rm -f "$tmp/prog"
    continue
  fi
  if [ "$build_status" -ne 0 ] || [ ! -x "$tmp/prog" ]; then
    unsupported=$((unsupported + 1))
    message="no admitido: $f (compilador=$build_status): $(head -c 300 "$tmp/build.txt" | tr '\n' ' ')"
    echo "$message"
    printf '%s\n' "$message" >> "$details"
    report_error_annotation "$f" "Native test unsupported" "$message"
    continue
  fi
  base="$(basename "$f" .titan)"
  vm_args=(); nat_args=(); vm_files=""; nat_files=""; output_group=""
  case "$base" in
    image_gif|image_gif_validate) output_group="gif" ;;
    image_webp|image_webp_validate) output_group="webp" ;;
    image_webp_write) output_group="webp-write" ;;
    image_io) output_group="io" ;;
  esac
  if [ -n "$output_group" ]; then
    vm_files="$tmp/file-output/$output_group/vm"
    nat_files="$tmp/file-output/$output_group/native"
    mkdir -p "$vm_files" "$nat_files"
    vm_args=("$vm_files/output")
    nat_args=("$nat_files/output")
  fi
  run_limited "$TEST_TIMEOUT" "$tmp/vm.done" "$ZETT" run "$f" "${vm_args[@]}" > "$tmp/vm.out" 2> "$tmp/vm.err"
  vm=$?
  run_limited "$TEST_TIMEOUT" "$tmp/nat.done" "$tmp/prog" "${nat_args[@]}" > "$tmp/nat.out" 2> "$tmp/nat.err"
  nat=$?
  vm_timed_out=0; [ -e "$tmp/vm.done" ] || vm_timed_out=1
  nat_timed_out=0; [ -e "$tmp/nat.done" ] || nat_timed_out=1
  rm -f "$tmp/prog"
  if [ "$vm_timed_out" -ne 0 ] || [ "$nat_timed_out" -ne 0 ]; then
    run_timeouts=$((run_timeouts + 1))
    timed_out_sides=""
    [ "$vm_timed_out" -eq 0 ] || timed_out_sides="VM"
    if [ "$nat_timed_out" -ne 0 ]; then
      [ -z "$timed_out_sides" ] || timed_out_sides="$timed_out_sides, "
      timed_out_sides="${timed_out_sides}nativo"
    fi
    message="tiempo agotado al ejecutar $f (límite ${TEST_TIMEOUT}s; lado(s): $timed_out_sides)"
    echo "$message"
    printf '%s\n' "$message" >> "$details"
    report_error_annotation "$f" "Native vs VM execution timeout" "$message"
    continue
  fi
  side_effects_match=1
  if [ -n "$vm_files" ] && ! diff -qr -- "$vm_files" "$nat_files" > "$tmp/files.diff"; then
    side_effects_match=0
  fi
  # This smoke test makes positive PDF assertions (header, page tree, xref and
  # EOF). Differential equality alone would incorrectly accept the same
  # assertion failure from both runtimes, so this one must also exit 0.
  success_required=0
  if [ "$base" = "pdf_smoke" ] && { [ "$vm" -ne 0 ] || [ "$nat" -ne 0 ]; }; then
    success_required=1
  fi
  if cmp -s "$tmp/vm.out" "$tmp/nat.out" \
      && cmp -s "$tmp/vm.err" "$tmp/nat.err" \
      && [ "$vm" = "$nat" ] \
      && [ "$side_effects_match" -eq 1 ] \
      && [ "$success_required" -eq 0 ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    {
      echo "DIFERENCIA en $f (salida VM=$vm nativo=$nat):"
      diff "$tmp/vm.out" "$tmp/nat.out" | head -8 || true
      diff "$tmp/vm.err" "$tmp/nat.err" | head -4 || true
      if [ "$side_effects_match" -eq 0 ]; then
        echo "Archivos creados distintos:"
        head -8 "$tmp/files.diff"
      fi
      if [ "$success_required" -eq 1 ]; then
        echo "pdf_smoke debe terminar con código 0 (VM=$vm, nativo=$nat)."
      fi
    } > "$tmp/one-failure.txt"
    cat "$tmp/one-failure.txt"
    cat "$tmp/one-failure.txt" >> "$details"
    report_error_annotation "$f" "Native vs VM mismatch" "$(head -c 4000 "$tmp/one-failure.txt")"
  fi
done
echo "programas: $total  idénticos: $pass  distintos: $fail  no admitidos: $unsupported  timeout-compilación: $build_timeouts  timeout-ejecución: $run_timeouts"
if [ -n "${GITHUB_ACTIONS:-}" ]; then
  echo "::notice title=Native vs VM summary::programs=$total identical=$pass different=$fail unsupported=$unsupported build_timeouts=$build_timeouts run_timeouts=$run_timeouts"
fi

# GitHub Actions check-run output remains available even when its log archive
# cannot be downloaded. Keep failures and unsupported cases in the summary.
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "## Native vs VM"
    echo
    echo "| Asignados | Idénticos | Distintos | No admitidos | Timeout al compilar | Timeout al ejecutar |"
    echo "| ---: | ---: | ---: | ---: | ---: | ---: |"
    echo "| $total | $pass | $fail | $unsupported | $build_timeouts | $run_timeouts |"
    echo
    if [ -s "$details" ]; then
      echo "### Diferencias, timeouts y programas no admitidos"
      echo
      echo '```text'
      cat "$details"
      echo '```'
    else
      echo "Todos los programas asignados coincidieron en stdout, stderr y código de salida; también coinciden los archivos de salida de las pruebas de imagen."
    fi
  } >> "$GITHUB_STEP_SUMMARY"
fi
[ "$fail" -eq 0 ] && [ "$unsupported" -eq 0 ] && [ "$build_timeouts" -eq 0 ] && [ "$run_timeouts" -eq 0 ]
