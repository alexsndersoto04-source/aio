// Construye un único archivo HTML autocontenido (CSS y JavaScript
// incrustados), sin servidor ni red. Dos usos, desde `frontend/`:
//
//   node scripts/diseno-suelto.mjs            → galería del sistema de diseño
//   node scripts/diseno-suelto.mjs --app      → aplicación en modo demostración
//
// Opciones:
//   --destino <archivo>   ruta de salida (por defecto ./moon-diseno.html
//                         o ./moon-demo.html según el modo)
//   --app                 usa la aplicación (index.html) con datos de ejemplo

import { build } from 'vite';
import react from '@vitejs/plugin-react';
import { readFileSync, writeFileSync, readdirSync, rmSync, mkdirSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

// La raíz del frontend es la carpeta que contiene este script.
const raiz = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const argumentos = process.argv.slice(2);
const modoApp = argumentos.includes('--app');
const indiceDestino = argumentos.indexOf('--destino');
const destino = resolve(
  indiceDestino >= 0 && argumentos[indiceDestino + 1]
    ? argumentos[indiceDestino + 1]
    : join(raiz, modoApp ? 'moon-demo.html' : 'moon-diseno.html')
);

const entrada = modoApp ? 'index.html' : 'design.html';
const salida = join(raiz, 'node_modules/.tmp-pagina-suelta');
mkdirSync(salida, { recursive: true });
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
      input: resolve(raiz, entrada),
      output: {
        format: 'iife',
        inlineDynamicImports: true,
        entryFileNames: 'app.js',
        assetFileNames: 'app.[ext]',
      },
    },
  },
});

const html = readFileSync(join(salida, entrada), 'utf8');
const css = readFileSync(join(salida, 'app.css'), 'utf8');
const js = readFileSync(join(salida, 'app.js'), 'utf8');
const archivos = readdirSync(salida);

// OJO: las sustituciones que insertan contenido usan una FUNCIÓN, no una
// cadena. Con una cadena, `String.replace` interpreta los `$&`, `$\``, `$'` y
// `$$` que aparecen en el código comprimido (¡los nombres de variable `$` son
// habituales!) e inserta texto donde no toca, rompiendo el programa.
let final = html
  // Quitar precargas de módulos (ya no hacen falta)
  .replace(/\s*<link rel="modulepreload"[^>]*>/g, '')
  // Incrustar la hoja de estilos
  .replace(/\s*<link rel="stylesheet"[^>]*href="[^"]*app\.css"[^>]*>/g, '')
  .replace('</head>', () => `<style>\n${css}\n</style>\n</head>`)
  // Incrustar el JavaScript
  .replace(/<script type="module"[^>]*src="[^"]*app\.js"[^>]*><\/script>/g, '')
  .replace(
    '</body>',
    () =>
      (modoApp ? '<script>window.MOON_DEMO = true;</script>\n' : '') +
      // Script clásico: se ejecuta al abrir el archivo con doble clic, sin
      // depender de módulos ni de CORS.
      `<script>\n${js.replace(/<\/script/g, '<\\/script')}\n</script>\n</body>`
  );

// Comprobación de seguridad: el HTML final debe conservar el programa entero.
const sinCierreSuelto = final.split('</body>').length - 1;
if (sinCierreSuelto !== 1) {
  throw new Error(
    `El HTML final tiene ${sinCierreSuelto} etiquetas </body>: el código quedó corrupto.`
  );
}

writeFileSync(destino, final, 'utf8');
const kb = (Buffer.byteLength(final) / 1024).toFixed(0);
console.log(`${modoApp ? 'Aplicación en modo demostración' : 'Galería del sistema de diseño'}`);
console.log(`Piezas compiladas: ${archivos.join(', ')}`);
console.log(`Listo: ${destino} (${kb} kB)`);
