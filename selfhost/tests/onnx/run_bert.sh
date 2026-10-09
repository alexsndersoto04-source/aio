#!/bin/sh
# BERT en std::onnx (load_bert, load_bert3, run_bert, run_bert3, run_bert_pooled) contra onnxruntime y tract.
# Genera modelos de HuggingFace de verdad con torch.onnx (legado y dynamo). Variables: COMPILER (titanc nativo)
# o TITAN, ZETT, OXPY (Python con torch, transformers, onnx, onnxruntime, numpy, onnxscript).
set -u
here=$(cd "$(dirname "$0")" && pwd)
TITAN=${TITAN:-$here/../../titan}
ZETT=${ZETT:-$(command -v zett)}
OXPY=${OXPY:-python3}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/m"
"$OXPY" "$here/gen_bert.py" "$work/m" 2>&1 | grep -v "^\[torch" || exit 2
if [ -n "${COMPILER:-}" ]; then
    "$COMPILER" "$here/bert.titan" "$work/bert" >/dev/null || { echo "no compila bert.titan"; exit 2; }
else
    "$TITAN" compile "$here/bert.titan" -o "$work/bert" >/dev/null || { echo "no compila bert.titan"; exit 2; }
fi
"$OXPY" "$here/compare_bert.py" "$work/m" "$work/bert" "$ZETT" "$here/bert.titan" || exit 1
# errores de la API (los textos que vienen del contenido del grafo son propios de tract y se enmascaran)
if [ -n "${COMPILER:-}" ]; then
    "$COMPILER" "$here/bert_errors.titan" "$work/bert_errors" >/dev/null || { echo "no compila bert_errors.titan"; exit 2; }
else
    "$TITAN" compile "$here/bert_errors.titan" -o "$work/bert_errors" >/dev/null || { echo "no compila bert_errors.titan"; exit 2; }
fi
mask='s/failed: onnx error: .*/failed: onnx error: .../'
"$work/bert_errors" "$work/m/bert_legacy.onnx" 2>&1 | sed "$mask" > "$work/err_nat.txt"
"$ZETT" run "$here/bert_errors.titan" "$work/m/bert_legacy.onnx" 2>&1 | sed "$mask" > "$work/err_vm.txt"
if diff "$work/err_nat.txt" "$work/err_vm.txt"; then
    echo "errores de BERT: OK ($(wc -l < "$work/err_nat.txt") líneas idénticas)"
else
    echo "errores de BERT: FALLA"
    exit 1
fi
echo "BERT: TODO OK"
