#!/bin/bash
# Verificación diferencial del backend LLVM.
#
# Para cada programa: selfhost/build_llvm.titan (todo en Titan) escribe su
# LLVM IR; LLVM de verdad lo compila y optimiza (clang del sistema; si no,
# python3 -m ziglang cc; como último recurso, llvmlite + ld, ver llc.py); se
# ejecuta y se compara salida estándar,
# salida de errores y código de salida con `zett run` (la VM de Rust).
# Lo que el backend no admite se cuenta aparte ("no admitido"), nunca como
# acierto.
#
# Uso: bash selfhost/native/llvm/verificar.sh [archivos...]
#      (sin archivos: selfhost/tests/native/*.titan)
# Variables: OPT (por defecto -O2), ZETT, TIMEOUT (segundos por programa; 60).
set -u
ZETT="${ZETT:-zett}"
OPT="${OPT:--O2}"
TIMEOUT="${TIMEOUT:-60}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if [ $# -gt 0 ]; then FILES=("$@"); else FILES=(selfhost/tests/native/*.titan); fi

# El compilador LLVM en Titan, compilado a ejecutable por el compilador
# nativo en Titan (así no hay que pasar por la VM para cada programa).
if [ ! -x selfhost/build_llvm ] || [ -n "$(find selfhost -name '*.titan' -newer selfhost/build_llvm | head -1)" ]; then
  echo "compilando selfhost/build_llvm..."
  (cd selfhost && "$ZETT" run build.titan build_llvm.titan build_llvm) || exit 1
fi

if command -v clang >/dev/null 2>&1; then
  LLVM="clang $(clang --version | head -1)"
  compile() { clang $OPT -nostdlib -static -fno-pie -no-pie -Wl,-e,_start "$1" -o "$2"; }
elif python3 -m ziglang cc --version >/dev/null 2>&1; then
  LLVM="ziglang cc ($(python3 -m ziglang cc --version | head -1))"
  compile() { python3 -m ziglang cc $OPT -nostdlib -static -fno-pie -no-pie -Wl,-e,_start "$1" -o "$2"; }
else
  LLVM="llvmlite $(python3 -c 'import llvmlite.binding as l; print(".".join(map(str, l.llvm_version_info)))')"
  compile() { python3 selfhost/native/llvm/llc.py "$1" "$2.o" $OPT && ld -static -e _start -o "$2" "$2.o"; }
fi
echo "LLVM: $LLVM ($OPT)"

# Distingue un timeout de un programa que termina normalmente con código 124.
run_limited() {
  local marker="$1"
  shift
  rm -f "$marker"
  timeout "$TIMEOUT" bash -c '
    marker=$1
    shift
    "$@"
    rc=$?
    : > "$marker"
    exit "$rc"
  ' _ "$marker" "$@"
  local rc=$?
  [ -e "$marker" ] || return 124
  return "$rc"
}

# Todos los .ll de una vez (el runtime se revisa una sola vez).
mkdir -p "$tmp/ll"
./selfhost/build_llvm --lote "$tmp/ll" "${FILES[@]}"

pass=0; fail=0; unsupported=0; native_only=0; timed_out=0
for f in "${FILES[@]}"; do
  base="$(basename "$f" .titan)"
  if [ ! -f "$tmp/ll/$base.ll" ]; then
    unsupported=$((unsupported + 1))
    echo "no admitido: $f: $(head -c 300 "$tmp/ll/$base.err" 2>/dev/null | tr '\n' ' ')"
    continue
  fi
  rm -f "$tmp/prog" "$tmp/prog.o"
  if ! compile "$tmp/ll/$base.ll" "$tmp/prog" > "$tmp/llvm.txt" 2>&1; then
    fail=$((fail + 1))
    echo "DIFERENCIA en $f: LLVM rechazó el IR: $(head -c 400 "$tmp/llvm.txt" | tr '\n' ' ')"
    continue
  fi
  rm -f "$tmp/ll/$base.ll"
  vm_arg=""
  native_arg=""
  case "$base" in
    image_gif|image_gif_validate)
      vm_arg="$tmp/gif-vm"
      native_arg="$tmp/gif-llvm"
      ;;
    image_webp|image_webp_validate)
      vm_arg="$tmp/webp-vm"
      native_arg="$tmp/webp-llvm"
      ;;
    image_webp_write)
      vm_arg="$tmp/webp-write-vm"
      native_arg="$tmp/webp-write-llvm"
      ;;
    image_io)
      vm_arg="$tmp/image-io-vm"
      native_arg="$tmp/image-io-llvm"
      ;;
  esac
  if [ -n "$vm_arg" ]; then
    run_limited "$tmp/vm.done" "$ZETT" run "$f" "$vm_arg" > "$tmp/vm.out" 2> "$tmp/vm.err"; vm=$?
  else
    run_limited "$tmp/vm.done" "$ZETT" run "$f" > "$tmp/vm.out" 2> "$tmp/vm.err"; vm=$?
  fi
  vm_timeout=0
  [ -e "$tmp/vm.done" ] || vm_timeout=1
  if [ -n "$native_arg" ]; then
    run_limited "$tmp/nat.done" "$tmp/prog" "$native_arg" > "$tmp/nat.out" 2> "$tmp/nat.err"; nat=$?
  else
    run_limited "$tmp/nat.done" "$tmp/prog" > "$tmp/nat.out" 2> "$tmp/nat.err"; nat=$?
  fi
  nat_timeout=0
  [ -e "$tmp/nat.done" ] || nat_timeout=1
  if [ "$vm_timeout" -eq 1 ] || [ "$nat_timeout" -eq 1 ]; then
    timed_out=$((timed_out + 1))
    echo "TIEMPO AGOTADO en $f (VM=$vm LLVM=$nat; límite ${TIMEOUT}s)"
    continue
  fi
  if [ "$base" = "image_webp_unsupported" ]; then
    # La VM acepta animaciones y lee el primer cuadro; el runtime nativo
    # rechaza esa característica. Comprueba el contrato nativo explícito.
    if [ "$nat" = 0 ] && [ ! -s "$tmp/nat.err" ] && cmp -s selfhost/native/image/webp_unsupported.expected "$tmp/nat.out"; then
      native_only=$((native_only + 1))
    else
      fail=$((fail + 1))
      echo "DIFERENCIA en $f: salida nativa distinta del rechazo esperado"
      diff -u selfhost/native/image/webp_unsupported.expected "$tmp/nat.out" | head -8
      cat "$tmp/nat.err" | head -4
    fi
    continue
  fi
  if cmp -s "$tmp/vm.out" "$tmp/nat.out" && cmp -s "$tmp/vm.err" "$tmp/nat.err" && [ "$vm" = "$nat" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "DIFERENCIA en $f (salida VM=$vm LLVM=$nat):"
    diff "$tmp/vm.out" "$tmp/nat.out" | head -8
    diff "$tmp/vm.err" "$tmp/nat.err" | head -4
  fi
done
echo "programas: ${#FILES[@]}  idénticos: $pass  rechazos nativos esperados: $native_only  distintos: $fail  tiempos agotados: $timed_out  no admitidos: $unsupported"
[ "$fail" -eq 0 ] && [ "$timed_out" -eq 0 ] && [ "$unsupported" -eq 0 ]
