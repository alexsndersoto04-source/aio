# Titan Audio — Reproductor de Música Hi-Fi para Android 🎵📱

Un reproductor de audio nativo y profesional para Android con diseño propio y exclusivo (sin imitaciones de Spotify ni stickers), barra interactiva de reproducción en la cortina de notificaciones de Android, lectura de carátulas reales incrustadas en tus canciones (ID3/FLAC) y reproducción directa de los archivos de tu teléfono.

---

## 📥 Descarga Directa del APK

Puedes descargar e instalar la versión oficial directamente en tu móvil:
* **Enlace directo de descarga (GitHub Releases):**
  [Descargar TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)
* También disponible en la pestaña de **Releases** de este repositorio.

---

## 🌟 Novedades y Características 100% Reales

### 1. Identidad Visual Propia y Exclusiva (Titan Audio)
* **Emblema original:** Logotipo geométrico en forma de «T» acústica con ondas resonantes en azul eléctrico cian (`#00e5ff`) y blanco titanio.
* **Sin imitaciones:** Se eliminaron por completo logos y referencias a Spotify.
* **Diseño ultra-limpio sin bordes toscos:** Fondo obsidiana profundo (`#090a0f`), superficies de titanio oscuro (`#131620`) y tipografía nítida con acabado de alta gama.

### 2. Barra de Reproducción en las Notificaciones de Android
* **Controles nativos multimedia (`MediaSessionCompat`):**
  - Aparece en la cortina de notificaciones y en la pantalla de bloqueo de tu teléfono.
  - Botones táctiles reales: **Anterior**, **Reproducir / Pausa**, **Siguiente**.
  - Muestra la carátula real del álbum, el título de la canción y el artista directamente en el sistema operativo.
  - Compatible con los botones de tus auriculares (cable y Bluetooth) para pausar y cambiar de pista.
* **Permiso `POST_NOTIFICATIONS`:** Configurado de forma nativa para que la notificación aparezca correctamente en Android 13, 14 y 15 sin ser bloqueada por el sistema.

### 3. Extracción Real de Carátulas Embebidas (Artwork)
* **Extractor de metadatos (`MediaMetadataRetriever`):** La app lee los bytes de imagen incrustados en las etiquetas ID3 / FLAC de tus archivos de música y los muestra en alta resolución tanto en el mini reproductor, como en el reproductor completo y en la barra de notificaciones.
* Si un archivo no tiene carátula integrada, muestra el monograma acústico de Titan Audio.

### 4. Reproducción 100% Real de tus Canciones
* **Cero simulaciones ni canciones demo:** La lista muestra exclusivamente la música real que tienes en tu teléfono.
* **Doble método de acceso:**
  1. **Escanear teléfono:** Consulta la base multimedia del móvil (`MediaStore`) y lista tus canciones en orden alfabético.
  2. **Abrir archivos (Selector del sistema):** Puedes pulsar el botón de carpeta para abrir el explorador de archivos de Android y seleccionar una o varias canciones a la vez desde tus carpetas o descargas.

### 5. Controles Hi-Fi y Ajustes
* **Volumen:** Deslizador maestro de audio.
* **Fundido cruzado (Crossfade):** Transición suave de 0 a 12 segundos entre canciones consecutivas.
* **Ecualizador de 3 bandas:** Ajuste de graves (250 Hz), medios (1 kHz) y agudos (4 kHz).
* **Gapless:** Reproducción continua sin pausas entre pistas.

---

## 📲 Pasos para instalar en tu teléfono Android

1. Descarga el archivo APK:
   👉 **[TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)**
2. Abre la notificación de descarga en tu móvil.
3. Si el sistema te pide autorización para instalar aplicaciones desconocidas, pulsa **Permitir**.
4. Pulsa **Instalar** y luego **Abrir**.
5. Concede los permisos de música y notificaciones cuando la app te los pida para activar la barra superior del sistema.
6. Pulsa **«Escanear teléfono»** o **«Abrir archivos»** para empezar a escuchar tu música.
