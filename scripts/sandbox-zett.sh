#!/bin/bash
# Deja listo el binario PRECOMPILADO de Titan/Zett en un sandbox Linux x86_64
# (p. ej. el entorno de Arena) sin compilar nada con cargo.
#
#   bash scripts/sandbox-zett.sh        # instala en ~/.local/bin/zett (+ alias titan)
#   zett version
#
# De dónde sale el binario: la rama `binaries` (workflow publish-linux-binary),
# archivo `zett-linux-x86_64`, verificado contra su `.sha256`.
#
# libasound.so.2: el binario enlaza ALSA (cpal, para std::audio). Si el sistema
# no la tiene y no se puede instalar con apt (sandbox sin red a Debian), se
# genera un STUB mínimo con los 48 símbolos versionados que usa el binario. Con
# el stub todo funciona igual salvo la salida de audio real: snd_pcm_open
# devuelve -ENODEV ("no hay dispositivo de audio"), que es lo mismo que pasaría
# en un servidor sin tarjeta de sonido.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="${ZETT_BIN_DIR:-$HOME/.local/bin}"
LIB_DIR="${ZETT_LIB_DIR:-$HOME/.local/lib/zett}"
BRANCH="${ZETT_BRANCH:-binaries}"
mkdir -p "$BIN_DIR" "$LIB_DIR"

echo "==> Descargando rama '$BRANCH' (solo metadatos + binario)"
git -C "$REPO" fetch -q origin "+refs/heads/$BRANCH:refs/remotes/origin/$BRANCH"

REAL="$LIB_DIR/zett-real"
git -C "$REPO" show "origin/$BRANCH:zett-linux-x86_64" > "$REAL"
chmod +x "$REAL"
want="$(git -C "$REPO" show "origin/$BRANCH:zett-linux-x86_64.sha256" | awk '{print $1}')"
got="$(sha256sum "$REAL" | awk '{print $1}')"
if [ "$want" != "$got" ]; then
  echo "SHA-256 no coincide: esperado $want, obtenido $got" >&2
  exit 1
fi
echo "    sha256 OK ($got)"
echo "    fuente: commit $(git -C "$REPO" show "origin/$BRANCH:COMMIT" 2>/dev/null || echo '?')"

# ¿Faltan librerías? -> stub de ALSA
if ldd "$REAL" | grep -q 'libasound.so.2 => not found'; then
  if ! sudo -n apt-get install -y -q libasound2 >/dev/null 2>&1; then
    echo "==> libasound.so.2 no disponible: generando stub en $LIB_DIR"
    tmp="$(mktemp -d)"
    syms="$(objdump -T "$REAL" | awk '/snd_/ {print $(NF-1), $NF}' | tr -d '()')"
    {
      echo '#include <stddef.h>'
      echo '#define ENODEV 19'
      echo "$syms" | while read -r ver sym; do
        case "$sym" in
          snd_pcm_status_sizeof|snd_pcm_hw_params_sizeof|snd_pcm_sw_params_sizeof)
            echo "size_t ${sym}_impl(void){return 4096;}" ;;
          *_malloc)
            echo "int ${sym}_impl(void **p){if(p)*p=NULL;return -ENODEV;}" ;;
          *_free|snd_pcm_status_get_htstamp|snd_pcm_status_get_trigger_htstamp)
            echo "void ${sym}_impl(void){}" ;;
          *)
            echo "long ${sym}_impl(void){return -ENODEV;}" ;;
        esac
        echo "__asm__(\".symver ${sym}_impl,${sym}@@${ver}\");"
      done
    } > "$tmp/stub.c"
    {
      for v in ALSA_0.9 ALSA_0.9.0rc4 ALSA_0.9.0rc8; do echo "$v { global: *; };"; done
    } > "$tmp/stub.map"
    gcc -shared -fPIC -o "$LIB_DIR/libasound.so.2" "$tmp/stub.c" \
        -Wl,--version-script="$tmp/stub.map" -Wl,-soname,libasound.so.2
    rm -rf "$tmp"
  fi
fi

# Wrapper: añade el directorio del stub al cargador solo para zett.
cat > "$BIN_DIR/zett" <<EOF
#!/bin/sh
LD_LIBRARY_PATH="$LIB_DIR\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}" exec "$REAL" "\$@"
EOF
chmod +x "$BIN_DIR/zett"
ln -sf "$BIN_DIR/zett" "$BIN_DIR/titan"

"$BIN_DIR/zett" version
echo "OK: $BIN_DIR/zett (alias: titan)"
case ":$PATH:" in *":$BIN_DIR:"*) ;; *) echo "Añade al PATH: export PATH=\"$BIN_DIR:\$PATH\"";; esac
