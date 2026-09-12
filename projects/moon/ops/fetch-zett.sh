#!/bin/bash
# Compila el binario `zett` (Titan) desde las fuentes de ESTE repo y lo deja en
# projects/moon/bin/zett.
#
# Antes se bajaba un binario ya compilado (de la release v1.0.0 o de la rama
# espejo `tools-zett-x86_64`). Se cambió a compilar aquí por dos razones:
#   1. Los binarios publicados son de versiones anteriores del lenguaje y puede
#      que no compilen este Moon; la rama espejo llegó a contener un archivo de
#      relleno (`echo zett-fake-v2`), no un binario real.
#   2. Compilando en el repo, lo que ejecutas es exactamente lo que verifica la
#      CI: el E2E completo corre contra `target/release/titan` recién compilado.
set -e
OPS="$(cd "$(dirname "$0")" && pwd)"
MOON="$OPS/.."
REPO_DIR="$(cd "$MOON/../.." && pwd)"
if ! command -v cargo >/dev/null 2>&1; then
  echo "Falta cargo (Rust). Instálalo con rustup: https://rustup.rs" >&2
  exit 1
fi
cd "$REPO_DIR"
cargo build --release -p titan_cli
mkdir -p "$MOON/bin"
cp "$REPO_DIR/target/release/titan" "$MOON/bin/zett"
chmod +x "$MOON/bin/zett"
"$MOON/bin/zett" --version
echo "OK: $MOON/bin/zett"
