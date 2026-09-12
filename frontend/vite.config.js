import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// En desarrollo, MOON_VISTA elige qué se abre en la raíz (solo `vite dev`):
//   diseno → la galería del sistema de diseño
//   app    → la aplicación en modo demostración (datos de ejemplo, sin servidor)
const vistaEnRaiz = {
  name: 'moon-vista-en-raiz',
  configureServer(servidor) {
    const vista = process.env.MOON_VISTA;
    if (vista !== 'diseno' && vista !== 'app') return;
    const destino = vista === 'diseno' ? '/design.html' : '/index.html';
    servidor.middlewares.use((req, _res, siguiente) => {
      if (req.url === '/' || req.url.startsWith('/?')) req.url = destino;
      siguiente();
    });
  },
  // Con MOON_VISTA=app la página activa el modo demostración (datos de ejemplo
  // en el navegador, sin servidor). Solo en desarrollo: al compilar no se toca.
  transformIndexHtml(html) {
    if (process.env.MOON_VISTA !== 'app') return html;
    return html.replace('<head>', '<head>\n    <script>window.MOON_DEMO = true;</script>');
  },
};

export default defineConfig({
  plugins: [react(), vistaEnRaiz],
  server: {
    host: '0.0.0.0',
    port: Number(process.env.PORT || 5173),
    proxy: {
      '/api': process.env.API_PROXY_TARGET || 'http://127.0.0.1:3000',
      // Tiempo real (mensajes y notificaciones en vivo).
      '/ws': { target: process.env.API_PROXY_TARGET || 'http://127.0.0.1:3000', ws: true },
    },
    // Permite abrir el servidor de desarrollo a través de hosts proxy
    // (p. ej. el preview de Arena). Solo afecta a `vite dev`, no al build.
    allowedHosts: true,
  },
  build: {
    // Dos páginas: la aplicación y la galería del sistema de diseño
    // (`/design.html`), que también se comprueba en cada build.
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        design: resolve(__dirname, 'design.html'),
      },
    },
  },
});
