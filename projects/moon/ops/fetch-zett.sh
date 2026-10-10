#!/bin/bash
# Deja `titan` (el compilador, construido desde las fuentes de ESTE repo, sin Rust) en
# projects/moon/bin/zett (un enlace al ejecutable: titan busca su runtime en selfhost/native/,
# junto al ejecutable real, así que no se copia).
#
# Lo que ejecutas es exactamente lo que construye la CI: bootstrap con la semilla, punto fijo
# y CLI `selfhost/titan`.
set -e
OPS="$(cd "$(dirname "$0")" && pwd)"
MOON="$OPS/.."
REPO_DIR="$(cd "$MOON/../.." && pwd)"
cd "$REPO_DIR"
[ -x selfhost/titan ] || bash selfhost/bootstrap.sh
mkdir -p "$MOON/bin"
ln -sf "$REPO_DIR/selfhost/titan" "$MOON/bin/zett"
"$MOON/bin/zett" version
echo "OK: $MOON/bin/zett -> $REPO_DIR/selfhost/titan"
