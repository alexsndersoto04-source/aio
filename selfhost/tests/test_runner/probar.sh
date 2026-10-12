#!/bin/bash
# `titan test`: qué cuenta como prueba, qué se omite y desde dónde puede importar.
# Sin dependencias: solo hace falta el compilador selfhost/titan construido.
#
#   bash selfhost/tests/test_runner/probar.sh
#
# 1. Un módulo auxiliar bajo tests/ (sin `fn main`) se OMITE, no falla.
# 2. Una prueba importa a la vez un módulo de src/ y un auxiliar hermano de tests/.
# 3. Una prueba que falla cuenta como fallo y hace que `titan test` salga con código 1.
# 4. Cuando todo pasa, el resumen no menciona omitidos y el código de salida es 0.
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
TITAN="${1:-${TITAN:-selfhost/titan}}"

if [ ! -x "$TITAN" ]; then
  echo "probar.sh: no se encuentra $TITAN (ejecuta antes: bash selfhost/bootstrap.sh)" >&2
  exit 2
fi

fail=0
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# --- proyecto de prueba: un módulo en src/, un auxiliar y pruebas en tests/ ---
proyecto() { # $1 = carpeta, $2 = "con_fallo" | "sin_fallo"
  local d="$1" modo="$2"
  mkdir -p "$d/src" "$d/tests"
  cat > "$d/Titan.toml" <<'EOF'
[package]
name = "prueba"
version = "0.1.0"
edition = "2026"
EOF
  cat > "$d/src/main.titan" <<'EOF'
fn main() { print("aplicación") }
EOF
  cat > "$d/src/util.titan" <<'EOF'
fn triple(n: int) -> int { n * 3 }
EOF
  # Auxiliar: solo declaraciones, sin punto de entrada.
  cat > "$d/tests/auxiliar.titan" <<'EOF'
fn doble(n: int) -> int { n * 2 }
EOF
  # La prueba importa de las dos raíces: src/ (util) y tests/ (auxiliar).
  cat > "$d/tests/prueba.titan" <<'EOF'
import util
import auxiliar

fn main() {
    std::testing::assert_eq(triple(7), 21, "triple viene de src/util.titan")
    std::testing::assert_eq(doble(7), 14, "doble viene de tests/auxiliar.titan")
    print("importa de src/ y de tests/")
}
EOF
  if [ "$modo" = "con_fallo" ]; then
    cat > "$d/tests/falla.titan" <<'EOF'
fn main() {
    std::testing::assert_eq(1, 2, "esto tiene que fallar")
}
EOF
  fi
}

echo "== 1. todo pasa: auxiliar omitido, imports de las dos raíces =="
proyecto "$tmp/bien" "sin_fallo"
out=$("$TITAN" test "$tmp/bien" 2>&1); rc=$?
printf '%s\n' "$out" | grep -q "test tests/prueba.titan ... ok" || {
  echo "FALLO: la prueba que importa de src/ y de tests/ no pasó" >&2
  printf '%s\n' "$out" | head -12 >&2
  fail=1
}
printf '%s\n' "$out" | grep -q "test tests/auxiliar.titan ... skipped (no entry point 'main'" || {
  echo "FALLO: el módulo auxiliar no se omitió (antes contaba como fallo)" >&2
  printf '%s\n' "$out" | head -12 >&2
  fail=1
}
printf '%s\n' "$out" | grep -q "^test result: ok\. 1 passed; 0 failed; 1 skipped$" || {
  echo "FALLO: el resumen no cuadra" >&2
  printf '%s\n' "$out" | tail -3 >&2
  fail=1
}
[ "$rc" -eq 0 ] || { echo "FALLO: el código de salida fue $rc, debía ser 0" >&2; fail=1; }

