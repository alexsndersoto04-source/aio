// Moon — Capturas de la tanda 4 (perfil completo) para la hoja de propuestas
// Uso:  cp test/visual-tanda4.mjs /tmp/correr-t4.mjs
//       env LD_LIBRARY_PATH=/tmp/al2023/lib node /tmp/correr-t4.mjs
// (Se corre desde /tmp porque ahí viven puppeteer-core y @sparticuz/chromium.)
import puppeteer from 'puppeteer-core';
import chromium from '@sparticuz/chromium';
import fs from 'node:fs';

const BASE = process.env.BASE || 'http://127.0.0.1:3012';
const SALIDA = process.env.SALIDA || '/tmp/t4';
fs.mkdirSync(SALIDA, { recursive: true });

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

const nav = await puppeteer.launch({
  args: [...chromium.args, '--no-sandbox', '--disable-dev-shm-usage'],
  executablePath: await chromium.executablePath(),
  headless: chromium.headless,
});
const p = await nav.newPage();
await p.setViewport({ width: 400, height: 1000, deviceScaleFactor: 2 });
await p.evaluateOnNewDocument(() => {
  localStorage.setItem('moon_access_token', 'demo');
  localStorage.setItem('moon_refresh_token', 'demo');
  const s = document.createElement('style');
  s.textContent = '.demo-banner{display:none !important}';
  document.addEventListener('DOMContentLoaded', () => document.head.appendChild(s));
});

const errores = [];
p.on('pageerror', (e) => errores.push(`pageerror: ${String(e).slice(0, 160)}`));

async function ir(ruta, espera = 2200, recargar = false) {
  await p.goto(`${BASE}/?demo#/${ruta}`, { waitUntil: 'networkidle2', timeout: 60000 });
  // Cambiar solo el hash no vuelve a cargar la página: cuando hace falta un
  // arranque de verdad (el candado del PIN) se recarga a mano.
  if (recargar) await p.reload({ waitUntil: 'domcontentloaded', timeout: 60000 });
  await dormir(espera);
}

/** Pulsa lo primero que contenga ese texto (nada de coordenadas). */
async function pulsar(texto, selector = 'button, a', espera = 1100) {
  const hecho = await p.evaluate((t, sel) => {
    const nodos = [...document.querySelectorAll(sel)];
    const el = nodos.find((n) => (n.textContent || '').toLowerCase().includes(t.toLowerCase()));
    if (!el) return false;
    el.scrollIntoView({ block: 'center' });
    el.click();
    return true;
  }, texto, selector);
  if (!hecho) throw new Error(`No encontré «${texto}»`);
  await dormir(espera);
}

/**
 * Deja un bloque justo debajo de la barra de arriba: busca el contenedor que
 * hace scroll y coloca ahí el elemento, sin depender de coordenadas.
 */
async function ver(selector) {
  // Dos pasadas: al cargar las imágenes la página se estira y el primer ajuste
  // se queda corto.
  for (let i = 0; i < 2; i += 1) {
    await p.evaluate((s) => {
      const el = document.querySelector(s);
      if (!el) return;
      // En esta aplicación el que hace scroll es <body>; la barra de arriba es
      // fija, así que el bloque se deja 64 px más abajo.
      const cuerpo = document.body;
      const scroller = cuerpo.scrollHeight > cuerpo.clientHeight + 4 ? cuerpo : document.scrollingElement;
      scroller.scrollTop += el.getBoundingClientRect().top - scroller.getBoundingClientRect().top - 64;
    }, selector);
    await dormir(700);
  }
}

/** Abre una pestaña del perfil y deja ver su cabecera. */
async function seccion(texto, ancla = '.tabs-perfil') {
  await pulsar(texto, '.tabs button', 1800);
  await ver(ancla);
}

async function foto(nombre) {
  await p.screenshot({ path: `${SALIDA}/${nombre}.png` });
  console.log('  ·', nombre);
}

console.log('Capturas de la tanda 4');

// 1. Mi perfil: cabecera, solicitudes para seguirte y números que se tocan
await ir('profile');
await foto('T1-perfil-solicitudes');

// 2. Aceptar una solicitud (cuenta privada)
await pulsar('Aceptar', '.solicitud button');
await foto('T2-aceptar-solicitud');

// 3. Lista de seguidores, abierta desde el número del perfil
await pulsar('seguidores', '.stat-pulsable');
await ver('.panel-gente');
await foto('T3-seguidores');

// 4. La otra lista: a quién sigo
await pulsar('Siguiendo', '.cambiar-lista button');
await ver('.panel-gente');
await foto('T4-siguiendo');

// 5. Mis me gusta
await seccion('Mis me gusta', '.titulo-pequeno');
await foto('T5-me-gusta');

// 6. Mis comentarios
await seccion('Mis comentarios');
await foto('T6-mis-comentarios');

// 7. Etiquetas que sigo
await seccion('Etiquetas');
await foto('T7-etiquetas');

// 8. Explorar: historial de búsqueda y etiquetas con botón de seguir
await ir('explore');
await foto('T8-explorar-historial');

// 9. Ajustes → Apariencia: idioma, oscuro por horario y ahorro de datos
await ir('settings/apariencia');
// El interruptor no lleva texto: se busca la fila por su título y se pulsa su switch.
await p.evaluate(() => {
  const fila = [...document.querySelectorAll('.fila-ajuste')]
    .find((f) => (f.textContent || '').includes('Oscuro por horario'));
  const sw = fila?.querySelector('.interruptor');
  if (sw) { sw.scrollIntoView({ block: 'center' }); sw.click(); }
});
await dormir(900);
await ver('.horas');
await foto('T9-ajustes-idioma-horario');

// 10. Ajustes → Datos: exportar, historial de búsqueda y etiquetas
await ir('settings/datos');
await ver('.rejilla-uso');
await foto('T10-ajustes-datos');

// 11. Cuenta privada de otra persona: pedir permiso
await ir('user/elena');
await foto('T11-perfil-privado');
await pulsar('Pedir seguir', 'button', 1200);
await foto('T12-solicitud-enviada');

// 12. Ajustes → Seguridad: PIN de entrada puesto (se deja para el final)
await ir('settings/seguridad');
await p.type('.pin-campo', '4821');
await dormir(250);
await pulsar('Poner el PIN', '.pin-form button', 1300);
await foto('T13-ajustes-pin');

// 13. Al volver a abrir Moon, el candado pide el PIN
await ir('feed', 2400, true);
await foto('T14-candado-pin');

// 14. Con el PIN correcto, Moon entra
await p.type('.bloqueo-campo', '4821');
await dormir(1600);
await foto('T15-pin-abierto');

console.log(errores.length ? '\nErrores en la consola:' : '\nSin errores de consola');
for (const e of errores.slice(0, 12)) console.log('  !', e);
await nav.close();
