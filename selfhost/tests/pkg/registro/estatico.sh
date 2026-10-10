#!/bin/bash
# Prueba del registro ESTÁTICO (el formato que GitHub sirve desde `registro/`):
#   1. publica dos paquetes con selfhost/registro_agregar.titan (firma ed25519 real),
#   2. los sirve por HTTPS como ficheros,
#   3. `titan fetch` los resuelve, descarga, verifica la firma y los instala.
# Igual que probar.sh, compila una CLI cuyo std_roots.titan solo confía en la CA de prueba.
# Uso: selfhost/tests/pkg/registro/estatico.sh   (necesita openssl, python3 y selfhost/titanc3)
# Variables: TITAN (CLI normal, por defecto selfhost/titan), COMPILER, PUERTO (47444), CACHE (reutiliza CA y CLI; compartible con probar.sh).
set -u
AQUI="$(cd "$(dirname "$0")" && pwd)"
SELFHOST="$(cd "$AQUI/../../.." && pwd)"
COMPILADOR="${COMPILER:-$SELFHOST/titanc3}"
PUERTO="${PUERTO:-47444}"
W="$(mktemp -d)"
SRV_PID=""
cleanup() { [ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null; rm -rf "$W"; }
trap cleanup EXIT
R="https://localhost:$PUERTO"
ok=0; mal=0
unset TITAN_REGISTRY_TOKEN
pasa() { ok=$((ok+1)); echo "  ok   $1"; }
falla() { mal=$((mal+1)); echo "  FALLA $1"; shift; for l in "$@"; do echo "       $l"; done; }
verdad() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then pasa "$d"; else falla "$d"; fi; }
esperar() { # desc rc texto-primera-línea comando...
  local desc="$1" rc_e="$2" txt_e="$3"; shift 3
  local out rc; out="$("$@" 2>&1)"; rc=$?
  local p; p="$(printf '%s\n' "$out" | head -1)"
  if [ "$rc" = "$rc_e" ] && [ "$p" = "$txt_e" ]; then pasa "$desc"; else falla "$desc" "esperado rc=$rc_e: $txt_e" "obtenido rc=$rc: $p"; fi
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
  (cd "$SELFHOST" && "$W/tc/titanc" titan.titan "$W/titan_t") || { echo "no compila la CLI"; exit 2; }
  T="$W/titan_t"
  if [ -n "${CACHE:-}" ]; then cp ca.crt srv.crt srv.key titan_t "$CACHE/"; T="$CACHE/titan_t"; fi
fi

echo "== publicar en un registro estático"
mkpkg() { # directorio nombre versión [dependencia versión]
  mkdir -p "$1/src"
  printf '[package]\nname = "%s"\nversion = "%s"\nedition = "2026"\n' "$2" "$3" > "$1/Titan.toml"
  [ $# -ge 5 ] && printf '[dependencies.%s]\nversion = "%s"\n' "$4" "$5" >> "$1/Titan.toml"
  printf 'fn nombre() -> string {\n    "%s %s"\n}\n' "$2" "$3" > "$1/src/lib.titan"
}
mkpkg base base-zett 1.0.0
mkpkg base2 base-zett 1.1.0
mkpkg util util-zett 0.2.0 base-zett "^1"
"$T" keygen clave >/dev/null; "$T" keygen otra >/dev/null
# publicar usa la CLI normal (necesita native/ junto a ella); fetch usa la CLI de prueba con la CA de prueba
TITAN_NORMAL="${TITAN:-$SELFHOST/titan}"
PUB() { (cd "$SELFHOST" && "$TITAN_NORMAL" run registro_agregar.titan "$@"); }
esperar "publica base-zett 1.0.0" 0 "Publicado base-zett 1.0.0 en $W/reg" PUB "$W/base" "$W/clave" "$W/reg"
esperar "publica base-zett 1.1.0 (misma clave)" 0 "Publicado base-zett 1.1.0 en $W/reg" PUB "$W/base2" "$W/clave" "$W/reg"
esperar "publica util-zett 0.2.0 con dependencia" 0 "Publicado util-zett 0.2.0 en $W/reg" PUB "$W/util" "$W/clave" "$W/reg"
esperar "no deja reescribir una versión" 1 "error: base-zett 1.0.0 ya está publicado (las versiones no se reescriben)" PUB "$W/base" "$W/clave" "$W/reg"
esperar "no deja publicar con otra clave" 1 "error: el paquete base-zett ya existe con otra clave de firma" PUB "$W/base" "$W/otra" "$W/reg"
verdad "el índice lleva la dependencia" grep -q '"base-zett": "^1"' "$W/reg/v1/packages/util-zett"
verdad "el índice usa ruta relativa" grep -q '"archive": "archivos/base-zett-1.0.0.tpkg"' "$W/reg/v1/packages/base-zett"

echo "== servir por HTTPS y usar con fetch"
python3 "$AQUI/estatico.py" "$PUERTO" srv.crt srv.key "$W/reg" &
SRV_PID=$!
for i in 1 2 3 4 5 6 7 8 9 10; do (exec 3<>/dev/tcp/127.0.0.1/$PUERTO) 2>/dev/null && break; sleep 0.5; done
mkdir app
printf '[package]\nname = "app"\nversion = "0.1.0"\nedition = "2026"\n[dependencies.util-zett]\nversion = "^0.2"\n' > app/Titan.toml
esperar "fetch resuelve util-zett + base-zett" 0 "Synchronized 2 remote packages" "$T" fetch --registry "$R" --project app
verdad "base-zett 1.1.0 (la más nueva ^1) instalado" test -f app/.titan/packages/base-zett/1.1.0/src/lib.titan
verdad "util-zett 0.2.0 instalado" test -f app/.titan/packages/util-zett/0.2.0/src/lib.titan
verdad "el lock apunta al archivo del registro" grep -q "$R/archivos/base-zett-1.1.0.tpkg" app/Titan.remote.lock
# un archivo alterado en el servidor debe rechazarse (hash y firma)
printf 'x' >> "$W/reg/archivos/util-zett-0.2.0.tpkg"
rm -rf app/.titan
esperar "archivo alterado → rechazado" 1 "PACKAGE ERROR: registry error: package SHA-256 mismatch" "$T" fetch --registry "$R" --project app

echo
echo "resultado: $ok bien, $mal mal"
[ "$mal" = 0 ]
