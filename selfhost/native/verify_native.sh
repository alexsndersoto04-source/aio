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
redis_port=""
redis_pid_file=""
cleanup() {
  if [ -n "$redis_port" ] && [ -n "$redis_pid_file" ] && [ -f "$redis_pid_file" ] && command -v redis-cli >/dev/null 2>&1; then
    redis_pid="$(cat "$redis_pid_file" 2>/dev/null || true)"
    serving_pid="$(redis-cli --no-auth-warning -a 'titan:pass@word' -h 127.0.0.1 -p "$redis_port" info server 2>/dev/null | awk -F: '$1 == "process_id" {gsub("\\r", "", $2); print $2; exit}')"
    if [[ "$redis_pid" =~ ^[0-9]+$ ]] && [ "$redis_pid" = "$serving_pid" ]; then
      redis-cli --no-auth-warning -a 'titan:pass@word' -h 127.0.0.1 -p "$redis_port" shutdown nosave >/dev/null 2>&1 || true
      for _ in $(seq 1 20); do
        if ! kill -0 "$redis_pid" 2>/dev/null; then break; fi
        sleep 0.05
      done
      if kill -0 "$redis_pid" 2>/dev/null; then kill -TERM "$redis_pid" 2>/dev/null || true; fi
    fi
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT
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
  vm_args=(); nat_args=(); vm_env=(); nat_env=(); vm_files=""; nat_files=""; output_group=""
  interop_path=""
  if [ "$base" = "redis" ] && [ -z "${TITAN_TEST_REDIS_URL:-}" ]; then
    if ! command -v redis-server >/dev/null 2>&1 || ! command -v redis-cli >/dev/null 2>&1; then
      echo "error: redis.titan requires a real redis-server and redis-cli, or TITAN_TEST_REDIS_URL" >&2
      exit 2
    fi
    redis_dir="$tmp/redis"
    mkdir -p "$redis_dir"
    redis_pid_file="$redis_dir/redis.pid"
    redis_port="$(python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)"
    redis-server --bind 127.0.0.1 --port "$redis_port" --protected-mode yes \
      --save "" --appendonly no --daemonize yes --dir "$redis_dir" \
      --pidfile "$redis_pid_file" --logfile "$redis_dir/redis.log" \
      --requirepass 'titan:pass@word' --loglevel warning
    redis_ready=0
    for _ in $(seq 1 50); do
      if redis-cli --no-auth-warning -a 'titan:pass@word' -h 127.0.0.1 -p "$redis_port" ping >/dev/null 2>&1; then
        redis_ready=1
        break
      fi
      sleep 0.1
    done
    if [ "$redis_ready" -ne 1 ]; then
      cat "$redis_dir/redis.log" >&2 || true
      echo "error: the real Redis server did not become ready" >&2
      exit 2
    fi
    redis_url="redis://default:titan%3Apass%40word@127.0.0.1:$redis_port/13"
    redis_bad_url="redis://default:wrong-password@127.0.0.1:$redis_port/13"
    vm_env=(env "TITAN_TEST_REDIS_URL=$redis_url" "TITAN_TEST_REDIS_BAD_URL=$redis_bad_url")
    nat_env=(env "TITAN_TEST_REDIS_URL=$redis_url" "TITAN_TEST_REDIS_BAD_URL=$redis_bad_url")
  fi
  interop_seed_snapshot=1
  case "$base" in
    audio_player_backend)
      audio_backend_dir="$tmp/audio-player-nonexec"
      mkdir -p "$audio_backend_dir"
      printf 'this file is never launched; it tests PATH permission checks\n' > "$audio_backend_dir/mpv"
      chmod 0644 "$audio_backend_dir/mpv"
      vm_env=(env "PATH=$audio_backend_dir" "TITAN_AUDIO_BACKEND_NONEXEC_ONLY=1")
      nat_env=(env "PATH=$audio_backend_dir" "TITAN_AUDIO_BACKEND_NONEXEC_ONLY=1")
      ;;
    audio_tags)
      audio_alias_dir="$tmp/audio-tags-lossy"
      python3 selfhost/tests/native/prepare_audio_tag_aliases.py "$audio_alias_dir"
      audio_cover_dir="$tmp/audio-tags-cover"
      audio_scan_limit_dir="$tmp/audio-tags-scan-limit"
      vm_env=(env "TITAN_AUDIO_TAGS_LOSSY_DIR=$audio_alias_dir" "TITAN_AUDIO_TAGS_COVER_DIR=$audio_cover_dir" "TITAN_AUDIO_TAGS_SCAN_LIMIT_DIR=$audio_scan_limit_dir")
      nat_env=(env "TITAN_AUDIO_TAGS_LOSSY_DIR=$audio_alias_dir" "TITAN_AUDIO_TAGS_COVER_DIR=$audio_cover_dir" "TITAN_AUDIO_TAGS_SCAN_LIMIT_DIR=$audio_scan_limit_dir")
      ;;
    kv_interop)
      interop_path="$tmp/kv-interoperability"
      vm_env=(env "TITAN_KV_INTEROP_MODE=seed" "TITAN_KV_INTEROP_PATH=$interop_path")
      nat_env=(env "TITAN_KV_INTEROP_MODE=native" "TITAN_KV_INTEROP_PATH=$interop_path")
      ;;
    kv_symlink_canonical)
      kv_target_one="$tmp/kv-target-one"
      kv_target_two="$tmp/kv-target-two"
      kv_symlink="$tmp/kv-symlink"
      kv_symlink_next="$tmp/kv-symlink-next"
      mkdir -p "$kv_target_one" "$kv_target_two"
      ln -s "$kv_target_one" "$kv_symlink"
      ln -s "$kv_target_two" "$kv_symlink_next"
      vm_env=(env "TITAN_KV_TARGET_ONE=$kv_target_one" "TITAN_KV_TARGET_TWO=$kv_target_two" "TITAN_KV_SYMLINK=$kv_symlink" "TITAN_KV_SYMLINK_NEXT=$kv_symlink_next")
      nat_env=(env "TITAN_KV_TARGET_ONE=$kv_target_one" "TITAN_KV_TARGET_TWO=$kv_target_two" "TITAN_KV_SYMLINK=$kv_symlink" "TITAN_KV_SYMLINK_NEXT=$kv_symlink_next")
      ;;
  esac
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
  run_limited "$TEST_TIMEOUT" "$tmp/vm.done" "${vm_env[@]}" "$ZETT" run "$f" "${vm_args[@]}" > "$tmp/vm.out" 2> "$tmp/vm.err"
  vm=$?
  if [ "$base" = "kv_symlink_canonical" ]; then
    rm -f "$kv_symlink" "$kv_symlink_next"
    rm -rf -- "$kv_target_one" "$kv_target_two"
    mkdir -p "$kv_target_one" "$kv_target_two"
    ln -s "$kv_target_one" "$kv_symlink"
    ln -s "$kv_target_two" "$kv_symlink_next"
  fi
  if [ "$base" = "kv_interop" ] && [ ! -f "$interop_path/.titan-kv-v1.json" ]; then
    interop_seed_snapshot=0
  fi
  run_limited "$TEST_TIMEOUT" "$tmp/nat.done" "${nat_env[@]}" "$tmp/prog" "${nat_args[@]}" > "$tmp/nat.out" 2> "$tmp/nat.err"
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
  interop_verify_status=0
  if [ "$base" = "kv_interop" ]; then
    run_limited "$TEST_TIMEOUT" "$tmp/interop-vm.done" env \
      "TITAN_KV_INTEROP_MODE=verify" "TITAN_KV_INTEROP_PATH=$interop_path" \
      "$ZETT" run "$f" > "$tmp/interop-vm.out" 2> "$tmp/interop-vm.err"
    interop_verify_status=$?
    if [ ! -e "$tmp/interop-vm.done" ]; then
      run_timeouts=$((run_timeouts + 1))
      message="tiempo agotado al verificar la interoperabilidad KV con Sled ($TEST_TIMEOUT s)"
      echo "$message"
      printf '%s\n' "$message" >> "$details"
      report_error_annotation "$f" "KV/Sled interop timeout" "$message"
      continue
    fi
  fi
  side_effects_match=1
  if [ -n "$vm_files" ]; then
    case "$output_group" in
      gif|webp|webp-write)
        # GIF/WebP encoders may produce different valid bitstreams for the same
        # pixels. Compare the emitted file set here; the paired Titan tests
        # decode every file and compare SHA-256 digests of lossless PNG/BMP
        # encodings, so output semantics remain checked byte-for-byte.
        find "$vm_files" -type f -printf '%P\n' | LC_ALL=C sort > "$tmp/vm-files.txt"
        find "$nat_files" -type f -printf '%P\n' | LC_ALL=C sort > "$tmp/nat-files.txt"
        if ! cmp -s "$tmp/vm-files.txt" "$tmp/nat-files.txt"; then
          side_effects_match=0
          {
            echo "Archivos creados distintos (nombres):"
            diff -u "$tmp/vm-files.txt" "$tmp/nat-files.txt" | head -12 || true
          } > "$tmp/files.diff"
        else
          empty_file="$(find "$vm_files" "$nat_files" -type f -empty -print -quit)"
          if [ -n "$empty_file" ]; then
            side_effects_match=0
            printf 'Archivo de imagen vacío: %s\n' "$empty_file" > "$tmp/files.diff"
          fi
        fi
        ;;
      *)
        if ! diff -qr -- "$vm_files" "$nat_files" > "$tmp/files.diff"; then
          side_effects_match=0
        fi
        ;;
    esac
  fi
  # These are positive behavioral smokes, not just differential comparisons:
  # input, audio, image/PDF, X11 when DISPLAY is configured, and Termux-path cases
  # must exit successfully on both sides. Otherwise matching failures could look
  # like parity.
  success_required=0
  if [ "$base" = "pdf_smoke" ] || [ "$base" = "termux_available" ] || [ "$base" = "input_std" ] || [ "$base" = "audio_synthesis" ] || [ "$base" = "audio_termux_missing" ] || [ "$base" = "audio_wav_io" ] || [ "$base" = "audio_wav_read" ] || [ "$base" = "audio_player_backend" ] || [ "$base" = "audio_tags" ] || [ "$base" = "kv_interop" ] || [ "$base" = "kv_symlink_canonical" ] || [ "$base" = "window_x11_open" ] || [ "$base" = "redis" ] || [[ "$base" == image_* ]]; then
    if [ "$vm" -ne 0 ] || [ "$nat" -ne 0 ] || [ "$interop_verify_status" -ne 0 ]; then
      success_required=1
    fi
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
        echo "La prueba positiva $base debe terminar con código 0 (VM=$vm, nativo=$nat)."
      fi
      if [ "$base" = "kv_interop" ] && [ "$interop_seed_snapshot" -eq 0 ]; then
        echo "El Zett de VM no publicó .titan-kv-v1.json; la interoperabilidad Rust/Sled no queda validada con este artefacto."
        echo "Origen del Zett: ${ZETT_SOURCE_COMMIT:-desconocido}"
      fi
      if [ "$base" = "kv_interop" ] && [ "$interop_verify_status" -ne 0 ]; then
        echo "La verificación final con Sled falló (código=$interop_verify_status):"
        cat "$tmp/interop-vm.err" | head -8
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
      echo "Todos los programas asignados coincidieron en stdout, stderr y código de salida; los codificadores GIF/WebP tienen los mismos archivos de salida y sus pruebas comparan SHA-256 de los píxeles decodificados."
    fi
  } >> "$GITHUB_STEP_SUMMARY"
fi
[ "$fail" -eq 0 ] && [ "$unsupported" -eq 0 ] && [ "$build_timeouts" -eq 0 ] && [ "$run_timeouts" -eq 0 ]
