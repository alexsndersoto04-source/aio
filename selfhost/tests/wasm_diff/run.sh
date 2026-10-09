#!/bin/bash
# Prueba diferencial del backend WebAssembly: `zett wasm` (Rust) contra el
# ejecutable compilado de selfhost/wasm_cli.titan. Para cada programa compara
# byte a byte el .wasm, el .map y el .map.json, y la salida estándar (sin rutas).
# Uso: selfhost/tests/wasm_diff/run.sh [programa-titan ...]
# Variables: ZETT (binario de Rust), WASM_TITAN (ejecutable del port, se compila
# con selfhost/titanc3 si no existe).
set -u
cd "$(dirname "$0")/../../.."
ZETT="${ZETT:-zett}"
BIN="${WASM_TITAN:-/tmp/wasm_diff/wasm_titan}"
mkdir -p "$(dirname "$BIN")"
if [ ! -x "$BIN" ]; then
    ./selfhost/titanc3 selfhost/wasm_cli.titan "$BIN" || exit 1
fi
files=("$@")
[ ${#files[@]} -eq 0 ] && files=(selfhost/tests/wasm_diff/*.titan examples/browser/main.titan)
fail=0
for f in "${files[@]}"; do
    name=$(basename "$f" .titan)
    d=/tmp/wasm_diff/$name
    rm -rf "$d"; mkdir -p "$d/rust" "$d/titan"
    "$ZETT" wasm "$f" --output "$d/rust/$name.wasm" > "$d/rust.out" 2>&1; rc1=$?
    "$BIN" "$f" --output "$d/titan/$name.wasm" > "$d/titan.out" 2>&1; rc2=$?
    sed -i "s#$d/rust#OUT#g" "$d/rust.out"; sed -i "s#$d/titan#OUT#g" "$d/titan.out"
    ok=1
    [ $rc1 -eq $rc2 ] || ok=0
    cmp -s "$d/rust.out" "$d/titan.out" || ok=0
    if [ $rc1 -eq 0 ]; then
        for ext in wasm wasm.map wasm.map.json; do
            cmp -s "$d/rust/$name.$ext" "$d/titan/$name.$ext" || { ok=0; echo "  difiere: $name.$ext"; }
        done
    fi
    # Ejecución real del .wasm del port en Node (v22): sin importaciones, el resultado
    # de main() debe coincidir con el de la VM; con importaciones, WebAssembly.validate.
    if [ $rc1 -eq 0 ] && command -v node >/dev/null 2>&1; then
        w=$(node selfhost/tests/wasm_diff/exec.js "$d/titan/$name.wasm" 2>&1 | tail -1)
        v=$("$ZETT" run "$f" 2>&1 | tail -1)
        case "$w" in
            "=> "*) if [ "$v" = "$w" ]; then echo "  node: $w (igual que la VM)"; elif echo "$v" | grep -q "WebAssembly"; then echo "  node: $w (la VM no ejecuta std::wasm/std::web)"; else echo "  node: $w pero la VM da: $v"; ok=0; fi ;;
            VALID) echo "  node: WebAssembly.validate OK (usa importaciones del navegador)" ;;
            *) echo "  node: $w"; ok=0 ;;
        esac
    fi
    if [ $ok -eq 1 ]; then echo "IGUAL  $f (rc=$rc1, $(stat -c %s "$d/rust/$name.wasm" 2>/dev/null || echo -) bytes)"; else echo "DIFIERE $f (rc rust=$rc1 titan=$rc2)"; fail=1; fi
done
# host.js real de examples/browser sobre un DOM mínimo: el registro del DOM debe ser el mismo.
if [ -f /tmp/wasm_diff/main/rust/main.wasm ] && command -v node >/dev/null 2>&1; then
    node selfhost/tests/wasm_diff/host_run.js /tmp/wasm_diff/main/rust/main.wasm > /tmp/wasm_diff/host_rust.json 2>&1
    node selfhost/tests/wasm_diff/host_run.js /tmp/wasm_diff/main/titan/main.wasm > /tmp/wasm_diff/host_titan.json 2>&1
    if cmp -s /tmp/wasm_diff/host_rust.json /tmp/wasm_diff/host_titan.json && grep -q "Evento click ejecutado" /tmp/wasm_diff/host_titan.json; then echo "host.js real + DOM de Node: mismo registro ($(grep -c '' /tmp/wasm_diff/host_titan.json) líneas)"; else echo "host.js: registros distintos o sin eventos"; fail=1; fi
fi
[ $fail -eq 0 ] && echo "TODO IGUAL" || echo "HAY DIFERENCIAS"
exit $fail
