// Capturas de la galería del sistema de diseño («Órbita»).
//
// Abre la página con un navegador real (Playwright/Chromium) y guarda
// imágenes PNG: escritorio y móvil, en claro y en oscuro, más algunos
// primeros planos de componentes.
//
// Uso (en el directorio `frontend/`):
//   PAGINA=/ruta/al/diseno.html node scripts/capturas.mjs
//
// Si no se indica PAGINA, usa `moon-diseno.html` de la carpeta del frontend
// (se puede generar con `node scripts/diseno-suelto.mjs`).

import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const raiz = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pagina = process.env.PAGINA || resolve(raiz, 'moon-diseno.html');
const salida = resolve(process.env.SALIDA || 'capturas');
const url = pathToFileURL(pagina).href;

// Nombre de archivo, tamaño de ventana, tema y modo de captura.
const tomas = [
  { archivo: '01-escritorio-claro.png', ancho: 1440, alto: 960, tema: 'light', completa: true },
  { archivo: '02-escritorio-oscuro.png', ancho: 1440, alto: 960, tema: 'dark', completa: true },
  { archivo: '03-movil-claro.png', ancho: 390, alto: 844, tema: 'light', completa: true },
  { archivo: '04-movil-oscuro.png', ancho: 390, alto: 844, tema: 'dark', completa: true },
  { archivo: '05-portada-claro.png', ancho: 1440, alto: 900, tema: 'light' },
  { archivo: '06-portada-oscuro.png', ancho: 1440, alto: 900, tema: 'dark' },
];

// Primeros planos por sección (identificadores de `design-preview.jsx`).
const detalles = [
  { archivo: '10-publicacion.png', seccion: '#sec-publicacion', tema: 'light' },
  { archivo: '11-avisos.png', seccion: '#sec-avisos', tema: 'dark' },
  { archivo: '12-mensajes.png', seccion: '#sec-mensajes', tema: 'light' },
  { archivo: '13-cargas.png', seccion: '#sec-cargas', tema: 'dark' },
  { archivo: '14-admin.png', seccion: '#sec-admin', tema: 'light' },
  { archivo: '15-iconos.png', seccion: '#sec-iconos', tema: 'dark' },
];

mkdirSync(salida, { recursive: true });

const navegador = await chromium.launch();
let total = 0;

async function nuevaPagina(ancho, alto, tema) {
  const contexto = await navegador.newContext({
    viewport: { width: ancho, height: alto },
    deviceScaleFactor: 1,
    colorScheme: tema,
    reducedMotion: 'reduce',
    locale: 'es-ES',
  });
  // El tema se recuerda en `localStorage`; así la página arranca ya pintada.
  await contexto.addInitScript((valor) => {
    try { localStorage.setItem('moon_theme', valor); } catch (e) {}
  }, tema);
  const pagina = await contexto.newPage();
  await pagina.goto(url, { waitUntil: 'load' });
  await pagina.evaluate(async () => { await document.fonts.ready; });
  await pagina.waitForTimeout(700); // margen para animaciones y fuentes
  return { contexto, pagina };
}

for (const t of tomas) {
  const { contexto, pagina } = await nuevaPagina(t.ancho, t.alto, t.tema);
  await pagina.screenshot({ path: resolve(salida, t.archivo), fullPage: !!t.completa });
  await contexto.close();
  total += 1;
  console.log(`ok ${t.archivo}`);
}

for (const d of detalles) {
  const { contexto, pagina } = await nuevaPagina(1440, 1000, d.tema);
  const elemento = pagina.locator(d.seccion);
  await elemento.scrollIntoViewIfNeeded();
  await pagina.waitForTimeout(250);
  await elemento.screenshot({ path: resolve(salida, d.archivo) });
  await contexto.close();
  total += 1;
  console.log(`ok ${d.archivo}`);
}

await navegador.close();
console.log(`${total} capturas en ${salida}`);
