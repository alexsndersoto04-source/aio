#!/usr/bin/env bash
# Verifica el decodificador de audio nativo (WAV/PCM/ADPCM/FLAC/Ogg/Vorbis) y el engine contra el
# oráculo (la VM de Rust, `zett run`). Requiere: zett, python3 con numpy+soundfile (PYTHONPATH), COMPILER.
# Uso: COMPILER=selfhost/titanc3 bash selfhost/tests/audio/verificar.sh
set -euo pipefail
cd "$(dirname "$0")/../../.."
COMPILER=${COMPILER:-selfhost/titanc3}
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
python3 selfhost/tests/audio/gen_fixtures.py selfhost/tests/audio/fixtures
zett run selfhost/tests/audio/decode_all.titan > "$T/oraculo.txt"
"$COMPILER" selfhost/tests/audio/decode_all.titan "$T/dec"
"$T/dec" > "$T/nativo.txt"
if cmp -s "$T/oraculo.txt" "$T/nativo.txt"; then echo "decode_all: idéntico ($(wc -l < "$T/nativo.txt") líneas)"; else echo "decode_all: DIFIERE"; diff "$T/oraculo.txt" "$T/nativo.txt" | head; exit 1; fi
"$COMPILER" selfhost/tests/audio/engine_flow.titan "$T/flow"
zett run selfhost/tests/audio/engine_flow.titan > "$T/flow_zett.txt" 2>&1 || true
"$T/flow" > "$T/flow_nat.txt" 2>&1 || true
# Las líneas que dependen del hardware (dispositivo ALSA) pueden diferir: se muestran, no se ocultan.
if cmp -s "$T/flow_zett.txt" "$T/flow_nat.txt"; then echo "engine_flow: idéntico"; else echo "engine_flow: diferencias (revisar que sean solo de hardware):"; diff "$T/flow_zett.txt" "$T/flow_nat.txt" || true; fi
echo "DSP (volumen/remuestreo/EQ) contra numpy: ver eng_ref.py (usa engine_sink.titan con TITAN_AUDIO_SINK=file:)"
