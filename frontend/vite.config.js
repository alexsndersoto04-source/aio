import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// En desarrollo, si se pide MOON_VISTA=diseno, la raíz abre la galería del
// sistema de diseño en vez de la aplicación. Solo afecta a `vite dev`.
const raizDiseno = {
  name: 'moon-raiz-diseno',
  configureServer(servidor) {
    if (process.env.MOON_VISTA !== 'diseno') return;
    servidor.middlewares.use((req, _res, siguiente) => {
      if (req.url === '/' || req.url.startsWith('/?')) req.url = '/design.html';
      siguiente();
    });
  },
};

export default defineConfig({
  plugins: [react(), raizDiseno],
  server: {
    host: '0.0.0.0',
    port: Number(process.env.PORT || 5173),
    proxy: { '/api': process.env.API_PROXY_TARGET || 'http://localhost:3000' },
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
