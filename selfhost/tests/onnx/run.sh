#!/bin/sh
# Verifica std::onnx del runtime nativo contra dos oráculos independientes:
#   - onnxruntime (referencia numérica de gen_models.py)
#   - tract, a través del binario `zett` (la implementación de Rust)
# Variables: TITAN (por defecto selfhost/titan) o COMPILER (un titanc nativo: `titanc fuente salida`), ZETT (por defecto `zett` del
# PATH), OXPY (Python con numpy+onnx+onnxruntime; por defecto python3).
# Salida 0 solo si todo coincide.
set -u
here=$(cd "$(dirname "$0")" && pwd)
TITAN=${TITAN:-$here/../../titan}
ZETT=${ZETT:-$(command -v zett)}
OXPY=${OXPY:-python3}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
unset GITHUB_ACTIONS
fail=0

mkdir -p "$work/m"
"$OXPY" "$here/gen_models.py" "$work/m" >/dev/null 2>&1 || { echo "no se pudieron generar los modelos (¿numpy/onnx/onnxruntime?)"; exit 2; }
for t in api errors bad; do
    if [ -n "${COMPILER:-}" ]; then
        "$COMPILER" "$here/$t.titan" "$work/$t" >/dev/null || { echo "no compila $t.titan"; exit 2; }
    else
        "$TITAN" compile "$here/$t.titan" -o "$work/$t" >/dev/null || { echo "no compila $t.titan"; exit 2; }
    fi
done

echo "== modelos: nativo vs onnxruntime vs tract =="
"$OXPY" "$here/compare.py" "$work/m" "$work/api" "$ZETT" || fail=1

echo "== errores de la API: nativo vs tract (idénticos) =="
mkdir -p "$work/empty"
"$work/errors" "$work/m/mlp" "$work/empty" > "$work/e_nat.txt" 2>&1
"$ZETT" run "$here/errors.titan" "$work/m/mlp" "$work/empty" > "$work/e_tr.txt" 2>&1
if diff "$work/e_nat.txt" "$work/e_tr.txt" >/dev/null; then
    echo "OK: $(wc -l < "$work/e_nat.txt") líneas idénticas"
else
    diff "$work/e_nat.txt" "$work/e_tr.txt"; fail=1
fi

echo "== archivos inválidos: siempre error =="
"$work/bad" "$work/m" > "$work/b.txt" 2>&1
cat "$work/b.txt"
grep -q "(mal)" "$work/b.txt" && fail=1
[ "$(grep -c ' => ERR$' "$work/b.txt")" = 4 ] || fail=1

[ $fail = 0 ] && echo "ONNX: TODO OK" || echo "ONNX: HAY FALLOS"
exit $fail
