# Audio — procesamiento, metadatos y motor de reproducción (`std::audio`)

`std::audio` es la base para construir música y reproductores en TITAN.
Todo es Rust puro, sin dependencias nativas de audio (no ALSA, no AAudio,
no PulseAudio al compilar): el motor de reproducción **usa los
reproductores que ya están instalados en el sistema**.

Tres capas:

| Capa | Nativos | Funciona en |
|---|---|---|
| **WAV I/O + síntesis** (Fase 9) | `read_wav`, `read_wav_bytes`, `write_wav`, `encode_wav`, `sine_wave`, `square_wave`, `saw_wave`, `white_noise`, `fade_in`, `fade_out` | Cualquier máquina (100 % Rust, crate `hound`) |
| **Metadatos + biblioteca** (Fase 41) | `tags`, `duration`, `scan_library` | Cualquier máquina (parser propio de ID3v1/v2, Vorbis, FLAC, WAV INFO) |
| **Reproducción** | Termux (Fase 9): `play`, `pause`, `resume`, `stop`, `info`, `record_*` · Escritorio (Fase 41): `player_*` | `termux-api` en Android; mpv/afplay/paplay/aplay/ffplay/SoundPlayer en escritorio |

## 1. Metadatos de pistas (`std::audio::tags` / `duration`)

```titan
let info = std::audio::tags("/musica/cancion.mp3")
print(info.title)          // "Canción" o nil
print(info.artist)         // "Artista" o nil
print(info.album)          // nil si no hay etiqueta
print(info.year)           // "1998" o nil
print(info.genre)          // "Rock"
print(info.track)          // 7 o nil
print(info.duration_secs)  // 214.5 (0.0 si el formato no lo dice)
print(info.sample_rate)    // 44100
print(info.channels)       // 2
print(info.bitrate_kbps)   // 320 (solo MP3; 0 en WAV/FLAC/OGG)
print(info.has_cover)      // true si trae carátula embebida

let solo_duracion = std::audio::duration("/musica/cancion.flac") // Float

// Carátula embebida en bytes (ID3v2 APIC/PIC en MP3, PICTURE en FLAC);
// vector vacío cuando el archivo no trae portada.
let portada = std::audio::cover("/musica/cancion.mp3")
```

### Formatos y etiquetas soportados

| Formato | Etiquetas leídas | Duración |
|---|---|---|
| **MP3** | ID3v2.2/2.3/2.4 (TIT2, TPE1, TALB, TPE2, TCON, TYER/TDRC, TRCK; texto ISO-8859-1, UTF-8 y UTF-16 con/sin BOM) + ID3v1 de respaldo + género ID3v1 estándar | Exacta con cabecera Xing/Info (VBR); estimada por CBR si no la hay |
| **WAV** | Chunk `LIST`/`INFO` (INAM, IART, IPRD, IGNR, ICRD, ITRK) y chunks `id3 `/`ID3 ` embebidos | Exacta (bytes de `data` / tasa de bytes) |
| **FLAC** | VORBIS_COMMENT (TITLE, ARTIST, ALBUM, ALBUMARTIST, GENRE, DATE, TRACKNUMBER) | Exacta (STREAMINFO) |
| **OGG Vorbis** | Comentario Vorbis | Exacta (granule de la última página) |
| **OGG Opus** | OpusTags | Exacta (granule menos pre-skip, a 48 kHz) |

La lectura es tolerante a archivos corruptos: todo está verificado contra
límites y con topes de iteración, así que un archivo roto devuelve menos
campos, nunca cuelga la VM.

## 2. Biblioteca (`std::audio::scan_library`)

```titan
let biblioteca = std::audio::scan_library("/musica")
let total = std::collections::length(biblioteca)
for i in 0..total {
    let t = biblioteca[i]
    print(t.path)             // ordenado por ruta
    print(t.title)
    print(t.duration_secs)
}
```

