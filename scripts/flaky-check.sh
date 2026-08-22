#!/usr/bin/env bash
#
# flaky-check.sh — caza tests intermitentes ejecutandolos muchas veces.
#
# Los tests que dependen del reloj (sockets locales, deadlines, hilos) pasan
# siempre en una maquina rapida y descansada, y fallan de vez en cuando en un
# runner de CI lento y cargado. Correrlos UNA vez no prueba nada; correrlos
# treinta veces seguidas en la maquina mas lenta que tengas, si.
#
# Un telefono con Termux es justo esa maquina. Si estos tests sobreviven a N
# repeticiones ahi, los margenes son de sobra para cualquier runner de GitHub,
# incluido macOS, que es donde este proyecto ha visto fallos intermitentes.
#
# Sale con codigo 0 solo si TODAS las repeticiones pasaron. Cuando algo falla,
# imprime el nombre exacto del test y deja el log completo en el directorio
# raiz del repositorio.

set -uo pipefail

ITERATIONS=10
BUILD_JOBS=""
RUN_ALL=0

usage() {
    cat <<'EOF'
Uso: scripts/flaky-check.sh [-n REPETICIONES] [-j JOBS] [-a] [-h]

  -n N   repeticiones (por defecto 10)
  -j N   jobs de compilacion; usa -j 1 en Termux con poca RAM
  -a     repetir el workspace completo en vez del grupo sensible al tiempo
  -h     esta ayuda

Ejemplos:
  scripts/flaky-check.sh                # 10 repeticiones del grupo sensible
  scripts/flaky-check.sh -n 30          # 30 repeticiones
  scripts/flaky-check.sh -j 1 -n 20     # Termux: compila con un solo job
  scripts/flaky-check.sh -a -n 5        # 5 repeticiones de toda la suite
EOF
}

while getopts ":n:j:ah" option; do
    case "$option" in
        n) ITERATIONS="$OPTARG" ;;
        j) BUILD_JOBS="$OPTARG" ;;
        a) RUN_ALL=1 ;;
        h) usage; exit 0 ;;
        *) echo "opcion desconocida: -$OPTARG" >&2; usage >&2; exit 1 ;;
    esac
done

case "$ITERATIONS" in
    ''|*[!0-9]*) echo "FAIL: -n necesita un entero positivo" >&2; exit 1 ;;
esac
if [ "$ITERATIONS" -lt 1 ]; then
    echo "FAIL: -n necesita un entero positivo" >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "FAIL: cargo no esta en el PATH. En Termux: pkg install rust" >&2
    exit 1
fi

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT" || exit 1

# Grupos sensibles al tiempo: "<paquete>|<filtro>|<descripcion>". El filtro se
# pasa tal cual al harness, asi que acepta rutas de modulo (server_mod::tests)
# o nombres sueltos (tcp_round_trip_works_across_tasks).
GROUPS="titan_stdlib|server_mod::tests|servidor HTTP/WebSocket sobre sockets reales
titan_stdlib|redis_mod::tests|cliente Redis contra servidores de prueba
titan_stdlib|game::tests|motor de timing (delta y FPS medidos con el reloj)
titan_stdlib|fswatch_mod::tests|vigilancia de ficheros con timeouts
titan_stdlib|process_mod::tests|procesos en segundo plano y drenado de tuberias
titan_sqlite|pool_reuses_connections_and_enforces_timeout|pool SQLite con timeout
titan_vm|tcp_round_trip_works_across_tasks|TCP entre tareas de la VM
titan_vm|advanced_http_client_is_callable_from_titan|cliente HTTP de la VM
titan_vm|tasks_execute_on_threads_and_join|spawn y join reales
titan_vm|channels_communicate_between_tasks|canales entre tareas
titan_vm|select_returns_channel_index_and_value|select con timeout
titan_vm|cancellation_stops_cooperative_tasks|cancelacion cooperativa
titan_vm|dropping_root_vm_cancels_unjoined_tasks_and_releases_runtime|limpieza al soltar la VM"

JOBS_FLAG=""
if [ -n "$BUILD_JOBS" ]; then
    JOBS_FLAG="-j $BUILD_JOBS"
fi

# Logs de corridas anteriores: fuera, para que el resumen no mienta.
rm -f "$ROOT"/flaky-check-failure-*.log

echo "==> Compilando los binarios de test (una sola vez)"
# shellcheck disable=SC2086
if [ "$RUN_ALL" -eq 1 ]; then
    CARGO_INCREMENTAL=0 cargo test --workspace --all-targets --no-run $JOBS_FLAG || exit 1
else
    CARGO_INCREMENTAL=0 cargo test -p titan_stdlib -p titan_sqlite -p titan_vm --no-run $JOBS_FLAG || exit 1
fi

LOG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/titan-flaky.XXXXXXXX")"
trap 'rm -rf "$LOG_DIR"' EXIT

report_failure() {
    tag="$1"
    log="$2"
    kept="$ROOT/flaky-check-failure-${tag}.log"
    cp "$log" "$kept"
    grep -E '^test .+ \.\.\. FAILED$' "$log" |
        sed 's/^test //; s/ \.\.\. FAILED$//; s/^/      test: /'
    grep -E "^thread '.+' panicked at " "$log" | head -3 | sed 's/^/      /'
    echo "      log completo: $(basename "$kept")"
}

echo "==> $ITERATIONS repeticiones"
iteration=1
while [ "$iteration" -le "$ITERATIONS" ]; do
    printf '  [%2d/%2d]\n' "$iteration" "$ITERATIONS"
    iteration_ok=1

    if [ "$RUN_ALL" -eq 1 ]; then
        log="$LOG_DIR/workspace-$iteration.log"
        if cargo test --workspace --all-targets --no-fail-fast >"$log" 2>&1; then
            echo "    ok  workspace completo"
        else
            iteration_ok=0
            echo "    FALLO  workspace completo"
            report_failure "workspace-$iteration" "$log"
        fi
    else
        # Sin pipe: el bucle debe correr en este shell para conservar el estado.
        while IFS='|' read -r package filter label; do
            [ -z "$package" ] && continue
            log="$LOG_DIR/${package}-${filter}-${iteration}.log"
            if cargo test -p "$package" "$filter" >"$log" 2>&1; then
                echo "    ok  $package :: $label"
            else
                iteration_ok=0
                echo "    FALLO  $package :: $label"
                report_failure "${package}-${filter}-${iteration}" "$log"
            fi
        done <<EOF
$GROUPS
EOF
    fi

    if [ "$iteration_ok" -eq 0 ]; then
        echo "  Repeticion $iteration en rojo."
    fi
    iteration=$((iteration + 1))
done

echo
if ls "$ROOT"/flaky-check-failure-*.log >/dev/null 2>&1; then
    echo "RESULTADO: hubo fallos. Tests implicados:"
    grep -hE '^test .+ \.\.\. FAILED$' "$ROOT"/flaky-check-failure-*.log 2>/dev/null |
        sed 's/^test //; s/ \.\.\. FAILED$//' | sort -u | sed 's/^/  - /'
    echo
    echo "Logs completos: $ROOT/flaky-check-failure-*.log"
    echo "Esos nombres identifican el test exacto: son lo que hay que pegar en el reporte."
    exit 1
fi

echo "RESULTADO: $ITERATIONS/$ITERATIONS repeticiones en verde."
echo "Los margenes aguantan en esta maquina; si es un telefono, aguantan en CI."
exit 0
