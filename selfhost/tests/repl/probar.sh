#!/bin/bash
# Compara `titan repl` con `zett repl` (salida, errores y código de salida) con varias sesiones.
# Uso: bash probar.sh [titan] [zett]
TITAN=${1:-$(dirname "$0")/../../titan}
ZETT=${2:-zett}
d=$(mktemp -d)
n=0; mal=0
sesion() {
  n=$((n+1))
  printf '%b' "$1" > "$d/in"
  "$TITAN" repl < "$d/in" > "$d/ot" 2> "$d/et"; ct=$?
  "$ZETT" repl < "$d/in" > "$d/oz" 2> "$d/ez"; cz=$?
  if cmp -s "$d/ot" "$d/oz" && cmp -s "$d/et" "$d/ez" && [ $ct = $cz ]; then
    echo "ok   sesión $n"
  else
    mal=$((mal+1)); echo "FALLA sesión $n: $(printf '%b' "$1" | head -c 80)"
    diff "$d/ot" "$d/oz" | head -5; diff "$d/et" "$d/ez" | head -5; echo "códigos: $ct vs $cz"
  fi
}
sesion '1+2\n:q\n'
sesion '1+2\nlet x = 5\nx\nstd::text::length("hey")\n1 +\nfoo()\n1 / 0\nprint("hi")\n:v\n:z\n:h\n'
sesion '\n   \n:quit\n1\n'
sesion 'std::process::exit(3)\n1+1\n'
sesion '"a" + 1\nlet s = "é"\n[1,2,3]\n"x\n'
sesion '"hola " + "mundo"\n3.5 * 2.0\ntrue && false\nnil\n'
sesion 'let a = [1,2,3]\nstd::array::len(a)\n'
sesion '1 + true\nfn f() {}\nlet x: int = "s"\n'
sesion '1+1'
sesion ':help\n:h\n:foo bar\n'
sesion 'print("a")\nprint("b")\n42\n'
echo "sesiones: $n, fallan: $mal"
rm -rf "$d"
[ $mal = 0 ]
