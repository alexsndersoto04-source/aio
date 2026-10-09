#!/bin/bash
# Prueba de red del gestor de paquetes: `titan fetch/update/publish` contra un registro HTTPS
# local (registry.py) con una CA propia. El cliente TLS nativo solo confía en los roots
# embebidos, así que el script compila una CLI de prueba cuyo std_roots.titan contiene SOLO la
# CA de prueba. Nada de esto toca selfhost/native/: se trabaja en una copia temporal.
# Uso: selfhost/tests/pkg/registro/probar.sh      (necesita openssl, python3 y selfhost/titanc3)
# Variables: COMPILER (por defecto selfhost/titanc3), PUERTO (por defecto 47443), CACHE (directorio
#            donde se guardan la CA y la CLI de prueba para no recompilar en cada ejecución).
set -u
AQUI="$(cd "$(dirname "$0")" && pwd)"
SELFHOST="$(cd "$AQUI/../../.." && pwd)"
COMPILADOR="${COMPILER:-$SELFHOST/titanc3}"
PUERTO="${PUERTO:-47443}"
W="$(mktemp -d)"
SRV_PID=""
cleanup() { [ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null; rm -rf "$W"; }
trap cleanup EXIT
R="https://localhost:$PUERTO"
ok=0; mal=0
unset TITAN_REGISTRY_TOKEN
set -f

pasa() { ok=$((ok+1)); echo "  ok   $1"; }
falla() { mal=$((mal+1)); echo "  FALLA $1"; shift; for l in "$@"; do echo "       $l"; done; }
# esperar "descripción" CODIGO_ESPERADO "texto exacto de la primera línea" comando...
esperar() {
  local desc="$1" rc_e="$2" txt_e="$3"; shift 3
  local out rc
  out="$("$@" 2>&1)"; rc=$?
  local primera; primera="$(printf '%s\n' "$out" | head -1)"
  if [ "$rc" = "$rc_e" ] && [ "$primera" = "$txt_e" ]; then pasa "$desc"; else falla "$desc" "esperado rc=$rc_e: $txt_e" "obtenido rc=$rc: $primera"; fi
}
verdad() { # verdad "desc" comando-de-test...
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then pasa "$desc"; else falla "$desc"; fi
}

cd "$W" || exit 2
if [ -n "${CACHE:-}" ]; then mkdir -p "$CACHE"; CACHE="$(cd "$CACHE" && pwd)"; fi
if [ -n "${CACHE:-}" ] && [ -f "$CACHE/titan_t" ]; then
  echo "== reutilizando CA y CLI de prueba de $CACHE"
  cp "$CACHE"/ca.crt "$CACHE"/srv.crt "$CACHE"/srv.key .
  T="$CACHE/titan_t"
else
  echo "== CA de prueba, certificado y compilador de prueba"
  openssl ecparam -name prime256v1 -genkey -noout -out ca.key 2>/dev/null
  openssl req -x509 -new -key ca.key -sha256 -days 3650 -subj "/CN=Titan Test CA" -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign" -out ca.crt 2>/dev/null
  openssl ecparam -name prime256v1 -genkey -noout -out srv.key 2>/dev/null
  openssl req -new -key srv.key -subj "/CN=localhost" -out srv.csr 2>/dev/null
  printf 'subjectAltName=DNS:localhost\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature\nextendedKeyUsage=serverAuth\n' > ext.cnf
  openssl x509 -req -in srv.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days 3650 -sha256 -extfile ext.cnf -out srv.crt 2>/dev/null
  mkdir tc && cp "$COMPILADOR" tc/titanc && cp -r "$SELFHOST/native" tc/native
  python3 "$SELFHOST/native/gen_roots.py" ca.crt > tc/native/std_roots.titan
  # titan.titan importa sus módulos por nombre: se compila desde selfhost/ con el compilador de prueba
  (cd "$SELFHOST" && "$W/tc/titanc" titan.titan "$W/titan_t") || { echo "no compila la CLI"; exit 2; }
  T="$W/titan_t"
  if [ -n "${CACHE:-}" ]; then cp ca.crt srv.crt srv.key titan_t "$CACHE/"; T="$CACHE/titan_t"; fi
fi

echo "== registro de prueba en $R"
python3 "$AQUI/mkdata.py" "$W/data" "$PUERTO" > /dev/null
python3 "$AQUI/registry.py" "$PUERTO" srv.crt srv.key "$W/data" &
SRV_PID=$!
for i in 1 2 3 4 5 6 7 8 9 10; do (exec 3<>/dev/tcp/127.0.0.1/$PUERTO) 2>/dev/null && break; sleep 0.5; done
LOG="$W/data/requests.log"

echo "== resolución con retroceso, descarga, verificación e instalación"
mkdir app
cat > app/Titan.toml <<'TOML'
[package]
name = "app"
version = "0.1.0"
edition = "2021"
[dependencies.alpha]
version = ">=1"
[dependencies.beta]
version = "^1"
[dependencies.gamma]
version = "~0.3"
TOML
esperar "fetch resuelve y sincroniza 3 paquetes" 0 "Synchronized 3 remote packages" "$T" fetch --registry "$R" --project app
lock="$(cat app/Titan.remote.lock)"
echo "$lock" | python3 -c "
import json,sys
d=json.load(sys.stdin)
v={p['name']:p['version'] for p in d['packages']}
sys.exit(0 if v=={'alpha':'1.0.0','beta':'1.5.0','gamma':'0.3.5'} else 1)" && pasa "retroceso: alpha 1.0.0 (alpha 1.1.0 pide beta ^2), beta 1.5.0, gamma 0.3.5 (se ignora la versión inválida)" || falla "versiones del lock" "$lock"
verdad "los tres paquetes quedan instalados" test -f app/.titan/packages/alpha/1.0.0/src/lib.titan -a -f app/.titan/packages/beta/1.5.0/Titan.toml -a -f app/.titan/packages/gamma/0.3.5/src/lib.titan
verdad "y guardados en la caché" test "$(find app/.titan/cache -name '*.tpkg' | wc -l)" = 3
verdad "no se bajó la versión prerelease beta 2.1.0-rc.1" bash -c "! grep -q 2.1.0-rc.1 app/Titan.remote.lock"
descargas1="$(grep -c 'GET /archive' "$LOG")"
verdad "se pidió el índice con Accept json/octet-stream" grep -q 'GET /v1/packages/alpha accept=application/json, application/octet-stream' "$LOG"
esperar "segundo fetch sincroniza igual" 0 "Synchronized 3 remote packages" "$T" fetch --registry "$R" --project app
descargas2="$(grep -c 'GET /archive' "$LOG")"
[ "$descargas1" = 3 ] && [ "$descargas2" = 3 ] && pasa "el segundo fetch usa la caché (0 descargas nuevas)" || falla "caché" "descargas: $descargas1 / $descargas2"
rm -rf app/.titan/packages
esperar "fetch --offline reinstala desde la caché" 0 "Synchronized 3 remote packages (offline)" "$T" fetch --offline --registry "$R" --project app
verdad "--offline dejó los paquetes instalados" test -f app/.titan/packages/beta/1.5.0/src/lib.titan
esperar "update vuelve a resolver y sincroniza" 0 "Synchronized 3 remote packages" "$T" update --registry "$R" --project app

echo "== requisitos especiales"
mk() { rm -rf e; mkdir e; printf '[package]\nname="e"\nversion="0.1.0"\nedition="2021"\n[dependencies.%s]\nversion="%s"\n' "$1" "$2" > e/Titan.toml; }
mk beta "=2.1.0-rc.1"; esperar "prerelease pedido explícitamente" 0 "Synchronized 1 remote packages" "$T" fetch --registry "$R" --project e
mk beta ">=2.1.0-rc.0, <2.1.0"; esperar "rango con prerelease en el comparador" 0 "Synchronized 1 remote packages" "$T" fetch --registry "$R" --project e
mk beta "^3"; esperar "ninguna versión cumple" 1 "PACKAGE ERROR: resolution error: no version of 'beta' satisfies constraints [\"^3\"]" "$T" fetch --registry "$R" --project e
mk looper "^1"; esperar "ciclo/conflicto sin solución" 1 "PACKAGE ERROR: resolution error: no version of 'looper' satisfies constraints [\"^1\"]" "$T" fetch --registry "$R" --project e
mk redirok "*"; esperar "redirección relativa del archivo" 0 "Synchronized 1 remote packages" "$T" fetch --registry "$R" --project e

echo "== errores del registro"
mk nonexist "*";      esperar "paquete inexistente (404)" 1 "PACKAGE ERROR: resolution error: registry error: registry returned HTTP 404" "$T" fetch --registry "$R" --project e
mk wrongname "*";     esperar "índice con otro nombre" 1 "PACKAGE ERROR: resolution error: registry error: registry package name mismatch" "$T" fetch --registry "$R" --project e
mk badjson "*";       esperar "JSON inválido" 1 "PACKAGE ERROR: resolution error: registry error: invalid registry metadata: key must be a string at line 1 column 2" "$T" fetch --registry "$R" --project e
mk noversions "*";    esperar "falta 'versions'" 1 'PACKAGE ERROR: resolution error: registry error: invalid registry metadata: missing field `versions`' "$T" fetch --registry "$R" --project e
mk missingfield "*";  esperar "falta 'archive'" 1 'PACKAGE ERROR: resolution error: registry error: invalid registry metadata: missing field `archive`' "$T" fetch --registry "$R" --project e
mk httparch "*";      esperar "archivo en http://" 1 "PACKAGE ERROR: registry error: registry URL must use HTTPS" "$T" fetch --registry "$R" --project e
mk redirhttp "*";     esperar "redirección a http:// (se rechaza)" 1 "PACKAGE ERROR: registry error: registry request failed: Connection refused (os error 111)" "$T" fetch --registry "$R" --project e
mk missingarch "*";   esperar "archivo inexistente (404)" 1 "PACKAGE ERROR: registry error: registry returned HTTP 404" "$T" fetch --registry "$R" --project e
mk delta "*";         esperar "firma Ed25519 inválida" 1 "PACKAGE ERROR: registry error: package Ed25519 signature is invalid" "$T" fetch --registry "$R" --project e
mk eps "*";           esperar "dependencia inexistente" 1 "PACKAGE ERROR: resolution error: registry error: registry returned HTTP 404" "$T" fetch --registry "$R" --project e
esperar "registro con http://" 1 "PACKAGE ERROR: registry error: registry URL must use HTTPS" "$T" fetch --registry "http://localhost:$PUERTO" --project e
esperar "conexión rechazada" 1 "PACKAGE ERROR: resolution error: registry error: registry request failed: Connection refused (os error 111)" "$T" fetch --registry "https://localhost:1" --project e
mk gamma "0.3.0"; : > e/x; "$T" fetch --registry "$R" --project e >/dev/null 2>&1
# un archivo alterado en la caché se detecta
f="$(find e/.titan/cache -name '*.tpkg' | head -1)"; printf 'x' >> "$f"; rm -rf e/.titan/packages
esperar "archivo de caché alterado" 1 "PACKAGE ERROR: registry error: package SHA-256 mismatch" "$T" fetch --offline --registry "$R" --project e || true

echo "== publish"
mkdir pub; (cd pub && "$T" new pp >/dev/null && "$T" keygen key.bin >/dev/null)
export TITAN_REGISTRY_TOKEN=secret-token
echo ok > data/post_mode;       esperar "publicación aceptada (201)" 0 "Published pp 0.1.0" "$T" publish --registry "$R" --project pub/pp --key pub/key.bin
python3 - "$W/data/last_post.json" <<'PY' && pasa "el cuerpo lleva name/version/sha256/signing_key/signature/archive_base64 y el sha256 coincide" || falla "cuerpo del POST"
import json, sys, base64, hashlib
d = json.load(open(sys.argv[1]))
assert set(d) == {"name", "version", "sha256", "signing_key", "signature", "archive_base64"}, set(d)
assert d["name"] == "pp" and d["version"] == "0.1.0"
assert hashlib.sha256(base64.b64decode(d["archive_base64"])).hexdigest() == d["sha256"]
PY
verdad "POST con Authorization: Bearer y Content-Type json" grep -q 'POST /v1/packages/pp/versions auth=Bearer secret-token ctype=application/json' "$LOG"
echo denied > data/post_mode;   esperar "rechazo 403" 1 "PACKAGE ERROR: registry rejected publication with HTTP 403" "$T" publish --registry "$R" --project pub/pp --key pub/key.bin
echo conflict > data/post_mode; esperar "conflicto de propietario (409)" 1 "PACKAGE ERROR: publisher key does not match existing package ownership" "$T" publish --registry "$R" --project pub/pp --key pub/key.bin
echo ok > data/post_mode;       TITAN_REGISTRY_TOKEN=otro esperar "token incorrecto (401)" 1 "PACKAGE ERROR: registry rejected publication with HTTP 401" "$T" publish --registry "$R" --project pub/pp --key pub/key.bin
unset TITAN_REGISTRY_TOKEN
esperar "sin token" 1 "PACKAGE ERROR: TITAN_REGISTRY_TOKEN is not set" "$T" publish --registry "$R" --project pub/pp --key pub/key.bin
TITAN_REGISTRY_TOKEN="" esperar "token vacío" 1 "PACKAGE ERROR: registry token is missing or invalid" "$T" publish --registry "$R" --project pub/pp --key pub/key.bin
TITAN_REGISTRY_TOKEN=secret-token esperar "clave inexistente" 1 "PACKAGE ERROR: I/O error: No such file or directory (os error 2)" "$T" publish --registry "$R" --project pub/pp --key nokey
TITAN_REGISTRY_TOKEN=secret-token esperar "registro http://" 1 "PACKAGE ERROR: registry URL must use HTTPS" "$T" publish --registry "http://localhost:$PUERTO" --project pub/pp --key pub/key.bin
TITAN_REGISTRY_TOKEN=secret-token esperar "conexión rechazada" 1 "PACKAGE ERROR: registry request failed: Connection refused (os error 111)" "$T" publish --registry "https://localhost:1" --project pub/pp --key pub/key.bin
rm -f /tmp/titan-publish-*.tpkg

echo
echo "resultado: $ok pasan, $mal fallan"
[ "$mal" = 0 ]
