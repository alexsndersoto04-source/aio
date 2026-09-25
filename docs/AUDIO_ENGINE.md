# Motor de audio nativo — `std::audio::engine_*` (Fase 42A)

Titan suena por sí solo: decodifica **MP3/FLAC/Vorbis/Opus/WAV/AAC dentro
del binario** (`symphonia`, Rust puro) y empuja el PCM a las bocinas
(`cpal`: CoreAudio / WASAPI / ALSA). Sin mpv, sin afplay, sin ayudantes.

```
 tu .titan → engine_play() → symphonia (decode) → resample/EQ/crossfade
          → cpal → bocinas
```

## Cuándo usar qué

| API | Qué es | Funciona en |
|---|---|---|
| `std::audio::tags/duration/scan_library/cover` (Fase 41) | Metadatos y biblioteca | Todos lados |
| `std::audio::player_*` (Fase 41) | Controla mpv/afplay/etc del sistema | Donde haya reproductor instalado |
| **`std::audio::engine_*` (Fase 42A)** | **Titan decodifica y suena** | Linux/Win/mac con bocinas; decode headless en CI; decode+cola en Android |

Regla: todo lo nuevo usa `engine_*`. `player_*` queda como respaldo para
máquinas donde quieras delegar (o sin bocinas pero con mpv + `ao=null`).

## Transporte

```titan
print(std::audio::engine_play("/musica/a.mp3"))  // "playing ... via hifi-engine"
print(std::audio::engine_state())                // "playing" | "paused" | "idle"
print(std::audio::engine_position())             // segundos; -1.0 si idle
print(std::audio::engine_duration())             // segundos; 0.0 si se ignora
print(std::audio::engine_seek(42.0))             // seek exacto a nivel de muestra
print(std::audio::engine_pause())
print(std::audio::engine_resume())
print(std::audio::engine_stop())
print(std::audio::engine_device())               // "Built-in Audio" o "" sin salida
```

`engine_play` falla rápido y tipado: archivo inexistente, archivo no
decodificable o máquina sin salida (`NoDevice` en CI/headless) devuelven
`Result::Err` en vez de colgar o fingir.

## Cola gapless + crossfade + historial

```titan
print(std::audio::engine_queue_add("/musica/b.mp3"))  // funciona antes del play
print(std::audio::engine_queue_add("/musica/c.flac"))
print(std::audio::engine_queue_list())                // Array de rutas
print(std::audio::engine_next())                      // salta; al final: stop
print(std::audio::engine_prev())                      // >3 s: reinicia; si no: atrás
print(std::audio::engine_queue_clear())
let actual = std::audio::engine_current()             // {path, codec, position_secs, duration_secs, state, queue_len}
```

* **Gapless** (defecto): la pista siguiente se abre en cuanto la actual
  termina, sin hueco y sin clic.
* **Crossfade** (`engine_set_crossfade(2.0)`): los últimos N segundos se
  mezclan muestra a muestra con la cabeza de la siguiente. `0.0` lo apaga.
* **Sin gapless** (`engine_set_gapless(false)`): se insertan 250 ms de
  silencio entre pistas.
* El **historial** (200 pistas) alimenta `engine_prev`; `engine_play`
  conserva la cola y manda la saliente al historial.

## Ajustes (sobreviven entre pistas y sesiones)

```titan
print(std::audio::engine_set_volume(70))      // 0..100, true si quedó
print(std::audio::engine_set_crossfade(2.0))  // 0.0..12.0 segundos
print(std::audio::engine_set_gapless(true))   // booleano
print(std::audio::engine_set_eq(3, 0, -2))    // graves/medios/agudos, -12..12 dB
let st = std::audio::engine_status()          // todo junto para la UI de ajustes
print(st.volume)          // 70
print(st.crossfade_secs)  // 2.0
print(st.gapless)         // true
print(st.eq[0])           // 3 (graves)
```

El EQ es lowshelf 250 Hz + peaking 1 kHz + highshelf 4 kHz (RBJ); en
`[0,0,0]` el camino es bit-transparente. Volumen y EQ se aplican antes
de la salida con clamp de seguridad (el EQ no puede romper el conversor).

`engine_status` devuelve el mapa completo para pintar la pantalla de
ajustes y el ahora-suena: `state, path, codec, position_secs,
duration_secs, sample_rate, channels, volume, crossfade_secs, gapless,
eq[3], queue_len, device`.

## Visualizador

```titan
let barras = std::audio::engine_levels()  // Array de 32 Floats 0.0..1.0
print(std::collections::length(barras))   // 32
```

32 bandas logarítmicas de 60 Hz a 16 kHz, calculadas sobre el mono de lo
que está sonando (post-volumen y post-EQ). En idle devuelve ceros: la UI
lo sondea sin manejar errores.

## Chequeo headless (CI, contenedores, Android)

```titan
let rep = std::audio::engine_decode("/musica/a.mp3")
print(rep.codec)          // "mp3"
print(rep.duration_secs)  // cabecera o medida
print(rep.sample_rate)    // 44100
print(rep.channels)       // 2
print(rep.verified_secs)  // segundos realmente decodificados (tope 5 s)
print(rep.peak)           // pico 0.0..1.0 del tramo verificado
```

No necesita bocinas ni abre hilos de salida: es lo que corre en CI y lo
que usará la caché de streaming de Telegram (Fase 43).

## Cómo funciona por dentro (resumen honesto)

* Un hilo decodifica por adelantado a un anillo de ~8 s; el callback de
  `cpal` solo copia (nada pesado en tiempo real).
* Todo se remuestrea a la tasa del dispositivo (lineal) y se adapta a sus
  canales (mono↔estéreo); el seek reposiciona contador y decodificador.
* Límites: cola 500, historial 200, decode acotado por segundos, archivos
  podridos se saltan (una pista rota no mata la sesión).
* Linux necesita `libasound2-dev` para compilar (la CI lo instala);
  macOS/Windows no necesitan nada. Android compila decode+cola+ajustes;
  la salida la pone el shell de la app (Fase 42B).

## Ejemplo completo

`examples/reproductor_hifi.titan` — genera dos tonos, verifica decode
headless, ajusta volumen/crossfade/EQ, encola y reproduce con niveles:

```bash
zett run examples/reproductor_hifi.titan
```
