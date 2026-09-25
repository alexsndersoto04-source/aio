# Titan Music — App Android estilo Spotify Oficial (Fase UI + APK) 🎵📱

Una aplicación para Android con diseño ultra-limpio idéntico a Spotify Mobile, navegación nativa en 5 pestañas, biblioteca doble (las canciones de tu teléfono y las de tu nube de Telegram), cola de reproducción, controles de audio Hi-Fi y soporte completo para reproducir los archivos de música reales de tu teléfono.

---

## 📥 Descarga Directa del APK

Puedes descargar e instalar la versión oficial directamente en tu móvil:
* **Enlace directo de descarga (GitHub Releases):**
  [Descargar TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)
* También disponible en la pestaña de **Releases** y en los artefactos generados en **Actions**.

---

## 🌟 Qué incluye la aplicación

### 1. Interfaz Móvil Auténtica de Spotify (Sin Stickers ni Bordes Pesados)
* **Diseño minimalista y profesional:** acabados limpios con fondo negro azabache (`#121212`), tarjetas de superficie oscura (`#181818`), tipografía de alta legibilidad e iconos vectoriales SVG nítidos (cero emojis gigantes, bordes toscos o efectos de pegatina).
* **5 Pestañas de navegación rápida:**
  1. **Inicio:** saludo dinámico según la hora del día, accesos directos limpios, botón de exploración de archivos del teléfono y lista de pistas.
  2. **Buscar:** barra de búsqueda moderna con borrado en 1 toque y tarjetas por género musical.
  3. **Tu biblioteca:** filtros en píldora (`Todas`, `Música del teléfono`, `Telegram Cloud`) y listado minimalista con carátula cuadrada y opciones.
  4. **Telegram:** panel limpio con estado de conexión en la nube, canal activo y vinculación directa de cuenta.
  5. **Ajustes:** panel Hi-Fi completo (Volumen, Crossfade, Ecualizador paramétrico de 3 bandas, visualizador de 32 barras de espectro y selección de dispositivo).
* **Mini reproductor flotante:** barra compacta sobre la navegación con título, artista, botón me gusta, play/pausa y barra de progreso verde milimétrica. Toca para abrir el reproductor a pantalla completa.
* **Reproductor a pantalla completa:** carátula con sombra suave, información de pista y procedencia, barra de deslizamiento (scrubber), controles de reproducción, repetición y aleatorio.

### 2. Reproducción Real de tus Canciones del Teléfono
* **Lector de almacenamiento nativo (`MediaStore`):** la aplicación escanea automáticamente las canciones almacenadas en la memoria de tu móvil (formatos MP3, M4A, FLAC, WAV, AAC, etc.) a través de identificadores seguros de Android.
* **Selector de archivos del sistema:** puedes pulsar el botón de carpeta superior o «Examinar archivos» para abrir el explorador de documentos de Android y escoger cualquier archivo de música. Al seleccionarlo, la canción real se carga y empieza a sonar de inmediato.
* **Compatibilidad con Scoped Storage (Android 10, 11, 12, 13, 14 y 15):** lectura de descriptores de archivo a través de `ContentResolver`, garantizando que suenan tus canciones reales y no tonos de prueba.

### 3. Nube de Telegram y Streaming Puro (Sin Llenar el Teléfono)
* **Cero espacio ocupado:** al reproducir música desde Telegram viaja únicamente el búfer necesario para escuchar la pista de forma fluida. Tu teléfono no se satura de archivos temporales.
* **Descarga explícita opcional:** botón para guardar al almacenamiento local solo cuando desees tener la canción sin conexión.

### 4. Panel de Configuración Hi-Fi (Pestaña «Ajustes»)
* **Control de Volumen:** deslizador fluido del 0% al 100%.
* **Fundido cruzado (Crossfade):** mezcla suave entre canciones (de 0.0 a 12.0 segundos, con botones rápidos de 2s, 4s, 8s, 12s) para transiciones sin saltos.
* **Ecualizador de 3 bandas (EQ):**
  - **Graves (Bass):** de -12 dB a +12 dB.
  - **Medios (Mids):** de -12 dB a +12 dB.
  - **Agudos (Treble):** de -12 dB a +12 dB.
  - **Presets de 1 toque:** Plano, Bass Boost, Vocal, Rock, Acústico, Electrónica.
* **Visualizador de Espectro en Vivo (32 barras):** medidor de frecuencias animado.
* **Reproducción sin pausas (Gapless):** interruptor para eliminar los silencios entre pistas.
* **Dispositivo de salida:** conmutación entre el altavoz del teléfono, auriculares con cable o auriculares Bluetooth.

---

## 📲 Cómo instalar y usar en tu teléfono Android

1. Descarga el APK desde tu navegador móvil:
   👉 **[TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)**
2. Abre el archivo descargado. Si Android solicita autorización para instalar apps de fuentes desconocidas, pulsa **Permitir**.
3. Pulsa **Instalar** y luego **Abrir**.
4. Concede el permiso de lectura multimedia cuando la aplicación lo solicite para que pueda listar tu música.
5. Si deseas reproducir una canción específica de tus carpetas o descargas, pulsa el icono de carpeta o el botón **«Examinar archivos»** y selecciónala.
