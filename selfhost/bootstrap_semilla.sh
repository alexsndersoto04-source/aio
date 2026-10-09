#!/bin/bash
# Semilla mínima para máquinas con poca memoria (4 GB), a partir de la VM de Rust.
#
# La VM de Rust no puede interpretar el compilador completo si el runtime incluye todos los módulos (135 000
# líneas: pasa de 4 GB). Este script construye un compilador nativo "delgado" (selfhost/titanc0): igual que
# el completo pero con SQLite, tokenize, ONNX, audio, imágenes, PostgreSQL y MySQL sustituidos por
# funciones vacías (native/semilla_lean.py). Después se usa como semilla del punto fijo:
#
#   bash selfhost/bootstrap_semilla.sh            # necesita `zett` en el PATH; deja selfhost/titanc0
#   SEMILLA=selfhost/titanc0 bash selfhost/verify_fixpoint.sh
#
# El titanc0 NO es el compilador final (no sabe hablar con SQLite, MySQL, etc.): el punto fijo lo reemplaza.
# (Sin Rust, la semilla es el binario de selfhost/semilla/: ver selfhost/semilla/LEEME.md.)
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
RT=selfhost/native/runtime.titan
STUB=selfhost/native/std_lean_stub.titan
cp "$RT" "$RT.orig"
trap 'mv -f "$RT.orig" "$RT"; rm -f "$STUB"' EXIT
python3 selfhost/native/semilla_lean.py "$RT.orig" selfhost/native "$RT" "$STUB"
zett run selfhost/build.titan selfhost/build.titan selfhost/titanc0
echo "listo: selfhost/titanc0 (semilla delgada)"
