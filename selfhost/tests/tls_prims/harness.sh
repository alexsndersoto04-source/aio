#!/bin/bash
# Banco de pruebas de primitivas del runtime (X25519, ECDSA, X.509...). Uso:
#   bash selfhost/tests/tls_prims/harness.sh PRUEBA.titan CMDS.titan [módulos...]
# Crea /tmp/sc con una copia del compilador y del runtime en la que
# `std::hash::blake3_bytes` llama a `tst_hook` (tst.titan + CMDS.titan).
set -e
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TEST="$1"; CMDS="$2"; shift 2
SC=/tmp/sc
COMP="${COMPILER:-$ROOT/selfhost/titanc7}"
rm -rf "$SC/native"; mkdir -p "$SC"
cp -r "$ROOT/selfhost/native" "$SC/native"
cp "$COMP" "$SC/titanc"
cp "$ROOT/selfhost/tests/tls_prims/tst.titan" "$SC/native/tst.titan"
cp "$CMDS" "$SC/native/tst_cmds.titan"
python3 - "$SC/native" "$@" <<'PY'
import sys,re
d=sys.argv[1]; mods=sys.argv[2:]
h=open(d+'/std_hash.titan').read()
h=h.replace('fn std__hash__blake3_bytes(v: any) -> any { hs_run(v, 6, true, "std::hash::blake3_bytes") }','fn std__hash__blake3_bytes(v: any) -> any { tst_hook(v) }')
open(d+'/std_hash.titan','w').write(h)
r=open(d+'/runtime.titan').read()
imp=''.join('import %s\n'%m for m in mods)+'import tst\nimport tst_cmds\n'
r=r.replace('import std_http_router\n','import std_http_router\n'+imp,1)
open(d+'/runtime.titan','w').write(r)
PY
cd "$ROOT"
unset GITHUB_ACTIONS
rm -f "$SC/prueba"; "$SC/titanc" "$TEST" "$SC/prueba" | tail -5; [ -x "$SC/prueba" ] || exit 1
"$SC/prueba"
