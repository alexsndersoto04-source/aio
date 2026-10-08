#!/bin/bash
# Verificación de std::term en situaciones que la batería general no puede
# reproducir: una terminal de verdad (pseudoterminal) y distintos entornos.
# Compara byte a byte la VM de Rust (`zett run`) con el ejecutable nativo.
# Uso (desde la raíz del repositorio): bash selfhost/native/term/verificar.sh
# ZETT selecciona el seed precompilado; COMPILER (opcional) usa titanc1 del bootstrap.
set -u
ZETT="${ZETT:-$(command -v zett)}"
COMPILER="${COMPILER:-}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
D="$ROOT/selfhost/native/term"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
ok=0; bad=0
check() {  # nombre, archivo VM, archivo nativo
  if cmp -s "$2" "$3"; then ok=$((ok + 1)); echo "igual: $1"
  else bad=$((bad + 1)); echo "DIFERENCIA: $1"; diff "$2" "$3" | head -6; fi
}
cd "$ROOT/selfhost"
for p in tamano teclas teclas_plazos; do
  if [ -n "$COMPILER" ]; then
    "$COMPILER" "$D/$p.titan" "$tmp/$p" > "$tmp/build.txt" 2>&1 || { cat "$tmp/build.txt"; exit 1; }
  else
    "$ZETT" run build.titan "$D/$p.titan" "$tmp/$p" > "$tmp/build.txt" 2>&1 || { cat "$tmp/build.txt"; exit 1; }
  fi
done
# terminal::size() sin terminal: tput según TERM, COLUMNS/LINES y PATH.
# (Sin TERM el errno depende de una carrera dentro del propio Rust: queda fuera.)
n=0
for envs in "TERM=xterm" "TERM=xterm COLUMNS=120 LINES=40" "TERM=dumb" "PATH=/nonexistent" \
            "PATH=/root:/nonexist" "PATH=" "PATH=/root:/usr/bin TERM=vt100"; do
  n=$((n + 1))
  env $envs "$ZETT" run "$D/tamano.titan" < /dev/null > "$tmp/v$n" 2>&1
  env $envs "$tmp/tamano" < /dev/null > "$tmp/n$n" 2>&1
  check "size con $envs" "$tmp/v$n" "$tmp/n$n"
done
# Dentro de una pseudoterminal de 33 x 77
TERM=xterm python3 "$D/../progress/pty_run.py" 33 77 "$ZETT" run "$D/tamano.titan" > "$tmp/vp"
TERM=xterm python3 "$D/../progress/pty_run.py" 33 77 "$tmp/tamano" > "$tmp/np"
check "size en una terminal" "$tmp/vp" "$tmp/np"
# Teclas en modo crudo (y la configuración de la terminal antes y después)
S="t $(cat "$D/teclas_pasos.txt") s:0.5 t"
TERM=xterm python3 "$D/pty_keys.py" 24 80 "$S" "$ZETT" run "$D/teclas.titan" > "$tmp/vk"
TERM=xterm python3 "$D/pty_keys.py" 24 80 "$S" "$tmp/teclas" > "$tmp/nk"
check "read_key en modo crudo" "$tmp/vk" "$tmp/nk"
# Plazos, SIGWINCH y modo normal
S="s:0.8 r:40:100 s:0.5 k:61620a s:0.5 k:0a s:0.3 t"
TERM=xterm python3 "$D/pty_keys.py" 24 80 "$S" "$ZETT" run "$D/teclas_plazos.titan" > "$tmp/vt"
TERM=xterm python3 "$D/pty_keys.py" 24 80 "$S" "$tmp/teclas_plazos" > "$tmp/nt"
check "plazos, cambio de tamaño y modo normal" "$tmp/vt" "$tmp/nt"
# NO_COLOR (memorizado la primera vez que se escribe un color)
NO_COLOR=1 "$ZETT" run "$ROOT/selfhost/tests/native/term_std.titan" > "$tmp/vc" 2>&1
"$ZETT" run build.titan "$ROOT/selfhost/tests/native/term_std.titan" "$tmp/term_std" > /dev/null 2>&1
NO_COLOR=1 "$tmp/term_std" > "$tmp/nc" 2>&1
check "NO_COLOR" "$tmp/vc" "$tmp/nc"
echo "pruebas: $((ok + bad))  iguales: $ok  distintas: $bad"
[ "$bad" -eq 0 ]