Recorre la carpeta en profundidad (máx. 8 niveles, máx. 5000 pistas),
filtra por extensión (`mp3`, `wav`, `flac`, `ogg`, `oga`, `opus`), lee los
metadatos de cada pista y devuelve la lista ordenada por ruta. Los
archivos ilegibles se saltan: una pista rota no rompe el escaneo.

## 3. Reproducción en escritorio (`std::audio::player_*`)

TITAN no lleva audio nativo dentro del binario: controla un reproductor
que ya esté instalado. Backend detectado en este orden: **mpv → afplay →
paplay → aplay → ffplay** (y `Media.SoundPlayer` vía PowerShell en
Windows, solo WAV).

```titan
print(std::audio::player_backend())      // "mpv", "afplay", ... o ""

print(std::audio::player_play("/musica/a.mp3"))  // "playing ... via mpv"
print(std::audio::player_position())     // segundos (Float); -1.0 si idle
print(std::audio::player_pause())        // o Result::Err tipado
print(std::audio::player_resume())
let aplicado = std::audio::player_set_volume(60) // true si el backend lo aplicó
print(std::audio::player_seek(42.0))
print(std::audio::player_queue_add("/musica/b.mp3"))  // cola real con mpv
print(std::audio::player_queue_clear())
print(std::audio::player_next())          // saltar a la siguiente pista (mpv)
print(std::audio::player_prev())          // volver a la anterior (mpv)
let actual = std::audio::player_current() // {index, count, title, path} (mpv)
print(actual.title)
print(std::audio::player_stop())
```

### Matriz de soporte por backend

| Backend | Plataforma | play | pause/resume | position | seek | volume | cola |
|---|---|---|---|---|---|---|---|
| `mpv` | Unix | ✅ | ✅ | ✅ (exacta) | ✅ | ✅ | ✅ |
| `afplay` | macOS | ✅ | ✅ (señales) | ✅ (reloj) | ❌ | ❌ | ❌ |
| `paplay` | Linux/Pulse | ✅ | ✅ (señales) | ✅ (reloj) | ❌ | ❌ | ❌ |
| `aplay` | Linux/ALSA | ✅ | ✅ (señales) | ✅ (reloj) | ❌ | ❌ | ❌ |
| `ffplay` | cualquiera | ✅ | ✅ (señales) | ✅ (reloj) | ❌ | ❌ | ❌ |
| `soundplayer` | Windows | ✅ (WAV) | ❌ | ✅ (reloj) | ❌ | ❌ | ❌ |

Las funciones que el backend no soporta devuelven un error **tipado**
(`Result::Err`), así que un reproductor escrito en TITAN consulta
`player_backend()` una vez y degrada con elegancia:

```titan
let salto = std::try::catch(|| std::audio::player_seek(1.0))
match salto {
    Result::Ok(msg) => print(msg),
    Result::Err(e) => print("este backend no hace seek: " + e),
}
```

Detalles de diseño:

* **mpv se controla por su socket JSON-IPC** (`--input-ipc-server`) con
  `UnixStream` de la std: pausa, seek, volumen y playlist reales, posición
  exacta leída del reproductor.
* En los backends simples, pausa/resume usan **SIGSTOP/SIGCONT** sobre el
  proceso del reproductor y la posición se calcula por reloj (resta las
  pausas).
* `player_play` detiene lo que sonaba antes de empezar; el proceso del
  backend queda bajo control de la VM (stop = kill + reap).
* Termux/Android mantiene además la vía clásica de la Fase 9
  (`termux-media-player`): `std::audio::play`, `pause`, `stop`, `record_*`.

## 4. Ejemplo completo

`examples/reproductor_musica.titan` — genera dos tonos, lee sus
metadatos, escanea la biblioteca y reproduce con pausa/resume/volumen/
cola/stop, degradando según el backend disponible:

```bash
zett run examples/reproductor_musica.titan
```
