# Titan Music — App Android estilo Spotify (Fase UI + APK) 🎵📱

Una aplicación para Android con diseño oscuro auténtico estilo Spotify, navegación en 5 pestañas, biblioteca doble (las canciones de tu teléfono y las de tu nube de Telegram), cola de reproducción, carátulas iluminadas, sonido 100% garantizado y un panel de configuración Hi-Fi avanzado.

---

## 📥 Descarga Directa del APK

Puedes descargar el instalador oficial directamente en tu móvil:
* **Enlace directo de descarga (GitHub Releases):**
  [Descargar TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)
* También disponible en la sección de **Releases** y en los artefactos de la pestaña **Actions**.

---

## 🌟 Qué incluye la aplicación

### 1. Interfaz Auténtica Estilo Spotify
* **Paleta oscura Spotify:** fondo negro profundo (`#121212`), acentos verde Spotify (`#1DB954`) y azul Telegram (`#0088cc`).
* **5 Pestañas de navegación rápida:**
  1. **🏠 Inicio:** cuadrícula de escuchas recientes, acceso rápido y lista de temas recomendados.
  2. **🔍 Buscar:** buscador instantáneo en tiempo real por título, artista o álbum, además de tarjetas de exploración rápida por género (Rock, Synthwave, Lo-Fi Chill, Acústico, Dance & Electrónica).
  3. **📚 Biblioteca:** visión dual de tu música con filtros rápidos: `Todas`, `📱 En este teléfono`, `☁️ Nube Telegram` y `⬇️ Descargadas`.
  4. **☁️ Telegram:** pantalla dedicada con tarjeta de estado en vivo, información del canal privado «Mi Música», botón interactivo para probar el streaming de audio y formulario paso a paso.
  5. **⚙️ Ajustes:** panel directo de configuración Hi-Fi accesible en cualquier momento desde la barra inferior o desde el icono de engranaje superior.
* **Mini reproductor flotante:** persistente en toda la app con barra de progreso en vivo, carátula, controles táctiles y apertura del reproductor completo en pantalla deslizable.
* **Reproductor en pantalla completa:** carátula con brillo ambiental, vinilo giratorio, indicador de fuente (`☁️ Streaming puro desde Telegram` o `📱 Reproducción local`), barra de tiempo milimétrica, cola y acceso a ajustes.

### 2. Audio 100% Audible y Garantizado (Motor Dual)
* **Sonido real sin silencios:** la aplicación cuenta con un motor de audio de respaldo con 5 pistas musicales estéreo de alta calidad empaquetadas:
  - 🎸 **Rock:** estilo Soda Stereo (*De Música Ligera* / *Persiana Americana*).
  - 🎹 **Synthwave:** sintetizadores ochenteros y arpegios espaciales.
  - ☕ **Lo-Fi Chill:** sonido cálido de piano Rhodes y vinilo relajante.
  - 🎻 **Acústico:** arpegios de guitarra natural al atardecer.
  - ⚡ **Dance / Electrónica:** bombos contundentes y sintetizadores estilo The Weeknd.
* **Reproducción dual nativa + WebView:** el reproductor nativo de Android (`AudioPlaybackService`) gestiona notificaciones en segundo plano, controles de pantalla de bloqueo y ecualización del sistema, complementado por audio inmediato HTML5 en la interfaz.

### 3. Nube de Telegram y Streaming Puro (Sin Llenar el Teléfono)
* **Cero espacio ocupado:** al reproducir una canción de la nube, viaja únicamente el búfer necesario en memoria. El teléfono no se llena de archivos temporales ni consume tu almacenamiento.
* **Descarga explícita con botón `⬇️`:** cada canción de la nube tiene su botón para descargar al almacenamiento local solo cuando tú lo decidas.
* **Pestaña dedicada de Telegram:**
  - Diagnóstico de conexión en tiempo real (🟢 Conectado / 🟡 Desconectado).
  - Indicador del canal activo (`«Mi Música»`).
  - Botón **«Probar sonido streaming»** para verificar audio instantáneo desde la nube.
  - Botón **«Sincronizar canciones»** para refrescar la lista de temas.

### 4. Panel de Configuración Hi-Fi (Pestaña «⚙️ Ajustes»)
* **Control de Volumen:** deslizador fluido del 0% al 100% con porcentaje en tiempo real.
* **Fundido cruzado (Crossfade):** mezcla suave entre canciones consecutivas (de 0.0 a 12.0 segundos con botones rápidos de 2s, 4s, 8s, 12s) para transiciones sin saltos.
* **Ecualizador paramétrico de 3 bandas (EQ):**
  - **Graves (Bass / 250 Hz):** de -12 dB a +12 dB.
  - **Medios (Mids / 1000 Hz):** de -12 dB a +12 dB.
  - **Agudos (Treble / 4000 Hz):** de -12 dB a +12 dB.
  - **Presets de 1 toque:** *Plano*, *Bass Boost (Refuerzo de graves)*, *Vocal*, *Rock*, *Acústico* y *Electrónica*.
* **Visualizador de Espectro en Vivo (32 barras):** analizador de frecuencias animado en tiempo real según el ritmo y la ecualización.
* **Reproducción sin pausas (Gapless):** interruptor para eliminar los silencios entre canciones, ideal para conciertos en directo o álbumes continuos.
* **Selector de salida de audio:** cambia al instante entre el altavoz del teléfono, auriculares de cable o auriculares Bluetooth.

### 5. Cola de reproducción interactiva (Queue)
* Visualiza la pista que está sonando y la lista de próximas canciones.
* Añade cualquier pista a la cola con el botón **➕**.
* Vacía la cola completa con un solo toque en **«Vaciar cola»**.

---

## 📲 Cómo instalar el APK en tu teléfono Android (Sideload)

1. En el navegador de tu teléfono Android, descarga el archivo:
   👉 **[TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)**
2. Abre la notificación de descarga o busca el archivo en tu carpeta «Descargas».
3. Si el sistema te pregunta «¿Permitir instalar aplicaciones de fuentes desconocidas?», pulsa **Permitir** o **Ajustes > Autorizar descargas de este navegador**.
4. Pulsa el botón **Instalar**.
5. Abre **Titan Music** y disfruta de tu música.

---

## ☁️ Conexión inicial con Telegram (Pestaña «Telegram»)

1. Abre Titan Music y pulsa en la pestaña inferior **«Telegram»** (o en la píldora superior).
2. Verás el estado de la nube y el canal **«Mi Música»**.
3. Si deseas conectar una cuenta personalizada:
   - Introduce tu número de teléfono con código de país (por ejemplo `+34 600 111 222` o `+58 412 ...`).
   - Introduce el código oficial que te envía Telegram.
   - Pulsa **«Guardar y Conectar»**.
4. ¡Listo! Todas las canciones de tu canal privado de Telegram aparecerán en streaming instantáneo sin ocupar memoria de tu teléfono.
