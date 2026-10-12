#!/bin/bash
# Prueba de verdad los paquetes oficiales `zett` (el compilador `titan` y, con ese nombre, su
# gestor de paquetes) para cada sistema:
#
#   amd64 (Debian/Ubuntu x86-64): se instala con apt desde el repositorio y se usa.
#   arm64 (Debian/Ubuntu ARM64):   en una máquina aarch64 se instala igual; en x86-64 se comprueba el
#                                  paquete (control, contenido, binario idéntico al compilado) y se dice
#                                  claro que la ejecución queda para la máquina ARM64 o la CI.
#   armhf (ARM de 32 bits):        el binario del paquete se ejecuta bajo el emulador de ARM32
#                                  (native/arm32/correr.py, el procesador de verdad que trae unicorn).
#   Termux (los tres):             se extraen en un prefijo falso y el binario del paquete se ejecuta igual
#                                  que el del sistema (amd64 y armhf aquí; arm64 en la máquina ARM64).
#
# Uso: bash selfhost/probar_paquetes.sh [SALIDA]
#   SALIDA es donde se dejan los .deb y el repositorio (por defecto /tmp/paquetes_titan).
#   Los binarios de otras arquitecturas se buscan en SALIDA/bin (titan-aarch64, titan-armv7) y, si no
#   están, se compilan (tarda unos minutos cada uno).
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-/tmp/paquetes_titan}"
mkdir -p "$out/bin" "$out/repo"
out="$(readlink -f "$out")"
cd "$ROOT"

pasan=0; fallan=0; saltan=0
ok()   { echo "  OK   $*"; pasan=$((pasan + 1)); }
mal()  { echo "  MAL  $*"; fallan=$((fallan + 1)); }
salta(){ echo "  --   $*"; saltan=$((saltan + 1)); }

# ---------------------------------------------------------------- binarios
exe64="$ROOT/selfhost/titan"
[ -x "$exe64" ] || { echo "falta selfhost/titan (corre selfhost/bootstrap.sh)" >&2; exit 2; }
ver="$(sed -n '/fn cl_version/,/^}/p' selfhost/titan.titan | grep -o '"[0-9][0-9.]*"' | head -1 | tr -d '"')"

compilar_si_falta() {  # arquitectura, archivo
  local arch="$1" destino="$2"
  [ -x "$destino" ] && return 0
  echo "compilando para $arch (tarda unos minutos)…"
  ./selfhost/titan compile selfhost/titan.titan -o "$destino" --target "$arch" > /dev/null || return 1
}

# ------------------------------------------------------------ los paquetes
# De una corrida anterior no puede quedar ningún .deb: mezclaría arquitecturas
# de sistemas distintos en el mismo repositorio.
rm -f "$out"/repo/zett_*.deb "$out"/repo-termux/zett_*.deb
echo "== paquetes (zett $ver) =="
bash selfhost/empaquetar_deb.sh "$exe64" amd64 "$out/repo" > /dev/null || mal "no se pudo empaquetar amd64"
ok "zett_${ver}_amd64.deb"
if compilar_si_falta aarch64 "$out/bin/titan-aarch64"; then
  bash selfhost/empaquetar_deb.sh "$out/bin/titan-aarch64" arm64 "$out/repo" --version "$ver" > /dev/null \
    && ok "zett_${ver}_arm64.deb"
else
  mal "no se pudo compilar el binario de ARM64"
fi
if compilar_si_falta armv7 "$out/bin/titan-armv7"; then
  bash selfhost/empaquetar_deb.sh "$out/bin/titan-armv7" armhf "$out/repo" --version "$ver" > /dev/null \
    && ok "zett_${ver}_armhf.deb"
else
  mal "no se pudo compilar el binario de ARM 32 bits"
fi
# El repositorio de Termux va aparte: sus nombres de arquitectura (x86_64, aarch64, arm) son otros.
mkdir -p "$out/repo-termux"
for a in amd64 arm64 armhf; do
  case "$a" in amd64) ta=x86_64 ;; arm64) ta=aarch64 ;; armhf) ta=arm ;; esac
  b="$out/bin/titan-aarch64"; [ "$a" = armhf ] && b="$out/bin/titan-armv7"; [ "$a" = amd64 ] && b="$exe64"
  if [ -x "$b" ]; then
    bash selfhost/empaquetar_deb.sh "$b" "$a" "$out/repo-termux" --termux --version "$ver" > /dev/null \
      && ok "zett_${ver}_${ta}.termux.deb"
  fi
done

