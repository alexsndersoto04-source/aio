#!/bin/bash
# Arma el paquete descargable de Titan: titan-<versión>-linux-<arq>.tar.gz con la CLI `titan`, el runtime
# (native/, que el compilador lee al compilar cada programa) y las instrucciones.
#
#   bash selfhost/empaquetar.sh <ejecutable titan> <x86_64|aarch64> [directorio de salida]
#
# El ejecutable de aarch64 se obtiene con: titan compile selfhost/titan.titan -o titan-aarch64 --target aarch64
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exe="$(readlink -f "$1")"
arch="$2"
mkdir -p "${3:-.}"
out="$(readlink -f "${3:-.}")"
case "$arch" in x86_64|aarch64) ;; *) echo "arquitectura: x86_64 o aarch64" >&2; exit 2 ;; esac
# La versión sale del propio ejecutable; si no puede ejecutarse (paquete
# cruzado de otra arquitectura) se lee del código fuente — nunca un valor fijo
# (antes el respaldo decía v1.2.0 y el paquete de aarch64 salía mal nombrado).
src_ver="$(sed -n '/fn cl_version/,/^}/p' "$ROOT/selfhost/titan.titan" | grep -o '"[0-9][0-9.]*"' | head -1 | tr -d '"')"
[ -n "$src_ver" ] || src_ver="0.0.0"
ver="$(grep -o 'v[0-9][0-9.]*' <<< "$("$exe" version 2> /dev/null || echo "v$src_ver")" | head -1 || true)"
[ -n "$ver" ] || ver="v$src_ver"
name="titan-$ver-linux-$arch"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/$name"
cp "$exe" "$tmp/$name/titan"
chmod +x "$tmp/$name/titan"
ln -s titan "$tmp/$name/zett"   # zett: el gestor de paquetes (el mismo ejecutable, con otro nombre)
( cd "$ROOT/selfhost" && find native -name '*.titan' | sort | tar -cf - -T - ) | tar -xf - -C "$tmp/$name"
cat > "$tmp/$name/LEEME.txt" <<TXT
Titan $ver — Linux $arch

  ./titan version
  ./titan run programa.titan
  ./titan compile programa.titan -o programa
  ./zett                      (el gestor de paquetes: zett add, fetch, update, pack, publish)

El ejecutable busca el runtime en la carpeta native/ que está junto a él: no los separes.
$(if [ "$arch" = aarch64 ]; then echo "En ARM64 Titan compila con LLVM: hace falta clang con lld (Termux: pkg install clang; Debian/Ubuntu: apt install clang lld)."; else echo "En x86-64 no hace falta nada más."; fi)
Más información: https://github.com/alexsndersoto04-source/aio
TXT
tar -C "$tmp" -czf "$out/$name.tar.gz" "$name"
( cd "$out" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256" )
echo "$out/$name.tar.gz"
