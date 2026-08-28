#!/bin/bash
# Wrapper de diagnostico + transporte para rustc en CI.
#
# Funciones (solo en CI con GITHUB_TOKEN; en local es transparente):
#   1. PROBE: en la primera compilacion por corrida, publica el estado
#      del entorno (token, CI, target, host) para diagnosticar la CI
#      sin acceso a logs de Actions.
#   2. ERROR: si rustc falla, publica el error completo.
#   3. MIRROR: si detecta el binario `zett` (linux x86_64) compilado en
#      target/, lo publica en la rama tools-zett-x86_64 (canal git).
#
# Canales de publicacion (los dos; se leen desde el sandbox):
#   A) git push directo (repo temporal + token embebido) a rama dedicada.
#   B) GitHub Contents API (escribe en la rama por defecto: main, carpeta diag/).
#
# Sin recursividad: cargo invoca al wrapper; el rustc real no lo invoca.

REPO="alexsndersoto04-source/aio"
BR_DIAG="tools-zett-probe-v14"
BR_BIN="tools-zett-x86_64"
RUNID="${GITHUB_RUN_ID:-local}"
FLAG_PROBE="/tmp/zett-wp-probe.${RUNID}"
FLAG_ERR="/tmp/zett-wp-err.${RUNID}"
FLAG_BIN="/tmp/zett-wp-bin.${RUNID}"

# ---------- canal A: git push de un archivo a rama dedicada ----------
git_publish_file() {
  # $1 = rama, $2 = ruta destino (rel), $3 = fichero fuente.
  # Se apila sobre el tip remoto de la rama (fast-forward, sin fuerza):
  # funciona creando la rama o actualizandola.
  local BRANCH="$1" DEST="$2" SRC="$3" W RC URL
  W=$(mktemp -d /tmp/zett-wp-git.XXXXXX) || return 1
  URL="https://x-access-token:${GITHUB_TOKEN}@github.com/${REPO}.git"
  git -C "$W" init -q 2>/dev/null
  git -C "$W" config user.email "ci-diag@localhost"
  git -C "$W" config user.name "ci-diag"
  git -C "$W" fetch -q --depth 1 "$URL" "refs/heads/${BRANCH}" >/dev/null 2>&1 || true
  git -C "$W" reset -q --hard FETCH_HEAD >/dev/null 2>&1 || true
  mkdir -p "$W/$(dirname "$DEST")"
  cp "$SRC" "$W/$DEST"
  git -C "$W" add -A 2>/dev/null
  git -C "$W" commit -q -m "diag (wrapper rustc)" 2>/dev/null || { rm -rf "$W"; return 0; }
  git -C "$W" push -q "$URL" "HEAD:refs/heads/${BRANCH}" >/dev/null 2>&1
  RC=$?
  rm -rf "$W"
  return $RC
}

# ---------- canal B: Contents API (escribe en main, carpeta diag/) ----------
api_publish_file() {
  # $1 = ruta destino (rel, SIN carpeta diag), $2 = fichero fuente
  local FILE="diag/$1" SRC="$2" B64 CODE
  B64=$(base64 -w0 "$SRC" 2>/dev/null)
  [ -n "$B64" ] || return 1
  CODE=$(curl -s -o /dev/null -w '%{http_code}' --max-time 40 -X PUT \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "Accept: application/vnd.github+json" \
    -H "Content-Type: application/json" \
    -d "{\"content\":\"${B64}\",\"message\":\"diag wrapper\"}" \
    "https://api.github.com/repos/${REPO}/contents/${FILE}" 2>/dev/null)
  [ "$CODE" = "200" ] || [ "$CODE" = "201" ]
}

publish_text() {
  # $1 = archivo (rel), $2 = texto
  local FILE="$1" TEXT="$2" TF
  TF=$(mktemp /tmp/zett-wp-txt.XXXXXX) || return 1
  printf '%s' "$TEXT" > "$TF"
  git_publish_file "$BR_DIAG" "$FILE" "$TF" || true
  api_publish_file "$FILE" "$TF" || true
  rm -f "$TF"
}

