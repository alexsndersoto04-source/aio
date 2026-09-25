# Titan · Nube musical en Telegram (Fase 43) ☁️🎵

Tus canciones viven en un canal privado de Telegram («Mi Música») y
Titan las toca **directo de la nube**, igual que Spotify o YouTube.

## La idea en 30 segundos

- **Biblioteca doble:** ves juntas las canciones del teléfono 📱 y las
  de la nube ☁️, cada una con su marquita.
- **Streaming puro:** al darle play a una de la nube, solo viajan unos
  segundos por adelantado (unos pocos MB en memoria) y suena al
  instante. Al cambiar de canción, esa memoria se libera: tu teléfono
  siempre queda libre.
- **Nada se descarga solo.** Lo único que toca el disco es lo que TÚ
  pidas con `std::audio::cloud_download`. Sin internet solo suenan las
  del teléfono o las que guardaste explícitamente.

## Primera vez (5 minutos)

1. **Crea tu app de Telegram** (gratis, una sola vez):
   entra a <https://my.telegram.org> con tu número, ve a «API
   development tools» y anota el `api_id` (número) y el `api_hash`
   (texto largo). Es TU app personal; nadie más la usa.
2. **Guárdalos en Titan:**
   `std::audio::cloud_setup(api_id, api_hash)`
3. **Pide el código:** `std::audio::cloud_login_phone("+34600111222")`
   (tu número en formato internacional). Te llega un código a Telegram.
4. **Entra:** `std::audio::cloud_login_code("12345")`. Si tienes
   verificación en dos pasos, sigue con
   `std::audio::cloud_login_password("tu-clave")`.

Listo. La sesión queda guardada en `~/.titan/telegram.session.json`
y las próximas veces entras sin código. El canal «Mi Música» se crea
solo la primera vez que lo necesites (privado, solo tuyo).

`std::audio::cloud_status()` te dice en qué punto estás
(`{configured, connected, authorized, user, phone, channel, note}`) y
`std::audio::cloud_logout()` cierra la sesión (tus canciones siguen
intactas en Telegram).

## Subir, listar, tocar, guardar

```titan
// Subir (la duración y etiquetas se detectan solas)
print(std::audio::cloud_upload("/musica/la_banda-mi_cancion.mp3"))

// Listar la nube (refresca los cloud:<id>)
let nube = std::audio::cloud_library()
print(nube[0].title)    // "Mi Canción"
print(nube[0].artist)   // "La Banda"
print(nube[0].id)       // 12

// Tocar directo de la nube (streaming, cero disco)
print(std::audio::cloud_play(12))

// Meterla a la cola mixta (vale mezclar teléfono + nube)
print(std::audio::cloud_queue_add(12))

// Guardarla EN SERIO en el teléfono (acto explícito tuyo)
print(std::audio::cloud_download(12, "/musica/guardadas/mi_cancion.mp3"))

// Borrarla del canal
print(std::audio::cloud_delete(12))
```

## Cómo suena por dentro

Cada pista nube es `cloud:<id>` para el motor de la Fase 42A: el
mismo hilo decodificador, la misma cola gapless, el mismo crossfade,
el mismo ecualizador y las mismas 32 barras. Buscar (seek) re-pide el
audio desde el nuevo punto. Funciona en escritorio, en CI (headless)
y en Android (decode + cola; la salida la pone el shell).

Ejemplo completo y verificable: `examples/reproductor_nube.titan`
(corre en CI sin login y degrada con mensaje).

## Privacidad

Tu `api_id`/`api_hash` y tu sesión viven solo en tu máquina
(`~/.titan/cloud.json` y `~/.titan/telegram.session.json`). El canal
es privado: solo lo ves tú. Titan nunca sube nada sin que lo pidas.
