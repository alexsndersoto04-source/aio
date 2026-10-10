#!/usr/bin/env python3
"""Genera los archivos de audio de prueba (libsndfile vía `soundfile`). Determinista salvo el codificador."""
import sys, os, math, random
import numpy as np
import soundfile as sf

out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "fixtures")
os.makedirs(out, exist_ok=True)
rnd = np.random.RandomState(1234)

def signal(rate, secs, ch, amp=0.6):
    n = int(rate * secs)
    t = np.arange(n) / rate
    cols = []
    for c in range(ch):
        f = 220.0 * (c + 1)
        s = 0.5 * np.sin(2 * math.pi * f * t) + 0.3 * np.sin(2 * math.pi * f * 2.7 * t + c)
        s += 0.05 * rnd.randn(n)
        env = np.minimum(1.0, np.minimum(t * 20, (secs - t) * 20))
        cols.append(s * env)
    x = np.stack(cols, axis=1) * amp
    return np.clip(x, -1, 1)

def w(name, rate, secs, ch, fmt, sub, amp=0.6, **kw):
    x = signal(rate, secs, ch, amp)
    sf.write(os.path.join(out, name), x, rate, format=fmt, subtype=sub, **kw)

w("pcm_u8_mono.wav", 8000, 0.5, 1, "WAV", "PCM_U8")
w("pcm16_mono.wav", 22050, 0.4, 1, "WAV", "PCM_16")
w("pcm16_stereo.wav", 44100, 0.3, 2, "WAV", "PCM_16")
w("pcm24_stereo.wav", 48000, 0.2, 2, "WAV", "PCM_24")
w("pcm32_mono.wav", 16000, 0.3, 1, "WAV", "PCM_32")
w("float32_stereo.wav", 44100, 0.2, 2, "WAV", "FLOAT")
w("float64_mono.wav", 32000, 0.2, 1, "WAV", "DOUBLE")
w("alaw_mono.wav", 8000, 0.4, 1, "WAV", "ALAW")
w("ulaw_mono.wav", 8000, 0.4, 1, "WAV", "ULAW")
w("ms_adpcm_mono.wav", 22050, 0.4, 1, "WAV", "MS_ADPCM")
w("ms_adpcm_stereo.wav", 44100, 0.3, 2, "WAV", "MS_ADPCM")
w("ima_adpcm_mono.wav", 22050, 0.4, 1, "WAV", "IMA_ADPCM")
w("ima_adpcm_stereo.wav", 44100, 0.3, 2, "WAV", "IMA_ADPCM")
w("wavex_6ch.wav", 48000, 0.1, 6, "WAVEX", "PCM_24")
w("loud_clip.wav", 44100, 0.2, 2, "WAV", "PCM_16", amp=1.5)
w("silence.wav", 8000, 0.2, 1, "WAV", "PCM_16", amp=0.0)

w("s16_stereo.flac", 44100, 0.6, 2, "FLAC", "PCM_16")
w("s16_mono.flac", 22050, 0.5, 1, "FLAC", "PCM_16")
w("s24_stereo.flac", 48000, 0.4, 2, "FLAC", "PCM_24")
w("s8_mono.flac", 8000, 0.4, 1, "FLAC", "PCM_S8")
w("flac_5ch.flac", 44100, 0.2, 5, "FLAC", "PCM_16")
w("silence.flac", 44100, 0.3, 2, "FLAC", "PCM_16", amp=0.0)
w("noise_high.flac", 96000, 0.3, 2, "FLAC", "PCM_24", amp=0.95)
w("q_low.ogg", 22050, 0.8, 1, "OGG", "VORBIS", compression_level=0.1)
w("q_mid.ogg", 44100, 1.0, 2, "OGG", "VORBIS", compression_level=0.5)
w("q_high.ogg", 48000, 0.7, 2, "OGG", "VORBIS", compression_level=0.9)
w("mono44.ogg", 44100, 0.6, 1, "OGG", "VORBIS")
w("long_6s.ogg", 44100, 6.5, 2, "OGG", "VORBIS", compression_level=0.4)
w("long_6s.flac", 44100, 6.5, 2, "FLAC", "PCM_16")
w("silence.ogg", 44100, 0.3, 2, "OGG", "VORBIS", amp=0.0)
# formatos que symphonia 0.5.5 (features por defecto) NO decodifica
w("nope_opus.opus", 48000, 0.3, 2, "OGG", "OPUS")
w("nope.mp3", 44100, 0.5, 2, "MP3", "MPEG_LAYER_III")
w("nope.aiff", 44100, 0.2, 2, "AIFF", "PCM_16")
w("nope.au", 44100, 0.2, 2, "AU", "PCM_16")
w("nope.caf", 44100, 0.2, 2, "CAF", "PCM_16")
w("nope.w64", 44100, 0.2, 2, "W64", "PCM_16")