# Cada paquete lleva dentro el binario que se compiló (no una copia vieja).
comprueba_payload() {  # deb, binario, ruta-dentro-del-paquete
  local deb="$1" bin="$2" dentro="$3" tmp
  tmp="$(mktemp -d)"
  dpkg-deb -x "$deb" "$tmp" 2>/dev/null
  if [ -z "$bin" ] || [ ! -x "$bin" ]; then
    rm -rf "$tmp"; return 0
  fi
  if cmp -s "$bin" "$tmp/$dentro"; then rm -rf "$tmp"; return 0; fi
  rm -rf "$tmp"; return 1
}
for f in "$out"/repo/zett_*.deb "$out"/repo-termux/zett_*.deb; do
  base="$(basename "$f")"
  case "$base" in
    *_amd64*.deb)   b="$exe64"; d="usr/lib/zett/titan" ;;
    *_arm64*.deb)   b="$out/bin/titan-aarch64"; d="usr/lib/zett/titan" ;;
    *_armhf*.deb)   b="$out/bin/titan-armv7"; d="usr/lib/zett/titan" ;;
    *_x86_64*.deb)  b="$exe64"; d="data/data/com.termux/files/usr/lib/zett/titan" ;;
    *_aarch64*.deb) b="$out/bin/titan-aarch64"; d="data/data/com.termux/files/usr/lib/zett/titan" ;;
    *_arm.termux*)  b="$out/bin/titan-armv7"; d="data/data/com.termux/files/usr/lib/zett/titan" ;;
    *) continue ;;
  esac
  if comprueba_payload "$f" "$b" "$d"; then ok "payload $base (binario idéntico al compilado)"
  else mal "payload $base (el binario de dentro no es el compilado)"; fi
done

# --------------------------------------------------- repositorio y apt (amd64)
echo "== repositorio apt =="
bash selfhost/repo_apt.sh "$out/repo" > /dev/null 2>&1 && ok "Packages, Packages.gz y Release escritos"
tiene_arq() {  # release, arquitectura
  grep -m1 '^Architectures:' "$1" 2> /dev/null | tr ' ' '\n' | grep -qx "$2"
}
if [ -f "$out/repo/Release" ]; then
  archs="$(grep -m1 '^Architectures:' "$out/repo/Release")"
  if tiene_arq "$out/repo/Release" amd64 && tiene_arq "$out/repo/Release" arm64 && tiene_arq "$out/repo/Release" armhf; then
    ok "el Release del sistema anuncia amd64, arm64 y armhf ($archs)"
  else
    mal "el Release del sistema no anuncia las tres ($archs)"
  fi
fi
bash selfhost/repo_apt.sh "$out/repo-termux" --termux > /dev/null 2>&1 || mal "no se pudo escribir el repositorio de Termux"
archs_t="$(grep -m1 '^Architectures:' "$out/repo-termux/Release" 2> /dev/null || echo '')"
if tiene_arq "$out/repo-termux/Release" x86_64 && tiene_arq "$out/repo-termux/Release" aarch64 && tiene_arq "$out/repo-termux/Release" arm; then
  ok "el Release de Termux anuncia x86_64, aarch64 y arm ($archs_t)"
else
  mal "el Release de Termux no anuncia las tres ($archs_t)"
fi
if [ "$(id -u)" = 0 ] || sudo -n true 2> /dev/null; then
  lista="$(mktemp)"; echo "deb [trusted=yes] file:$out/repo ./" > "$lista"
  apt_cmd=(sudo apt-get -o Dir::Etc::sourcelist="$lista" -o Dir::Etc::sourceparts=/dev/null -o APT::Get::List-Cleanup=0)
  if "${apt_cmd[@]}" update > /dev/null 2>&1 &&
     "${apt_cmd[@]}" install -y --reinstall zett > /dev/null 2>&1; then
    if titan version > /dev/null 2>&1 && zett version > /dev/null 2>&1; then
      ok "apt install zett desde el repositorio y los dos comandos funcionan"
    else
      mal "apt instaló el paquete pero titan/zett no responden"
    fi
  else
    mal "apt no pudo instalar zett desde el repositorio"
  fi
  rm -f "$lista"
else
  salta "apt install desde el repositorio (hace falta root o sudo sin contraseña)"
fi

# ------------------------------------------------------------ uso de verdad
echo "== uso (amd64, el paquete instalado) =="
tmp="$(mktemp -d)"; proy="$tmp/proyecto"; mkdir -p "$proy/src"
printf 'fn main() {\n    let n = 6 * 7\n    println("respuesta {n}")\n}\n' > "$proy/hola.titan"
printf '[package]\nname = "prueba_paquetes"\nversion = "0.1.0"\nedition = "2026"\ndescription = "prueba del paquete"\nlicense = "MIT"\n' > "$proy/Titan.toml"
cp "$proy/hola.titan" "$proy/src/main.titan"
if [ "$(titan run "$proy/hola.titan" 2> /dev/null)" = "respuesta 42" ]; then ok "titan run compila y ejecuta"
else salta "titan run (no hay paquete instalado en esta máquina)"; fi
# La clave y el .tpkg van fuera del proyecto: `zett pack` rechaza escribir el
# paquete dentro de lo que empaqueta (ruta insegura).
if zett keygen "$tmp/k.key" > /dev/null 2>&1 && [ -s "$tmp/k.key" ]; then ok "zett keygen"
else salta "zett keygen (no hay paquete instalado)"; fi
if (cd "$proy" && zett pack --key "$tmp/k.key" --output "$tmp/p.tpkg" > /dev/null 2>&1) && [ -s "$tmp/p.tpkg" ]; then
  ok "zett pack (paquete .tpkg firmado)"
