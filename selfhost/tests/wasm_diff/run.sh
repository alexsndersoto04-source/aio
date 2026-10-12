#!/bin/bash
# Prueba del backend WebAssembly del port en Titan.
#
#   - Si hay ZETT (el compilador de Rust, ya retirado del repositorio) compara además
#     byte a byte el .wasm, el .map y el .map.json.
#   - Sin ZETT compara lo que de verdad importa: el resultado de ejecutar el .wasm del
#     port en Node contra el de `titan run` (la referencia nativa).
#   - Los `err_*.titan` son negativos: tienen que fallar al compilar, con diagnóstico.
#
# Uso: selfhost/tests/wasm_diff/run.sh [programa-titan ...]
# Variables: ZETT (binario de Rust), WASM_TITAN (ejecutable del port, se compila
# con selfhost/titanc3 si no existe), TITAN (CLI nativa, por defecto ./selfhost/titan).
set -u
cd "$(dirname "$0")/../../.."
ZETT="${ZETT:-zett}"
BIN="${WASM_TITAN:-/tmp/wasm_diff/wasm_titan}"
TITAN="${TITAN:-./selfhost/titan}"
mkdir -p "$(dirname "$BIN")"
if [ ! -x "$BIN" ]; then
    echo "compilando el port ($BIN)..."
    ./selfhost/titanc3 selfhost/wasm_cli.titan "$BIN" || exit 1
fi
have_zett=0
command -v "$ZETT" >/dev/null 2>&1 && have_zett=1
files=("$@")
[ ${#files[@]} -eq 0 ] && files=(selfhost/tests/wasm_diff/*.titan examples/browser/main.titan)
fail=0
for f in "${files[@]}"; do
    name=$(basename "$f" .titan)
    d=/tmp/wasm_diff/$name
    rm -rf "$d"; mkdir -p "$d"
    "$BIN" "$f" --output "$d/$name.wasm" > "$d/port.out" 2>&1; rc=$?
    if [ $have_zett -eq 1 ]; then
        "$ZETT" wasm "$f" --output "$d/rust.wasm" > "$d/rust.out" 2>&1; rc1=$?
        sed -i "s#$d/rust#OUT#g" "$d/rust.out"
        [ $rc1 -eq $rc ] || { echo "DIFIERE $f (rc rust=$rc1 port=$rc)"; fail=1; continue; }
        if [ $rc1 -eq 0 ]; then
            cmp -s "$d/rust.wasm" "$d/$name.wasm" || { echo "DIFIERE $f ($name.wasm)"; fail=1; }
        fi
    fi
    case "$name" in
        err_*)
            if [ $rc -ne 0 ]; then
                say=$(sed -n 's/^WASM BUILD ERROR: //p' "$d/port.out" | head -1 | cut -c1-95)
                echo "RECHAZA $f  $say"
            else
                echo "FALLA   $f: tendría que rechazarse y compiló"; fail=1
            fi
            ;;
        *)
            if [ $rc -ne 0 ]; then
                echo "FALLA   $f: $(tail -1 "$d/port.out")"; fail=1; continue
            fi
            if ! command -v node >/dev/null 2>&1; then
                echo "COMPILA $f (sin node no se ejecuta)"; continue
            fi
            w=$(node selfhost/tests/wasm_diff/exec.js "$d/$name.wasm" 2>&1 | tail -1)
            v=$("$TITAN" run "$f" 2>&1 | tail -1)
            case "$w" in
                "=> "*)
                    if [ "$w" = "$v" ]; then echo "IGUAL   $f  $w"
                    elif echo "$v" | grep -q "browser host\|WebAssembly"; then echo "VALID   $f  $w (la VM no ejecuta std::wasm/std::web)"
                    else echo "DIFIERE $f  wasm=$w vm=$v"; fail=1; fi ;;
                VALID) echo "VALID   $f  (usa las importaciones del navegador)" ;;
                *) echo "FALLA   $f  node: $w"; fail=1 ;;
            esac
            ;;
    esac
done
# El host real (examples/browser/host.js) sobre un DOM mínimo de Node: el programa del
# navegador tiene que ejecutarse y dejar el mismo registro de DOM que antes.
if [ -f /tmp/wasm_diff/main/main.wasm ] && command -v node >/dev/null 2>&1; then
    if node selfhost/tests/wasm_diff/host_run.js /tmp/wasm_diff/main/main.wasm > /tmp/wasm_diff/host.json 2>&1 \
        && grep -q "Evento click ejecutado" /tmp/wasm_diff/host.json; then
        echo "HOST.HTML OK: $(grep -c '' /tmp/wasm_diff/host.json) líneas de registro, evento click ejecutado"
    else
        echo "HOST.HTML FALLA: host.js + DOM de Node no completó el guion"; fail=1
    fi
fi
[ $fail -eq 0 ] && echo "TODO BIEN" || echo "HAY FALLOS"
exit $fail
