#!/usr/bin/env bash
# Programas de Titan SIN sistema operativo (aarch64-none): el compilador en
# Titan escribe el LLVM IR, LLVM lo convierte en una imagen ARM64 que arranca
# sola (MMU, puerto serie, PSCI) y se ejecuta en un procesador ARM64:
#   CORRER=unicorn (por defecto): native/bare/correr.py (motor de QEMU);
#   CORRER=qemu: qemu-system-aarch64 -machine virt de verdad (en CI).
# La salida del puerto serie (stdout y stderr van al mismo) se compara byte a
# byte con la VM de Rust (`zett run`, stdout+stderr juntos). Con unicorn se
# compara también el código de salida (PSCI SYSTEM_OFF lo deja en x1); QEMU
# no lo da, así que ahí solo cuenta la salida.
#
# Uso: bash selfhost/native/bare/probar.sh [PROGRAMA.titan...]
# Sin argumentos: las pruebas de selfhost/tests/native que no usan archivos,
# procesos, red, reloj, azar ni entrada (sin sistema operativo no existen y
# el runtime responde -ENOSYS, de verdad).
set -u
ZETT="${ZETT:-zett}"
CORRER="${CORRER:-unicorn}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if [ $# -gt 0 ]; then
  FILES=("$@")
else
  mapfile -t FILES < <(grep -L -E "std::(fs|process|net|env|os|path|http|tcp|udp|signal|term|dirs|time|thread|random|io|args|chan|sync|datetime|crypto|gui|window|image|audio|db|sql|compress|zip|tls|dns|mobile|password|uuid|freestanding_)|go |spawn" selfhost/tests/native/*.titan)
fi

if [ ! -x selfhost/build_llvm ] || [ -n "$(find selfhost -name '*.titan' -newer selfhost/build_llvm | head -1)" ]; then
  echo "compilando selfhost/build_llvm..."
  (cd selfhost && "$ZETT" run build.titan build_llvm.titan build_llvm) || exit 1
fi

if command -v clang >/dev/null 2>&1 && command -v ld.lld >/dev/null 2>&1; then
  compile() { clang --target=aarch64-none-elf -O2 -nostdlib -static -ffreestanding -fno-pic -fuse-ld=lld -Wl,-T,"$tmp/ll/bare.ld" "$1" -o "$2"; }
else
  compile() { python3 -m ziglang cc -target aarch64-freestanding-none -O2 -nostdlib -static -ffreestanding -fno-pic -Wno-override-module -Wl,-T,"$tmp/ll/bare.ld" "$1" -o "$2"; }
fi
if [ "$CORRER" = qemu ]; then
  run() { timeout 120 qemu-system-aarch64 -machine virt -cpu cortex-a57 -m 1G -nographic -monitor none -serial stdio -kernel "$1" < /dev/null; }
else
  run() { timeout 300 python3 selfhost/native/bare/correr.py "$1"; }
fi

mkdir -p "$tmp/ll"
./selfhost/build_llvm --lote-bare "$tmp/ll" "${FILES[@]}"

pass=0; fail=0; unsupported=0
for f in "${FILES[@]}"; do
  base="$(basename "$f" .titan)"
  if [ ! -f "$tmp/ll/$base.ll" ]; then
    unsupported=$((unsupported + 1))
    echo "no admitido: $f: $(head -c 300 "$tmp/ll/$base.err" 2>/dev/null | tr '\n' ' ')"
    continue
  fi
  rm -f "$tmp/prog.elf"
  if ! compile "$tmp/ll/$base.ll" "$tmp/prog.elf" > "$tmp/llvm.txt" 2>&1; then
    fail=$((fail + 1))
    echo "DIFERENCIA en $f: LLVM rechazó el IR: $(head -c 400 "$tmp/llvm.txt" | tr '\n' ' ')"
    continue
  fi
  rm -f "$tmp/ll/$base.ll"
  timeout 60 "$ZETT" run "$f" > "$tmp/vm.out" 2>&1; vm=$?
  run "$tmp/prog.elf" > "$tmp/bare.out" 2> "$tmp/bare.err"; bare=$?
  same_code=1
  if [ "$CORRER" != qemu ] && [ "$vm" != "$bare" ]; then same_code=0; fi
  if cmp -s "$tmp/vm.out" "$tmp/bare.out" && [ $same_code = 1 ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "DIFERENCIA en $f (salida VM=$vm sin-SO=$bare):"
    diff "$tmp/vm.out" "$tmp/bare.out" | head -8
    head -c 300 "$tmp/bare.err"
  fi
done
echo "programas: ${#FILES[@]}  idénticos: $pass  distintos: $fail  no admitidos: $unsupported"
[ "$fail" -eq 0 ] && [ "$unsupported" -eq 0 ]
