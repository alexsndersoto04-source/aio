#!/usr/bin/env bash
# Pruebas de imagen usando el runtime nativo que ya importa estos módulos.
set -euo pipefail
cd "$(dirname "$0")/../../.."
work=$(mktemp -d)
rt=$(mktemp selfhost/native/image-runtime-XXXXXX.titan)
driver=$(mktemp selfhost/image-build-XXXXXX.titan)
if [ "${VERIFY_KEEP:-0}" = 1 ]; then
    trap 'rm -f "$rt" "$driver"; echo "resultados temporales: $work"' EXIT
else
    trap 'rm -rf "$work"; rm -f "$rt" "$driver"' EXIT
fi
cat selfhost/native/runtime.titan > "$rt"
python3 - "$rt" "$driver" <<'PY'
import sys
from pathlib import Path
source = Path('selfhost/build.titan').read_text()
source = source.replace('"runtime.titan"', '"' + Path(sys.argv[1]).name + '"')
Path(sys.argv[2]).write_text(source)
PY
tests=(image_base image_pixels image_resample image_png image_bmp image_jpeg image_gif image_webp_lossless_read image_webp_lossy_read image_webp_unsupported image_webp_write image_io)
if [ -n "${IMAGE_TESTS:-}" ]; then
    read -r -a tests <<< "$IMAGE_TESTS"
fi
for test in "${tests[@]}"; do
    if [ "$test" = image_io ]; then
        zett run "selfhost/tests/native/$test.titan" "$work/image-output" > "$work/$test.vm"
        zett run "$driver" "selfhost/tests/native/$test.titan" "$work/$test"
        test -x "$work/$test"
        "$work/$test" "$work/image-output" > "$work/$test.native"
    elif [ "$test" = "image_gif" ]; then
        zett run "selfhost/tests/native/$test.titan" "$work/image-gif-vm" > "$work/$test.vm"
        zett run "$driver" "selfhost/tests/native/$test.titan" "$work/$test"
        test -x "$work/$test"
        "$work/$test" "$work/image-gif-native" > "$work/$test.native"
        zett run "$driver" selfhost/tests/native/image_gif_validate.titan "$work/image_gif_validate"
        test -x "$work/image_gif_validate"
        zett run selfhost/tests/native/image_gif_validate.titan "$work/image-gif-native" > "$work/$test.vm-decode-native"
        "$work/image_gif_validate" "$work/image-gif-native" > "$work/$test.native-decode-native"
        diff -u "$work/$test.vm-decode-native" "$work/$test.native-decode-native"
    elif [ "$test" = "image_webp_write" ]; then
        zett run "selfhost/tests/native/$test.titan" "$work/image-webp-vm" > "$work/$test.vm"
        zett run "$driver" "selfhost/tests/native/$test.titan" "$work/$test"
        test -x "$work/$test"
        "$work/$test" "$work/image-webp-native" > "$work/$test.native"
        zett run "$driver" selfhost/tests/native/image_webp_validate.titan "$work/image_webp_validate"
        test -x "$work/image_webp_validate"
        zett run selfhost/tests/native/image_webp_validate.titan "$work/image-webp-vm" > "$work/$test.vm-decode-vm"
        zett run selfhost/tests/native/image_webp_validate.titan "$work/image-webp-native" > "$work/$test.vm-decode-native"
        "$work/image_webp_validate" "$work/image-webp-vm" > "$work/$test.native-decode-vm"
        "$work/image_webp_validate" "$work/image-webp-native" > "$work/$test.native-decode-native"
        diff -u "$work/$test.vm-decode-vm" "$work/$test.native-decode-vm"
        diff -u "$work/$test.vm-decode-native" "$work/$test.native-decode-native"
    else
        zett run "selfhost/tests/native/$test.titan" > "$work/$test.vm"
        zett run "$driver" "selfhost/tests/native/$test.titan" "$work/$test"
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
