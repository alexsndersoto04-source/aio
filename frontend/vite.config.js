import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: { '/api': process.env.API_PROXY_TARGET || 'http://localhost:3000' },
    // Permite abrir el servidor de desarrollo a través de hosts proxy
    // (p. ej. el preview de Arena). Solo afecta a `vite dev`, no al build.
    allowedHosts: true,
  },
});
