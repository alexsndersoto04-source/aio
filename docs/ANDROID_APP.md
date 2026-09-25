# Titan Audio — Reproductor Hi-Fi & Arquitectura Full Stack Profesional 🎵📱

Una plataforma musical de nivel profesional, completa de extremo a extremo:
1. **Backend Multimedia en Tiempo Real:** Servidor de streaming con soporte RFC 7233 de rangos parciales (HTTP 206), biblioteca dinámica indexada y API REST.
2. **Frontend Móvil Ultra-Limpio:** Diseño continuo, minimalista y oscuro sin marcos pesados, flechas desalineadas ni etiquetas técnicas impropias.
3. **Núcleo Android Nativo con MediaStyle:** Integración directa con el sistema operativo mediante `MediaSessionCompat` y `NotificationCompat.MediaStyle`, mostrando controles multimedia táctiles nativos (⏮, ⏯, ⏭) y carátula en alta definición, con icono de estado propio y sin rastro de terceros.

---

## 📥 Descarga Directa del APK Oficial

* **Enlace de Descarga Directa (GitHub Releases):**
  [Descargar TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)
* Compilado de forma automática y reproducible en GitHub Actions.

---

## 🚀 Mejoras de Ingeniería y Diseño Profesional

### 1. Barra de Notificaciones y Pantalla de Bloqueo Nativas (`MediaStyle`)
* **Controles táctiles multimedia integrados:** Se implementó `NotificationCompat.MediaStyle` conectado a `MediaSessionCompat`. En lugar de botones de texto genéricos (`[ Anterior ] [ Reproducir ] [ Siguiente ]`), el sistema operativo Android muestra la tarjeta de reproductor multimedia oficial con botones vectoriales de salto y pausa.
* **Icono de barra de estado exclusivo (`ic_stat_music`):** Icono monocromático vectorial de la onda acústica de Titan Audio, eliminando cualquier icono residual o fallback del sistema.
* **Iconos del lanzador en todas las densidades:** Paquetes PNG dedicados para pantallas mdpi, hdpi, xhdpi, xxhdpi y xxxhdpi con la identidad visual propia de Titan Audio.

### 2. Filtrado Inteligente de Música Real (Cero Archivos Residuales)
* El escáner de memoria de Android (`MediaStore`) ahora aplica un filtro inteligente de duración (`>= 40s`) y excluye automáticamente notas de voz de WhatsApp/Telegram, sintetizadores TTS, ringtones, alarmas y cachés temporales.
* Solo aparecen en tu biblioteca las canciones completas que realmente tienes en el teléfono.

### 3. Rediseño Completo de la Interfaz (Limpia, Alineada y Sin Bordes)
* **Eliminación de elementos desalineados:** Se corrigió la estructura visual para eliminar flechas sueltas debajo de los números de pista.
* **Mini reproductor flotante:**
  - Carátula de 44x44 px a la izquierda.
  - Título y artista completos con ajuste elíptico en el centro.
  - Botones táctiles de reproducción y avance a la derecha.
  - Barra de progreso sutil de 2 px en color cian neón (`#00e5ff`) en el borde superior.
* **Reproductor a pantalla completa:** Al pulsar sobre el mini reproductor, se despliega la vista inmersiva con carátula de alta resolución, control deslizante de tiempo (scrubber) con minutos/segundos y panel de control completo.
* **Navegación limpia:** Segmentos intuitivos (`Mi Teléfono`, `Nube Streaming`, `Telegram`) sin textos técnicos fuera de lugar.

### 4. Estudio Hi-Fi y Procesamiento de Audio
* **Ecualizador paramétrico de 3 bandas:** Ajustes de Graves (60 Hz), Medios (1.0 kHz) y Agudos (10 kHz) de -12 dB a +12 dB.
* **Visualizador de espectro:** 32 barras dinámicas en tiempo real.
* **Crossfade (0 a 12 s):** Transición suave y continua entre canciones consecutivas.
* **Modo Gapless:** Reproducción sin silencios entre pistas.

---

## 📲 Guía Rápida de Instalación

1. Descarga el archivo: **[TitanMusic-v1.0.0.apk](https://github.com/alexsndersoto04-source/aio/releases/download/v1.0.26/TitanMusic-v1.0.0.apk)**.
2. Pulsa en la descarga para instalar o actualizar.
3. Abre **Titan Audio**: tu música aparecerá de inmediato en pantalla.
4. Toca cualquier canción para reproducir y desliza la cortina superior de tu teléfono para ver la barra multimedia nativa de Android.