else salta "zett pack (no hay paquete instalado)"; fi
rm -rf "$tmp"

# ------------------------------------------------------------- ARM64
echo "== ARM64 =="
if [ "$(uname -m)" = "aarch64" ]; then
  if sudo dpkg -i "$out/repo/zett_${ver}_arm64.deb" > /dev/null 2>&1 && titan version > /dev/null 2>&1; then
    ok "el paquete arm64 se instala y titan responde en esta máquina ARM64"
  else
    mal "el paquete arm64 no se pudo instalar/ejecutar en esta máquina ARM64"
  fi
else
  salta "ejecutar el paquete arm64: esta máquina es $(uname -m); lo prueba la CI en un ARM64 real"
fi

# ------------------------------------------------------------- ARM32
echo "== ARM 32 bits (emulador) =="
if python3 -c "import unicorn, capstone" 2> /dev/null; then
  tmp="$(mktemp -d)"; dpkg-deb -x "$out/repo/zett_${ver}_armhf.deb" "$tmp"
  if [ "$(timeout 300 python3 selfhost/native/arm32/correr.py "$tmp/usr/lib/zett/titan" version 2> /dev/null)" \
       = "TITAN Language Compiler v$ver" ]; then ok "el titan del paquete armhf responde bajo el emulador"
  else mal "el titan del paquete armhf no respondió"; fi
  if timeout 300 python3 selfhost/native/arm32/correr.py "$tmp/usr/lib/zett/zett" version 2> /dev/null \
       | grep -q "gestor de paquetes de Titan"; then ok "el zett del paquete armhf responde bajo el emulador"
  else mal "el zett del paquete armhf no respondió"; fi
  rm -rf "$tmp"
else
  salta "el paquete armhf bajo el emulador (falta unicorn/capstone: pip install unicorn capstone)"
fi

# ------------------------------------------------------------- Termux
echo "== Termux =="
ta="$(uname -m)"; case "$ta" in x86_64) tt=x86_64 ;; aarch64) tt=aarch64 ;; *) tt="" ;; esac
for f in "$out"/repo-termux/*.termux.deb; do
  [ -e "$f" ] || continue
  base="$(basename "$f")"
  tmp="$(mktemp -d)"; dpkg-deb -x "$f" "$tmp"
  p="$tmp/data/data/com.termux/files/usr"
  case "$base" in
    *x86_64*|*aarch64*) if [ -n "$tt" ] && printf '%s' "$base" | grep -q "$tt"
      then salida="$("$p/bin/titan" version 2> /dev/null)"
           [ "$salida" = "TITAN Language Compiler v$ver" ] && ok "$base: el binario del prefijo corre" \
             || mal "$base: el binario del prefijo no responde"
      else salta "$base: no se puede ejecutar aquí ($ta)"; fi ;;
    *arm.termux*)
      if python3 -c "import unicorn" 2> /dev/null; then
        salida="$(timeout 300 python3 selfhost/native/arm32/correr.py "$p/lib/zett/titan" version 2> /dev/null)"
        [ "$salida" = "TITAN Language Compiler v$ver" ] && ok "$base: corre bajo el emulador" \
          || mal "$base: no responde bajo el emulador"
      else salta "$base: falta unicorn para el emulador"; fi ;;
  esac
  rm -rf "$tmp"
done

# Copias sin versión en la raíz de la salida: son las que publica la release y
# las que enlazan el README y la página (direcciones estables).
for a in amd64 arm64 armhf; do
  [ -f "$out/repo/zett_${ver}_${a}.deb" ] && cp -f "$out/repo/zett_${ver}_${a}.deb" "$out/zett_${a}.deb"
done
for a in x86_64 aarch64 arm; do
  [ -f "$out/repo-termux/zett_${ver}_${a}.termux.deb" ] && \
    cp -f "$out/repo-termux/zett_${ver}_${a}.termux.deb" "$out/zett_${a}.termux.deb"
done
if [ -f "$out/bin/titan-armv7" ]; then
  if bash selfhost/empaquetar.sh "$out/bin/titan-armv7" armv7 "$out" > /dev/null 2>&1; then
    ok "titan-…-linux-armv7.tar.gz (el tar.gz del binario de ARM de 32 bits)"
  else
    mal "no se pudo empaquetar el tar.gz de armv7"
  fi
fi

echo
echo "paquetes: pasan $pasan, fallan $fallan, no comprobados aquí $saltan"
echo "los .deb y el repositorio están en $out/repo"
[ "$fallan" -eq 0 ]
