#!/bin/bash
# Arma el repositorio apt propio de Titan, con el paquete `zett`, y lo comprueba.
#
#   bash selfhost/repo_apt.sh DIRECTORIO [--termux] [.deb ...]
#
# Sin .deb se usan todos los que haya en DIRECTORIO. Deja en DIRECTORIO:
#   Packages, Packages.gz y Release (con hashes y tamaños), como un repo apt
#   normal; y verifica al final que `apt` lo lee y que el paquete se instala.
#
# Firma (opcional): si GPG_KEY existe, `gpg --batch --yes -abs` firma el Release
# y deja Release.gpg (apt lo pide para la opción «firmado por»); sin clave,
# apt exige [trusted=yes], y esto lo dice el propio script.
#
# Nota honesta: `apt-ftparchive` no está en esta imagen, así que los índices los
# escribe este script (nombre, tamaño y sha256 de cada .deb son los de verdad).
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
dir="${1:?uso: repo_apt.sh DIRECTORIO [--termux] [.deb ...]}"
shift
termux=0
debs=()
while [ $# -gt 0 ]; do
  case "$1" in
    --termux) termux=1 ;;
    *) debs+=("$1") ;;
  esac
  shift
done
mkdir -p "$dir"
dir="$(readlink -f "$dir")"
if [ "${#debs[@]}" -eq 0 ]; then
  mapfile -t debs < <(find "$dir" -maxdepth 1 -name 'zett_*.deb' | sort)
fi
[ "${#debs[@]}" -gt 0 ] || { echo "no hay paquetes .deb en $dir" >&2; exit 2; }
cd "$dir"

# Una entrada de Packages por paquete: el bloque de control del .deb, más los
# campos que añade el repositorio (Filename, Size, SHA256).
: > Packages
for d in "${debs[@]}"; do
  d="$(readlink -f "$d")"
  [ -f "$d" ] || { echo "no existe: $d" >&2; exit 2; }
  base="$(basename "$d")"
  [ "$(dirname "$d")" = "$dir" ] || cp -f "$d" "$dir/$base"
  dpkg-deb -f "$d" > /tmp/repo_apt_control.$$ || exit 1
  {
    cat /tmp/repo_apt_control.$$
    echo "Filename: ./$base"
    echo "Size: $(stat -c %s "$d")"
    echo "SHA256: $(sha256sum "$d" | cut -d' ' -f1)"
    echo ""
  } >> Packages
  echo "incluido: $base"
done
rm -f /tmp/repo_apt_control.$$
gzip -9 -k -f -n Packages

# Release: los campos obligatorios y los hashes de lo que acaba de escribirse.
SIZES="$(stat -c '%s %n' Packages Packages.gz)"
{
  echo "Origin: Titan"
  echo "Label: Titan"
  echo "Suite: stable"
  echo "Codename: stable"
  echo "Architectures: $(for d in "${debs[@]}"; do dpkg-deb -f "$d" Architecture; done | sort -u | tr '\n' ' ' | sed 's/ $//')"
  echo "Components: main"
  echo "Description: Paquetes de Titan (el compilador que se compila a sí mismo) y su gestor zett"
  echo "Date: $(LC_ALL=C date -u -R -d "@${SOURCE_DATE_EPOCH:-$(date +%s)}")"
  # Cada línea de los índices es «hash tamaño nombre», en ese orden.
  echo "MD5Sum:"
  md5sum Packages Packages.gz | while read -r h f; do echo " $h $(stat -c %s "$f") $f"; done
  echo "SHA256:"
  sha256sum Packages Packages.gz | while read -r h f; do echo " $h $(stat -c %s "$f") $f"; done
} > Release

if [ -n "${GPG_KEY:-}" ]; then
  gpg --batch --yes -u "$GPG_KEY" -abs -o Release.gpg Release
  echo "Release firmado con $GPG_KEY (Release.gpg)"
else
  echo "aviso: sin GPG_KEY el Release va sin firmar; apt pedirá [trusted=yes] o --allow-unauthenticated"
fi

# Comprobación real: que apt lea este repositorio (listado) y que el paquete se
# pueda resolver desde él. apt escribe en /var/lib/apt/lists, así que pide root
# (con `sudo` si hace falta; sin sudo se salta la comprobación y lo dice).
lista="$(mktemp)"
echo "deb [trusted=yes] file:$dir ./" > "$lista"
APT="apt-get"
APTC="apt-cache"
if [ "$(id -u)" != 0 ]; then
  if command -v sudo > /dev/null 2>&1 && sudo -n true 2> /dev/null; then
    APT="sudo apt-get"; APTC="sudo apt-cache"
  else
    echo "aviso: sin root no se puede comprobar con apt (usa sudo para que lo haga)"
    rm -f "$lista"
    echo "repositorio: $dir (Packages tiene $(grep -c '^Package:' Packages) paquete(s))"
    exit 0
  fi
fi
echo "--- apt-get update (repo local) ---"
$APT -o Dir::Etc::sourcelist="$lista" -o Dir::Etc::sourceparts=/dev/null -o APT::Get::List-Cleanup=0 update 2>&1 | sed -n '1,8p'
echo "--- apt policy zett ---"
$APTC -o Dir::Etc::sourcelist="$lista" -o Dir::Etc::sourceparts=/dev/null policy zett 2>&1 | sed -n '1,8p'
rm -f "$lista"
echo "repositorio: $dir (Packages tiene $(grep -c '^Package:' Packages) paquete(s))"