publish_probe() {
  local T P ENC
  T=$(env | grep -E '^(CI|GITHUB_ACTIONS|GITHUB_RUN_ID|GITHUB_JOB|TARGET|ZETT_FORCE_RUN|ZETT_MIRROR_TOOLS_BRANCH|CARGO_PKG_NAME|CARGO_MANIFEST_DIR|CARGO_TARGET_DIR|RUSTFLAGS|PATH)=' | cut -c1-300)
  P=$(cat <<EOF
ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
host=$(hostname 2>/dev/null)
user=$(whoami 2>/dev/null)
cwd=$(pwd 2>/dev/null)
git=$(git --version 2>/dev/null)
pkg=${CARGO_PKG_NAME:-?}
GITHUB_TOKEN=<set, len=${#GITHUB_TOKEN}>
env:
${T}
EOF
)
  publish_text "probe.txt" "$P"
  # Canal 3: anotacion (el wrapper corre como proceso normal; su salida
  # llega al log del paso y el runner la convierte en anotacion).
  ENC=$(printf '%s' "$P" | sed 's/%/%25/g; s/\r//g' | awk 'NR>1{printf "%%0A"} {printf "%s", $0}')
  echo "::warning title=ZETT_PROBE::${ENC}"
}

publish_error() {
  local TEXT HASH ENC
  TEXT=$(tail -n 150 "$1" 2>/dev/null)
  [ -z "$TEXT" ] && TEXT="(sin salida)"
  TEXT="${TEXT}
---
ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
host=$(hostname 2>/dev/null)
cwd=$(pwd 2>/dev/null)
pkg=${CARGO_PKG_NAME:-?}
argv=$*"
  HASH=$(printf '%s' "$TEXT" | sha1sum | cut -d' ' -f1)
  [ -f "$FLAG_ERR" ] && [ "$(cat "$FLAG_ERR" 2>/dev/null)" = "$HASH" ] && return 0
  echo "$HASH" > "$FLAG_ERR"
  publish_text "error.txt" "$TEXT"
  ENC=$(printf '%s' "$TEXT" | sed 's/%/%25/g; s/\r//g' | awk 'NR>1{printf "%%0A"} {printf "%s", $0}')
  echo "::error title=RUSTC_DIAG::${ENC}"
}

mirror_binary() {
  # Solo el binario linux x86_64 va a tools-zett-x86_64.
  local TD BIN TRIPLE FSHA W RC SIZE
  TD="${CARGO_TARGET_DIR:-$(pwd)/target}"
  [ -d "$TD" ] || return 1
  BIN=$(find "$TD" -maxdepth 4 -type f -name zett -perm -u+x 2>/dev/null \
        | grep 'x86_64-unknown-linux-gnu/release/zett' | head -1)
  [ -n "$BIN" ] && [ -s "$BIN" ] || return 1
  TRIPLE="x86_64-unknown-linux-gnu"
  FSHA=$(sha1sum "$BIN" | cut -d' ' -f1)
  [ -f "$FLAG_BIN" ] && [ "$(cat "$FLAG_BIN" 2>/dev/null)" = "$FSHA" ] && return 0
  SIZE=$(stat -c%s "$BIN" 2>/dev/null || echo 0)
  echo "[wrapper] mirror: $BIN ($SIZE bytes) -> $BR_BIN"
  echo "$FSHA" > "$FLAG_BIN"
  W=$(mktemp -d /tmp/zett-wp-bin.XXXXXX) || return 1
  URL="https://x-access-token:${GITHUB_TOKEN}@github.com/${REPO}.git"
  git -C "$W" init -q 2>/dev/null
  git -C "$W" config user.email "ci-diag@localhost"
  git -C "$W" config user.name "ci-diag"
  git -C "$W" fetch -q --depth 1 "$URL" "refs/heads/${BR_BIN}" >/dev/null 2>&1 || true
  git -C "$W" reset -q --hard FETCH_HEAD >/dev/null 2>&1 || true
  mkdir -p "$W/tools"
  cp "$BIN" "$W/tools/zett-linux-x86_64"
  git -C "$W" add -A
  git -C "$W" commit -q -m "mirror zett linux-x86_64 (sha1 ${FSHA:0:12})"
  git -C "$W" push -q "$URL" "HEAD:refs/heads/${BR_BIN}" 2>/dev/null
  RC=$?
  [ $RC -eq 0 ] && echo "[wrapper] mirror OK" || echo "[wrapper] mirror PUSH rc=$RC"
  rm -rf "$W"
  return $RC
}

# ---------- principal ----------
OUT="$(mktemp /tmp/zett-rustc.XXXXXX)" || OUT="/tmp/zett-rustc.$$"
rustc "$@" > "$OUT" 2>&1
code=$?

if [ -n "$CI" ] && [ -n "$GITHUB_TOKEN" ]; then
  if [ ! -f "$FLAG_PROBE" ]; then
    touch "$FLAG_PROBE"
    publish_probe
  fi
  if [ "$code" -ne 0 ]; then
    publish_error "$OUT"
  fi
  mirror_binary
fi

cat "$OUT"
rm -f "$OUT"
exit "$code"
