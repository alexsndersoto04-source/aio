# Audio en Titan (`std::audio`)

Dos cosas distintas que conviene no confundir:

1. **Decodificar** — convertir un archivo comprimido (mp3, flac, ogg, m4a) en
   muestras PCM con las que Titan puede calcular.
2. **Reproducir** — sacar sonido por el dispositivo. Titan no enlaza
   ALSA/CoreAudio/WASAPI: delega en un reproductor del sistema.

## Decodificacion

Con la feature `audio_decode_mod` (incluida en el binario `zett` por defecto)
Titan decodifica audio comprimido por si mismo, usando
[symphonia](https://github.com/pdeljanov/Symphonia) en Rust puro. No hay C-deps
ni FFI, asi que funciona igual en escritorio, en Android y en los cruces ARM.

| Nativa | Firma | Devuelve |
|---|---|---|
| `std::audio::formats()` | `() -> array` | Formatos que este build sabe decodificar |
| `std::audio::probe(path)` | `(string) -> map` | Metadata sin decodificar el audio entero |
| `std::audio::decode(path)` | `(string) -> map` | PCM completo en memoria |
| `std::audio::decode_to_wav(src, dst)` | `(string, string) -> map` | Decodifica y escribe un WAV |

`decode` y `decode_to_wav` devuelven un map con:

| Campo | Tipo | Significado |
|---|---|---|
| `samples` | `[float]` | Muestras en `[-1.0, 1.0]`, interleaved: `[L0, R0, L1, R1, ...]` |
| `sample_rate` | `int` | Hz (44100, 48000, ...) |
| `channels` | `int` | 1 = mono, 2 = estereo |
| `frames` | `int` | Muestras por canal (`samples.len() / channels`) |

`probe` devuelve `format` (nombre real del codec: `mp3`, `flac`, `pcm_s16le`),
`sample_rate`, `channels`, `duration_ms` y `frames`.

Formatos soportados: **wav, aiff, mp3, mp2, mp1, flac, ogg/vorbis, m4a/aac,
alac, adpcm, mkv**.

## Reproduccion

`std::audio::play(path)` elige solo el reproductor, en este orden de prioridad:

| Backend | Donde | Formatos |
|---|---|---|
| `termux-media-player` | Android/Termux (`pkg install termux-api`) | Lo que decodifique Android: mp3, m4a, ogg, ... |
| `mpv` | Escritorio | Practicamente todo |
| `ffplay` | Escritorio (ffmpeg) | Practicamente todo |
| `paplay` | PulseAudio/PipeWire | Solo WAV/FLAC |
| `aplay` | ALSA | Solo WAV PCM |

En Android el orden deja `termux-media-player` primero, asi que el
comportamiento existente no cambia.

| Nativa | Firma | Notas |
|---|---|---|
| `std::audio::backends()` | `() -> array` | Los que estan instalados, en orden de prioridad |
| `std::audio::backend()` | `() -> string` | El que usaria `play()`, o `"none"` |
| `std::audio::play(path)` | `(string) -> string` | No bloqueante: devuelve enseguida |
| `std::audio::play_with(path, backend)` | `(string, string) -> string` | Fuerza un backend |
| `std::audio::stop()` | `() -> string` | Mata la reproduccion en curso |
| `std::audio::pause()` / `resume()` | `() -> string` | Solo `termux-media-player` |

`play` no bloquea y registra el proceso hijo, por lo que `stop()` puede
cortarlo. En escritorio `pause`/`resume` devuelven un error explicito en vez de
fallar en silencio: usa `stop()` y `play()` de nuevo.

## Ejemplo

```titan
fn main() {
    // Que puede hacer esta maquina
    let actual = std::audio::backend()
    print("reproductor: {actual}")

    // Metadata barata, sin decodificar
    let meta = std::audio::probe("/sdcard/Music/cancion.mp3")
    let codec = meta.format
    let hz = meta.sample_rate
    let ms = meta.duration_ms
    print("{codec} a {hz} Hz, {ms} ms")

    // Reproducir
    std::audio::play("/sdcard/Music/cancion.mp3")

    // ... mas tarde
    std::audio::stop()
}
```

Y para procesar el audio (fades, mezcla, visualizador, analisis):

```titan
fn main() {
    let pcm = std::audio::decode("/sdcard/Music/cancion.mp3")
    let canales = pcm.channels
    let total = pcm.frames
    print("{total} cuadros en {canales} canales")

    // Los backends que solo tocan WAV (aplay, paplay) necesitan este paso:
    std::audio::decode_to_wav("/sdcard/Music/cancion.mp3", "/tmp/cancion.wav")
    std::audio::play_with("/tmp/cancion.wav", "aplay")
}
```

## Esqueleto de reproductor

Con las piezas de arriba se arma un reproductor completo. Este es el esqueleto
minimo, encima del cual va tu interfaz, tu lista real (usando `std::fs` para
listar carpetas) y tu logica de shuffle/repeat:

```titan
fn main() {
    // ---- diagnostico: que hay en esta maquina ----
    let bks = std::audio::backends()
    for b in bks {
        let s = b
        print("reproductor disponible: {s}")
    }

    // ---- la cola del reproductor ----
    let cola = ["/sdcard/Music/cancion1.mp3", "/sdcard/Music/cancion2.flac"]

    // ---- reproducir una a una, con la metadata a mano ----
    for ruta in cola {
        let r = ruta
        let meta = std::audio::probe(r)
        let fmt = meta.format
        let ms = meta.duration_ms
        print("[{fmt}] {r} ({ms} ms)")
        std::audio::play(r)
    }

    // ---- controles ----
    std::audio::pause()
    std::audio::resume()
    std::audio::stop()
}
```

Para un control mas fino:

- `std::audio::play_with(ruta, backend)` fuerza un reproductor concreto de la
  lista que devuelve `std::audio::backends()`.
- `std::audio::decode(ruta)` te da el audio en crudo (PCM) para lo que el
  reproductor delegado no puede hacer: fades, ecualizador, mezclas,
  visualizadores, analisis. `samples` son floats intercalados en
  [-1.0, 1.0], a `sample_rate` Hz y `channels` canales.
- `std::audio::decode_to_wav(origen, destino)` convierte cualquier formato a
  WAV para los backends que solo entienden WAV (`aplay`, `paplay`).

## Limites honestos

- `decode` carga la pista entera en memoria: un mp3 de 4 minutos a 44.1 kHz
  estereo son ~40 MB de `float`. Para streaming hace falta decodificacion
  incremental, que aun no esta expuesta.
- No hay salida de audio nativa: siempre hay un reproductor externo de por
  medio. Si no hay ninguno instalado, `std::audio::backend()` devuelve `"none"`
  y `play()` falla con un error claro.
- `pause`/`resume` solo existen en el backend de Termux.
