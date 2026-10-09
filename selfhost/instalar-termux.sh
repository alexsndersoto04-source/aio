#!/data/data/com.termux/files/usr/bin/bash
# Instala Titan en Termux (Android, ARM64).
#
#   bash instalar-termux.sh [URL del paquete titan-vX-linux-aarch64.tar.gz]
#
# Sin argumento descarga el de la última release de GitHub. Instala en $PREFIX/opt/titan y enlaza
# $PREFIX/bin/titan. En ARM64 Titan compila con LLVM, así que también instala clang.
#
# Estado honesto: el ejecutable ARM64 se verifica en CI sobre Linux ARM64 (no en un teléfono). Si algo
# falla en tu dispositivo, abre un issue con la salida de `titan version` y `uname -a`.
set -eu
if [ "$(uname -m)" != "aarch64" ]; then
  echo "este paquete es para ARM64 (aarch64); esta máquina es $(uname -m)" >&2
  exit 2
fi
PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"
URL="${1:-https://github.com/alexsndersoto04-source/aio/releases/latest/download/titan-linux-aarch64.tar.gz}"
pkg install -y clang curl tar
command -v ld.lld > /dev/null 2>&1 || pkg install -y lld || true
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fL "$URL" -o "$tmp/titan.tar.gz"
mkdir -p "$PREFIX/opt/titan"
tar -xzf "$tmp/titan.tar.gz" -C "$tmp"
rm -rf "$PREFIX/opt/titan"
mv "$tmp"/titan-* "$PREFIX/opt/titan"
ln -sf "$PREFIX/opt/titan/titan" "$PREFIX/bin/titan"
# zett (el gestor de paquetes) viene en el paquete como un enlace a titan; los paquetes antiguos no lo traen.
if [ -e "$PREFIX/opt/titan/zett" ]; then ln -sf "$PREFIX/opt/titan/zett" "$PREFIX/bin/zett"; fi
titan version
if command -v zett > /dev/null 2>&1; then zett version; fi
echo "listo. Prueba: printf 'fn main() {\n    println(\"hola desde Termux\")\n}\n' > hola.titan && titan run hola.titan"
echo "(la primera compilación tarda más: genera una caché del runtime junto al ejecutable)"
