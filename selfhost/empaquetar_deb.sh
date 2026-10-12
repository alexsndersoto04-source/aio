#!/bin/bash
# Arma el paquete .deb que instalan los gestores del sistema con el nombre `zett`:
#
#   apt install ./zett_<versión>_<arq>.deb        (Debian/Ubuntu)
#   apt install ./zett_<versión>_<arq>.termux.deb (Termux: pkg/apt del prefijo)
#
# Dentro van el ejecutable (`titan`), el enlace `zett` (el mismo binario con el nombre del gestor de
# paquetes de Titan) y el runtime `native/` que el compilador lee al compilar cada programa.
#
#   bash selfhost/empaquetar_deb.sh <ejecutable> <amd64|arm64|armhf> [directorio de salida] [--termux] [--version V]
#
# El paquete es determinista: el mismo ejecutable y el mismo runtime dan el mismo .deb byte a byte
# (las fechas se fijan con SOURCE_DATE_EPOCH, la del último commit).
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exe_arg="${1:?uso: empaquetar_deb.sh <ejecutable> <amd64|arm64|armhf> [salida] [--termux] [--version V]}"
arch="${2:?falta la arquitectura: amd64, arm64 o armhf}"
out_dir="."
termux=0
ver_override=""
shift 2 || true
while [ $# -gt 0 ]; do
  case "$1" in
    --termux) termux=1 ;;
    --version) ver_override="${2:?--version necesita un valor}"; shift ;;
    -*) echo "opción desconocida: $1" >&2; exit 2 ;;
    *) out_dir="$1" ;;
  esac
  shift
done
exe="$(readlink -f "$exe_arg")"
[ -f "$exe" ] || { echo "no existe el ejecutable: $exe_arg" >&2; exit 2; }
mkdir -p "$out_dir"
out="$(readlink -f "$out_dir")"

# Versión: la que dice el propio ejecutable; si no puede correr aquí (paquete de
# otra arquitectura) se usa --version o, en su defecto, la del código fuente.
src_ver="$(sed -n '/fn cl_version/,/^}/p' "$ROOT/selfhost/titan.titan" | grep -o '"[0-9][0-9.]*"' | head -1 | tr -d '"')"
[ -n "$src_ver" ] || src_ver="0.0.0"
ver="$ver_override"
if [ -z "$ver" ]; then
  ver="$(grep -o 'v[0-9][0-9.]*' <<< "$("$exe" version 2> /dev/null || echo v$src_ver)" | head -1 | sed 's/^v//' || true)"
fi
[ -n "$ver" ] || ver="$src_ver"

# Nombre de arquitectura del paquete y del gestor de paquetes de cada sistema.
if [ "$termux" = 1 ]; then
  case "$arch" in
    amd64) deb_arch="x86_64" ;;
    arm64) deb_arch="aarch64" ;;
    armhf) deb_arch="arm" ;;
    *) echo "arquitectura: amd64, arm64 o armhf" >&2; exit 2 ;;
  esac
  prefix="/data/data/com.termux/files/usr"
  sufijo=".termux"
else
  case "$arch" in
    amd64 | arm64 | armhf) deb_arch="$arch" ;;
    *) echo "arquitectura: amd64, arm64 o armhf" >&2; exit 2 ;;
  esac
  prefix="/usr"
  sufijo=""
fi

# `clang` y `lld` solo hacen falta donde el backend compila con LLVM (ARM64 y ARM de 32 bits);
# en x86-64 el compilador escribe el ejecutable él mismo, sin libc y sin herramientas externas.
depends=""
recommends=""
case "$arch" in
  arm64 | armhf) depends="clang, lld" ;;
  amd64) recommends="clang, lld" ;;
esac

name="zett_${ver}_${deb_arch}${sufijo}"
pkg="$(mktemp -d)"
trap 'rm -rf "$pkg"' EXIT
mkdir -p "$pkg/DEBIAN" "$pkg$prefix/lib/zett" "$pkg$prefix/bin" "$pkg$prefix/share/doc/zett"
cp "$exe" "$pkg$prefix/lib/zett/titan"
chmod 755 "$pkg$prefix/lib/zett/titan"
ln -s titan "$pkg$prefix/lib/zett/zett"          # zett: el gestor de paquetes (el mismo ejecutable)
( cd "$ROOT/selfhost" && find native -name '*.titan' | sort | tar -cf - -T - ) | tar -xf - -C "$pkg$prefix/lib/zett"
ln -s "../lib/zett/titan" "$pkg$prefix/bin/titan"
ln -s "../lib/zett/titan" "$pkg$prefix/bin/zett"

# Licencia y notas, donde Debian espera la documentación del paquete.
cp "$ROOT/LICENSE" "$pkg$prefix/share/doc/zett/copyright"
cat > "$pkg$prefix/share/doc/zett/LEEME.txt" <<TXT
Titan $ver — paquete \`zett\` ($deb_arch)

  titan version
  titan run programa.titan
  titan compile programa.titan -o programa
  zett                        (el gestor de paquetes: zett add, fetch, update, pack, publish)

El ejecutable busca el runtime en la carpeta native/ que está junto a él, dentro de
$prefix/lib/zett/: no los separes.
$(if [ "$arch" = amd64 ]; then echo "En x86-64 no hace falta nada más."; else echo "En ARM (64 o 32 bits) Titan compila con LLVM: hace falta clang con lld${termux:+ (Termux: pkg install clang lld; Debian: apt install clang lld)}."; fi)
Más información: https://github.com/alexsndersoto04-source/aio
TXT

installed_size="$(du -sk --apparent-size "$pkg" | cut -f1)"
# El bloque de control se escribe línea a línea: una línea vacía lo terminaría
# (los campos condicionales no pueden dejar huecos).
{
  echo "Package: zett"
  echo "Version: $ver"
  echo "Architecture: $deb_arch"
  echo "Maintainer: alexsndersoto04-source <266633823+alexsndersoto04-source@users.noreply.github.com>"
  echo "Installed-Size: $installed_size"
  echo "Section: devel"
  echo "Priority: optional"
  echo "Homepage: https://github.com/alexsndersoto04-source/aio"
  echo "Provides: titan"
  [ -n "$depends" ] && echo "Depends: $depends"
  [ -n "$recommends" ] && echo "Recommends: $recommends"
  echo "Description: Titan: compilador que se compila a sí mismo, y zett (su gestor de paquetes)"
  echo " Titan es un lenguaje de programación con su compilador escrito en Titan"
  echo " (autoalojado, punto fijo titanc1 == titanc2 == titanc3), sin libc y sin"
  echo " dependencias en tiempo de ejecución: el ejecutable habla con el núcleo."
  echo " Este paquete instala el compilador (titan) y, con el nombre zett, su"
  echo " gestor de paquetes (add, fetch, install, update, keygen, pack, publish)."
  [ "$arch" != amd64 ] && echo " Compilar para ARM (64 o 32 bits) usa clang y lld."
} > "$pkg/DEBIAN/control"

# La carpeta de documentación no debe ir vacía ni con permisos raros (dpkg).
find "$pkg" -type d -exec chmod 755 {} +
find "$pkg" -type f -exec chmod go-w {} +

export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "$ROOT" log -1 --format=%ct 2> /dev/null || echo 0)}"
dpkg-deb --build --root-owner-group "$pkg" "$out/$name.deb" > /dev/null
( cd "$out" && sha256sum "$name.deb" > "$name.deb.sha256" )
echo "$out/$name.deb"
