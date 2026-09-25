# Titan Music — App Android estilo Spotify (Fase UI + APK) 🎵📱

Una aplicación para Android con diseño oscuro estilo Spotify, biblioteca doble (las canciones de tu teléfono y las de tu nube de Telegram), cola de reproducción, carátulas y un reproductor de audio avanzado.

---

## 🌟 Qué incluye la aplicación

### 1. Biblioteca Doble (Teléfono 📱 + Nube Telegram ☁️)
* **Pestaña «En este teléfono»:** detecta y muestra automáticamente las canciones guardadas en el almacenamiento de tu teléfono (MP3, FLAC, WAV, M4A, OGG).
* **Pestaña «Nube Telegram»:** se conecta directamente a tu canal privado «Mi Música» de Telegram.
* **Pestaña «Todas»:** une toda tu música en una sola lista elegante con etiquetas distintivas (`[📱 Teléfono]` o `[☁️ Telegram]`).
* **Búsqueda instantánea:** encuentra cualquier canción al instante mientras escribes por título, artista o álbum.

### 2. Streaming de Telegram puro (sin llenar el teléfono)
* **Cero espacio ocupado:** al reproducir una canción de la nube, viaja solo unos segundos de audio en memoria. El teléfono no se llena de archivos temporales ni consume tu almacenamiento.
* **Descarga solo explícita:** cada canción de la nube tiene su botón **«⬇️ Guardar»**. Solo si tú pulsas ese botón, la canción se descarga a tu teléfono para escucharla sin conexión a internet.
* **Subir a la nube:** puedes subir canciones desde tu teléfono a tu canal privado de Telegram con un solo toque.

### 3. Reproductor AVANZADO con configuraciones (Hi-Fi)
Diseñado para la mejor calidad de sonido y personalización total:
* **Control de Volumen:** deslizador fluido del 0% al 100% con porcentaje en tiempo real.
* **Fundido cruzado (Crossfade):** mezcla suave entre canciones consecutivas (de 0.0 a 12.0 segundos) para que la música nunca se corte bruscamente.
* **Ecualizador paramétrico de 3 bandas (EQ):**
  - **Graves (Bass / 250 Hz):** de -12 dB a +12 dB
  - **Medios (Mids / 1000 Hz):** de -12 dB a +12 dB
  - **Agudos (Treble / 4000 Hz):** de -12 dB a +12 dB
  - **Ajustes rápidos de 1 toque (Presets):** *Plano*, *Refuerzo de graves (Bass Boost)*, *Vocal*, *Rock*, *Acústico* y *Electrónica*.
* **Reproducción sin pausas (Gapless):** elimina los silencios molestos entre canciones, ideal para conciertos en directo o álbumes continuos.
* **Selección de dispositivo:** cambia fácilmente la salida de audio entre el altavoz del teléfono, auriculares conectados por cable o auriculares/altavoces Bluetooth.
* **Visualizador en vivo de 32 barras:** analizador de espectro de frecuencias animado en tiempo real según el ritmo de la música.

### 4. Cola de reproducción (Queue)
* Visualiza la pista que está sonando ahora y las siguientes en espera.
* Añade canciones a la cola con el botón **➕**.
* Reordena o quita canciones de la lista, o vacía la cola completa con el botón **«Vaciar cola»**.

---

## 📲 Cómo instalar el APK en tu teléfono (Sideload)

1. Ve a la sección **Releases** de este repositorio en GitHub (o en los artefactos de la pestaña **Actions**).
2. Descarga el archivo **`TitanMusic-v1.0.0.apk`**.
3. En tu teléfono Android, abre el archivo descargado.
4. Si tu teléfono te pide permiso para «Instalar aplicaciones de fuentes desconocidas», pulsa **Permitir**.
5. Pulsa **Instalar** y ¡listo! Ya tienes tu reproductor estilo Spotify instalado.

---

## ☁️ Conexión inicial con Telegram (solo 1 vez)

1. Abre Titan Music y pulsa en la píldora superior **«Nube Telegram»**.
2. Escribe tu número de teléfono (con prefijo internacional, por ejemplo `+34 600 111 222`).
3. Te llegará un código oficial a tu aplicación de Telegram: introdúcelo en Titan Music.
4. Se conectará a tu canal privado «Mi Música» y podrás escuchar toda tu biblioteca en streaming directo sin ocupar espacio en disco.