echo "== 2. una prueba que falla: se reporta y sale con código 1 =="
proyecto "$tmp/mal" "con_fallo"
out=$("$TITAN" test "$tmp/mal" 2>&1); rc=$?
printf '%s\n' "$out" | grep -q "test tests/falla.titan ... FAILED" || {
  echo "FALLO: la prueba fallida no se reportó como FAILED" >&2
  printf '%s\n' "$out" | head -12 >&2
  fail=1
}
printf '%s\n' "$out" | grep -q "assertion failed: esto tiene que fallar" || {
  echo "FALLO: no se imprimió el motivo de la aserción" >&2
  printf '%s\n' "$out" | head -12 >&2
  fail=1
}
printf '%s\n' "$out" | grep -q "^test result: FAILED\. 1 passed; 1 failed; 1 skipped$" || {
  echo "FALLO: el resumen con fallo no cuadra" >&2
  printf '%s\n' "$out" | tail -3 >&2
  fail=1
}
[ "$rc" -eq 1 ] || { echo "FALLO: el código de salida fue $rc, debía ser 1" >&2; fail=1; }

echo "== 3. proyecto recién creado (sin tests/): check y run no se caen =="
# El cargador canonicalizaba <raíz>/tests sin mirar si existía, y `titan new` no crea
# tests/: `titan check`/`run`/`dap` con un proyecto recién hecho morían con «native
# function 'std::path::canonical' failed: No such file or directory» (lo delataba la
# prueba del DAP). Aquí se comprueba lo mínimo: que arranca y hace lo que toca.
"$TITAN" new recien --path "$tmp" > /dev/null 2>&1 || { echo "FALLO: titan new" >&2; fail=1; }
[ -f "$tmp/recien/src/main.titan" ] || { echo "FALLO: titan new no creó src/main.titan" >&2; fail=1; }
if ! out=$("$TITAN" check "$tmp/recien" 2>&1); then
  echo "FALLO: check de un proyecto recién creado: $out" >&2
  fail=1
fi
if ! out=$("$TITAN" run "$tmp/recien" 2>&1); then
  echo "FALLO: run de un proyecto recién creado: $out" >&2
  fail=1
fi
printf '%s\n' "$out" | grep -q "Hello from Titan!" || {
  echo "FALLO: run de un proyecto recién creado no imprimió el saludo: $out" >&2
  fail=1
}
# Y con un proyecto sin tests/ `titan test` avisa en claro (nada de reventar con
# «native function 'std::path::canonical' failed»): mensaje y código 1, como un
# error de uso, no un fallo del runtime.
out=$("$TITAN" test "$tmp/recien" 2>&1); rc=$?
printf '%s\n' "$out" | grep -q "TEST ERROR: no .titan tests found under" || {
  echo "FALLO: titan test de un proyecto sin tests/ no avisó en claro: $out" >&2
  fail=1
}
printf '%s\n' "$out" | grep -qi "canonical\|RUNTIME ERROR" && { echo "FALLO: titan test se cayó: $out" >&2; fail=1; }
[ "$rc" -eq 1 ] || { echo "FALLO: titan test de un proyecto sin tests/ salió con $rc, debía ser 1" >&2; printf '%s\n' "$out" | head -5 >&2; fail=1; }

# `--path DIR`: el proyecto tiene que quedar en DIR/<nombre> (antes la opción se
# aceptaba y se ignoraba: el proyecto aparecía en el directorio actual).
mkdir -p "$tmp/destino"
out=$("$TITAN" new app --path "$tmp/destino" 2>&1) || { echo "FALLO: titan new --path: $out" >&2; fail=1; }
[ -f "$tmp/destino/app/src/main.titan" ] || { echo "FALLO: --path no creó el proyecto en DIR/<nombre>: $out" >&2; fail=1; }
[ -d "$ROOT/app" ] && { echo "FALLO: --path ignorado: creó el proyecto en el directorio actual" >&2; fail=1; }

if [ "$fail" -eq 0 ]; then
  echo "probar.sh (test_runner): todo bien"
fi
exit "$fail"
