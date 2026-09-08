#!/usr/bin/env bash
# Espejo del binario oficial de zett (linux x86_64) desde la release v1.0.0
# a la rama ZETT_MIRROR_TOOLS_BRANCH (default: tools-zett-x86_64).
#
# Corre como PASO DE WORKFLOW (solo ubuntu-latest) porque:
#   - tiene GITHUB_TOKEN con contents:write de forma garantizada
#   - sus anotaciones ::warning::/::error:: SI se publican (el output de un
#     build script de cargo no siempre llega como anotacion)
#
# Canales de publicacion (en orden):
#   1. git push directo (temp repo + token embebido)
#   2. GitHub API contents (PUT base64, sin git)
# Diagnostico extra en tools-zett-diag:mirror.txt (API) en todos los casos.
set -u

REPO="alexsndersoto04-source/aio"
RELEASE="v1.0.0"
ASSET="zett-linux-x86_64.tar.gz"
BRANCH="${ZETT_MIRROR_TOOLS_BRANCH:-tools-zett-x86_64}"
DIAG_BRANCH="tools-zett-diag"
TOKEN="${GITHUB_TOKEN:-}"

say() { echo "::warning::[zett-mirror] $*"; }
err() { echo "::error::[zett-mirror] $*"; }

# Publica texto de diagnostico en $DIAG_BRANCH:mirror.txt via API (la rama
# ya existe; el PUT crea el commit). No bloquea: solo informacion.
diag() {
  local txt="$1" b64 put
  b64=$(printf '%s' "$txt" | base64 -w0)
  put=$(curl -s -X PUT \
    -H "Authorization: Bearer $TOKEN" \
    -H "Accept: application/vnd.github+json" \
    -H "Content-Type: application/json" \
    -d "{\"content\":\"$b64\",\"message\":\"diag: zett-mirror (workflow step)\"}" \
    "https://api.github.com/repos/$REPO/contents/mirror.txt?branch=$DIAG_BRANCH" \
    -w '\nHTTP:%{http_code}' | tail -1)
  say "diag_api=$put"
}

say "inicio branch=$BRANCH release=$RELEASE token_len=${#TOKEN} ci=${CI:-no} runner=${RUNNER_OS:-?}"

if [ -z "$TOKEN" ]; then
  err "GITHUB_TOKEN vacio en el paso del mirror"
  exit 0
fi

DIR=$(mktemp -d)
trap 'rm -rf "$DIR"' EXIT
cd "$DIR" || exit 0

URL="https://github.com/$REPO/releases/download/$RELEASE/$ASSET"
if ! curl -fsSL --retry 3 --connect-timeout 30 "$URL" -o z.tar.gz; then
  err "curl fallo al descargar $URL"
  diag "FASE=curl-descarga (release $RELEASE)"
  exit 0
fi
SZ=$(wc -c < z.tar.gz)
say "descargado $SZ bytes"
if [ "$SZ" -lt 1000000 ]; then
  err "descarga sospechosamente pequena ($SZ bytes) — probable pagina de error"
  diag "FASE=tamano-anomalo size=$SZ"
  exit 0
fi

if ! tar -xzf z.tar.gz; then
  err "tar fallo al extraer z.tar.gz"
  diag "FASE=tar"
  exit 0
fi
chmod +x zett || true
if ! VER=$(./zett --version 2>&1 | head -1); then
  err "el binario extraido no ejecuta --version"
  diag "FASE=exec-version"
  exit 0
fi
say "binario verificado: $VER"

# ---- Canal 1: git push directo ----
git init -q repo && cd repo || { err "git init fallo"; diag "FASE=git-init"; exit 0; }
mkdir -p tools
cp ../zett tools/zett-linux-x86_64
git add -A
git -c user.name=ci-mirror -c user.email=ci@local \
  commit -q -m "tools: zett binario oficial $RELEASE (linux x86_64, $(date -u +%Y-%m-%dT%H:%M:%SZ))"
GITPUSH=$(git push -f "https://x-access-token:$TOKEN@github.com/$REPO.git" "HEAD:refs/heads/$BRANCH" 2>&1 | tail -3)
if git push -f -q "https://x-access-token:$TOKEN@github.com/$REPO.git" "HEAD:refs/heads/$BRANCH" 2>/dev/null; then
  say "CANAL-GIT OK: rama=$BRANCH actualizada con el binario real"
  diag "FASE=ok-canal-git size=$SZ version=$VER"
  exit 0
fi
say "canal-git fallo: $(echo "$GITPUSH" | tr '\n' ' ' | head -c 300)"

# ---- Canal 2: GitHub API contents (base64) ----
B64=$(base64 -w0 ../zett)
curl -s -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -d '{"source":"main"}' \
  "https://api.github.com/repos/$REPO/branches/$BRANCH" > /dev/null
PUT=$(curl -s -X PUT \
  -H "Authorization: Bearer $TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "Content-Type: application/json" \
  -d "{\"content\":\"$B64\",\"message\":\"tools: zett binario oficial $RELEASE (linux x86_64, API)\"}" \
  "https://api.github.com/repos/$REPO/contents/tools/zett-linux-x86_64?branch=$BRANCH" \
  -w '\nHTTP:%{http_code}' | tail -1)
if [ "$PUT" = "HTTP:200" ] || [ "$PUT" = "HTTP:201" ]; then
  say "CANAL-API OK: rama=$BRANCH actualizada con el binario real"
  diag "FASE=ok-canal-api size=$SZ version=$VER"
  exit 0
fi
err "todos los canales fallaron: git=(ver arriba) api=$PUT"
diag "FASE=fallo-todos-canales git_tail=$(echo "$GITPUSH" | tr '\n' ' ' | head -c 200) api=$PUT"
exit 0
