#!/usr/bin/env bash
# Pruebas de imagen usando el runtime nativo que ya importa estos módulos.
#
# El lado "VM" era la VM de Rust (`zett run`); ahora es la VM de bytecode del
# propio port (`titan build` + `titan exec`), y los binarios nativos los arma la
# CLI del port (`titan compile`). La comparación es la misma: la salida del
# programa tiene que coincidir byte a byte, y en las pruebas de GIF/WebP también
# los cuadros que escribe el validador.
set -euo pipefail
cd "$(dirname "$0")/../../.."
TITAN="${TITAN:-$PWD/selfhost/titan}"
work=$(mktemp -d)
rt=$(mktemp selfhost/native/image-runtime-XXXXXX.titan)
driver=$(mktemp selfhost/image-build-XXXXXX.titan)
exe=$(mktemp -u selfhost/image-build-exe-XXXXXX)
if [ "${VERIFY_KEEP:-0}" = 1 ]; then
    trap 'rm -f "$rt" "$driver" "$exe"; echo "resultados temporales: $work"' EXIT
else
    trap 'rm -rf "$work"; rm -f "$rt" "$driver" "$exe"' EXIT
fi
cat selfhost/native/runtime.titan > "$rt"
python3 - "$rt" "$driver" <<'PY'
import sys
from pathlib import Path
source = Path('selfhost/build.titan').read_text()
source = source.replace('"runtime.titan"', '"' + Path(sys.argv[1]).name + '"')
Path(sys.argv[2]).write_text(source)
PY
# El compilador de pruebas se arma una vez; tiene que quedar en selfhost/ para
# que encuentre el runtime temporal que está en selfhost/native/.
"$TITAN" compile "$driver" -o "$exe" > /dev/null
# La VM del port: compila el programa a bytecode y lo ejecuta con los
# argumentos que recibe la función.
vm_run() {
    local src="$1" tbc="$2"
    shift 2
    "$TITAN" build "$src" -o "$tbc" > /dev/null
    "$TITAN" exec "$tbc" "$@"
}
tests=(image_base image_pixels image_resample image_png image_bmp image_jpeg image_gif image_webp_lossless_read image_webp_lossy_read image_webp_unsupported image_webp_write image_io)
if [ -n "${IMAGE_TESTS:-}" ]; then
    read -r -a tests <<< "$IMAGE_TESTS"
fi
for test in "${tests[@]}"; do
    if [ "$test" = image_io ]; then
        vm_run "selfhost/tests/native/$test.titan" "$work/$test.tbc" "$work/image-output" > "$work/$test.vm"
        "$exe" "selfhost/tests/native/$test.titan" "$work/$test"
        test -x "$work/$test"
        "$work/$test" "$work/image-output" > "$work/$test.native"
    elif [ "$test" = "image_gif" ]; then
        vm_run "selfhost/tests/native/$test.titan" "$work/$test.tbc" "$work/image-gif-vm" > "$work/$test.vm"
        "$exe" "selfhost/tests/native/$test.titan" "$work/$test"
        test -x "$work/$test"
        "$work/$test" "$work/image-gif-native" > "$work/$test.native"
        "$exe" selfhost/tests/native/image_gif_validate.titan "$work/image_gif_validate"
        test -x "$work/image_gif_validate"
        vm_run selfhost/tests/native/image_gif_validate.titan "$work/image_gif_validate.tbc" "$work/image-gif-native" > "$work/$test.vm-decode-native"
        "$work/image_gif_validate" "$work/image-gif-native" > "$work/$test.native-decode-native"
        diff -u "$work/$test.vm-decode-native" "$work/$test.native-decode-native"
    elif [ "$test" = "image_webp_write" ]; then
        vm_run "selfhost/tests/native/$test.titan" "$work/$test.tbc" "$work/image-webp-vm" > "$work/$test.vm"
        "$exe" "selfhost/tests/native/$test.titan" "$work/$test"
        test -x "$work/$test"
        "$work/$test" "$work/image-webp-native" > "$work/$test.native"
        "$exe" selfhost/tests/native/image_webp_validate.titan "$work/image_webp_validate"
        test -x "$work/image_webp_validate"
        vm_run selfhost/tests/native/image_webp_validate.titan "$work/image_webp_validate.tbc" "$work/image-webp-vm" > "$work/$test.vm-decode-vm"
        vm_run selfhost/tests/native/image_webp_validate.titan "$work/image_webp_validate.tbc" "$work/image-webp-native" > "$work/$test.vm-decode-native"
        "$work/image_webp_validate" "$work/image-webp-vm" > "$work/$test.native-decode-vm"
        "$work/image_webp_validate" "$work/image-webp-native" > "$work/$test.native-decode-native"
        diff -u "$work/$test.vm-decode-vm" "$work/$test.native-decode-vm"
        diff -u "$work/$test.vm-decode-native" "$work/$test.native-decode-native"
    else
        vm_run "selfhost/tests/native/$test.titan" "$work/$test.tbc" > "$work/$test.vm"
        "$exe" "selfhost/tests/native/$test.titan" "$work/$test"
        test -x "$work/$test"
        "$work/$test" > "$work/$test.native"
    fi
    diff -u "$work/$test.vm" "$work/$test.native"
    if [ "$test" = image_webp_unsupported ]; then
        echo "$test: primer cuadro y rechazos de entrada dañada idénticos a la VM"
    else
        echo "$test: idéntico a la VM"
    fi
done
