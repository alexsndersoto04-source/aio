// Construye un único archivo HTML autocontenido con la galería del sistema
// de diseño de Moon (CSS + JS incrustados, sin servidor ni red).
// Uso:  node scripts/diseno-suelto.mjs [destino.html]
import { build } from 'vite';
import react from '@vitejs/plugin-react';
import { readFileSync, writeFileSync, readdirSync, rmSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

// La raíz del frontend es la carpeta que contiene este script.
const raiz = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const salida = '/tmp/diseno-inline';
const destino = process.argv[2] || resolve(raiz, 'moon-diseno.html');

rmSync(salida, { recursive: true, force: true });

await build({
  root: raiz,
  base: './',
  configFile: false,
  plugins: [react()],
  logLevel: 'warn',
  build: {
    outDir: salida,
    emptyOutDir: true,
    cssCodeSplit: false,
    assetsInlineLimit: 100 * 1024 * 1024,
    modulePreload: { polyfill: false },
    rollupOptions: {
      input: resolve(raiz, 'design.html'),
      output: {
        inlineDynamicImports: true,
        entryFileNames: 'app.js',
        assetFileNames: 'app.[ext]',
      },
    },
  },
});

const archivos = readdirSync(salida);
const html = readFileSync(join(salida, 'design.html'), 'utf8');
const css = readFileSync(join(salida, 'app.css'), 'utf8');
const js = readFileSync(join(salida, 'app.js'), 'utf8');

let final = html
  // Quitar precargas de módulos (ya no hacen falta)
  .replace(/\s*<link rel="modulepreload"[^>]*>/g, '')
  // Incrustar la hoja de estilos
  .replace(/\s*<link rel="stylesheet"[^>]*href="[^"]*app\.css"[^>]*>/g, '')
  .replace('</head>', `<style>\n${css}\n</style>\n</head>`)
  // Incrustar el JavaScript
  .replace(/<script type="module"[^>]*src="[^"]*app\.js"[^>]*><\/script>/g, '')
  .replace('</body>', `<script type="module">\n${js.replace(/<\/script/g, '<\\/script')}\n</script>\n</body>`);

writeFileSync(destino, final, 'utf8');
const kb = (Buffer.byteLength(final) / 1024).toFixed(0);
console.log(`Carpeta temporal: ${archivos.join(', ')}`);
console.log(`Listo: ${destino} (${kb} kB)`);
