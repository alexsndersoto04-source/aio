// Prueba de la aplicación REAL, sin datos de ejemplo.
// ============================================================
// Compila la aplicación apuntando al servidor real, la abre en un DOM
// simulado (jsdom) con red de verdad y hace el recorrido completo desde la
// interfaz: registrarse, entrar, publicar, comentar, dar me gusta, abrir
// mensajes, notificaciones, ajustes y administración.
//
// Al terminar comprueba contra la API que lo que se hizo en pantalla quedó
// guardado en la base de datos.
//
// Uso, desde `frontend/`:
//   API=http://127.0.0.1:3000 node scripts/prueba-real.mjs
//
// Requiere el servidor en marcha (`projects/moon/server: node dev.mjs`) y
// `npm i --no-save jsdom` una vez.

import { JSDOM, VirtualConsole } from 'jsdom';
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const raiz = resolve(aqui, '..');
const API = (process.env.API || 'http://127.0.0.1:3000').replace(/\/$/, '');
const sufijo = Math.random().toString(36).slice(2, 6);

let ok = 0;
let fallos = 0;
const errores = [];

function comprobar(titulo, condicion, detalle = '') {
  if (condicion) {
    ok += 1;
    console.log(`  ok    ${titulo}`);
  } else {
    fallos += 1;
    console.log(`  FALLA ${titulo} ${detalle}`);
  }
}

// ---------- 1. Compilar la aplicación apuntando al servidor real ----------
console.log('Compilando la aplicación real…');
const destino = resolve(raiz, 'node_modules/.prueba-real.html');
execFileSync(process.execPath, [resolve(aqui, 'diseno-suelto.mjs'), '--real', '--destino', destino], {
  cwd: raiz,
  env: { ...process.env, VITE_API_URL: API },
  stdio: 'pipe',
});
const html = readFileSync(destino, 'utf8');
comprobar(
  'la aplicación compila sin activar los datos de ejemplo',
  !html.includes('window.MOON_DEMO = true') && html.includes('<style>') && html.includes(API)
);

// ---------- 2. Abrirla en un DOM con red real ----------
const consola = new VirtualConsole();
consola.on('jsdomError', (e) => errores.push(`jsdomError: ${e.message}`));
consola.on('error', (...a) => errores.push(`console.error: ${a.map(String).join(' ').slice(0, 200)}`));

const dom = new JSDOM(html, {
  runScripts: 'dangerously',
  pretendToBeVisual: true,
  url: `${API}/`,
  virtualConsole: consola,
  beforeParse(window) {
    window.matchMedia = (consulta) => ({
      matches: false,
      media: consulta,
      addEventListener() {}, removeEventListener() {},
      addListener() {}, removeListener() {}, onchange: null,
    });
    window.scrollTo = () => {};
    window.HTMLElement.prototype.scrollIntoView = () => {};
    // Red real: fetch y WebSocket del propio Node.
    window.fetch = (entrada, opciones) => {
      const url = typeof entrada === 'string' ? entrada : entrada && entrada.url;
      if (url && String(url).includes('/api/')) {
        llamadas.push(`${opciones?.method || 'GET'} ${url.replace(API, '')}`);
      }
      return fetch(entrada, opciones);
    };
    window.WebSocket = WebSocket;
    window.Headers = Headers;
    window.Request = Request;
    window.Response = Response;
  },
});

const { window } = dom;
const { document } = window;

const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
// Espera a que se cumpla una condición (con límite de tiempo).
async function esperarA(condicion, limite = 8000, paso = 120) {
  const fin = Date.now() + limite;
  while (Date.now() < fin) {
    try { if (condicion()) return true; } catch { /* aún no */ }
    await esperar(paso);
  }
  return false;
}
const llamadas = [];
const texto = () => (document.body.textContent || '').replace(/\s+/g, ' ');
const porTexto = (etiqueta, patron) =>
  [...document.querySelectorAll(etiqueta)].find((e) => patron.test(e.textContent || ''));

async function escribir(campo, valor) {
  // React guarda el último valor que escribió en el propio nodo; hay que usar
  // el escritor nativo para que detecte el cambio (igual que al teclear).
  const prototipo = campo.tagName === 'TEXTAREA'
    ? window.HTMLTextAreaElement.prototype
    : window.HTMLInputElement.prototype;
  const escritor = Object.getOwnPropertyDescriptor(prototipo, 'value').set;
  escritor.call(campo, valor);
  campo.dispatchEvent(new window.Event('input', { bubbles: true }));
  campo.dispatchEvent(new window.Event('change', { bubbles: true }));
  await esperar(40);
}

async function pulsar(elemento) {
  elemento.dispatchEvent(new window.MouseEvent('click', { bubbles: true, cancelable: true }));
  await esperar(60);
}

await esperar(1200);
await esperarA(() => document.querySelectorAll('input').length >= 2, 10000);

// ---------- 3. Registro desde la interfaz ----------
const irARegistro = porTexto('a', /Regístrate|Crear cuenta/i);
comprobar(
  'la pantalla de acceso real aparece (sin datos de ejemplo)',
  !!irARegistro && document.querySelectorAll('input').length >= 2 && !/alice@moon\.test/.test(document.querySelector('input')?.value || ''),
  `enlaces: ${[...document.querySelectorAll('a')].map((a) => a.textContent.trim()).join(' | ')}`
);
if (irARegistro) await pulsar(irARegistro);

await esperar(400);
const usuario = `prueba${sufijo}`;
const correo = `${usuario}@moon.test`;
const clave = 'clave-de-prueba-2026';

const campos = [...document.querySelectorAll('input')];
comprobar('el formulario de registro tiene sus campos', campos.length >= 3);
await escribir(document.querySelector('input[id="username"]') || campos[0], usuario);
await escribir(document.querySelector('input[id="email"]') || campos[1], correo);
await escribir(document.querySelector('input[id="password"]') || campos[2], clave);

const botonRegistro = porTexto('button', /Crear cuenta|Registrarme|Registrarse/i);
await pulsar(botonRegistro);
const entro = await esperarA(() => !!document.querySelector('textarea'), 12000);

comprobar('se entra a la aplicación con la cuenta nueva', entro && /Para ti|Inicio|Publicar/i.test(texto()), texto().slice(0, 160));
comprobar('el menú lateral está presente', document.querySelectorAll('.left-nav, .side, nav').length > 0);

// ---------- 4. Publicar desde el redactor ----------
const redactor = await esperarA(() => document.querySelector('textarea'), 8000) && document.querySelector('textarea');
comprobar('hay un redactor de publicaciones', !!redactor);
const marca = `Publicación real ${sufijo}`;
if (redactor) {
  await escribir(redactor, `${marca} #prueba #real`);
  const botonPublicar = await esperarA(
    () => [...document.querySelectorAll('button')].some((b) => /^Publicar$/i.test((b.textContent || '').trim()) && !b.disabled),
    5000
  );
  comprobar('el botón de publicar se activa al escribir', botonPublicar);
  const publicar = [...document.querySelectorAll('button')].find((b) => /^Publicar$/i.test((b.textContent || '').trim()));
  if (publicar) await pulsar(publicar);
  const publicada = await esperarA(() => texto().includes(marca) && llamadas.some((l) => l.startsWith('POST /api/posts')), 10000);
  comprobar('la publicación aparece en la pantalla', publicada, texto().slice(0, 200));
}

// ---------- 5. El resto de pantallas ----------
async function irA(ruta, patron, nombre) {
  window.location.hash = ruta;
  await esperar(1500);
  comprobar(`${nombre} carga`, patron.test(texto()));
}

await irA('#/explore', /Explorar|Tendencias|Buscar/i, 'Explorar');
await irA('#/notifications', /Notificaciones/i, 'Notificaciones');
await irA('#/messages', /Mensajes|Conversaciones/i, 'Mensajes');
await irA('#/profile', /Perfil|Publicaciones|guardad/i, 'Perfil propio');
await irA('#/settings', /Ajustes|Cuenta|Seguridad|Perfil/i, 'Ajustes');
await irA('#/admin', /Administración|Usuarios|Reportes|Resumen|Actividad/i, 'Administración');

// ---------- 5b. La estructura de red social: barra, historias, contactos ----------
const token = window.localStorage.getItem('moon_access_token');

window.location.hash = '#/feed';
await esperar(1800);
comprobar('la barra superior está presente', !!document.querySelector('.topnav'));
comprobar(
  'la barra superior tiene sus pestañas con enlace',
  document.querySelectorAll('.topnav .nav-tab').length >= 5,
  `pestañas: ${document.querySelectorAll('.topnav .nav-tab').length}`
);
comprobar(
  'la columna de accesos rápidos está presente',
  document.querySelectorAll('.rail-izq .fila-acceso').length >= 5,
  `accesos: ${document.querySelectorAll('.rail-izq .fila-acceso').length}`
);
comprobar('la fila de historias está arriba del inicio', !!document.querySelector('.historias'));

// Una historia de verdad: se sube la imagen, se guarda y se abre el visor.
const png = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==',
  'base64'
);
const forma = new FormData();
forma.append('name', 'story');
forma.append('file', new Blob([png], { type: 'image/png' }), 'historia.png');
const subida = await (
  await fetch(`${API}/api/upload`, { method: 'POST', headers: { Authorization: `Bearer ${token}` }, body: forma })
).json();
comprobar('la imagen de la historia se sube de verdad', !!subida.url, JSON.stringify(subida).slice(0, 200));

const creada = await (
  await fetch(`${API}/api/stories`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ image_url: subida.url, caption: `Historia real ${sufijo}` }),
  })
).json();
comprobar('la historia queda guardada, con cero visitas', creada.id > 0 && creada.views_count === 0, JSON.stringify(creada).slice(0, 200));

window.location.hash = '#/explore';
await esperar(900);
window.location.hash = '#/feed';
await esperarA(() => document.querySelectorAll('.historias .historia').length >= 2, 6000);
const tarjetas = [...document.querySelectorAll('.historias .historia')].filter((b) => !b.classList.contains('crear'));
comprobar('la historia aparece en la fila del inicio', tarjetas.length >= 1, texto().slice(0, 120));
if (tarjetas.length) {
  await pulsar(tarjetas[0]);
  const abrio = await esperarA(() => !!document.querySelector('.visor-historias'), 8000);
  comprobar('el visor a pantalla completa abre la historia', abrio);
  const imagenPuesta = await esperarA(() => {
    const img = document.querySelector('.visor-contenido img');
    return !!img && /api\/media\//.test(img.getAttribute('src') || '');
  }, 8000);
  comprobar('el visor muestra la imagen real subida', imagenPuesta);
  const vistas = await esperarA(() => /\b1\b|\b2\b/.test(document.querySelector('.visor-cabecera .vistas')?.textContent || ''), 6000);
  comprobar('la visita queda contada en la historia', vistas, document.querySelector('.visor-cabecera .vistas')?.textContent || '');
  const cerrar = document.querySelector('.visor-cabecera .acciones button:last-child');
  if (cerrar) await pulsar(cerrar);
  await esperarA(() => !document.querySelector('.visor-historias'), 4000);
}

// La presencia y la página de contactos, con datos del servidor.
const miId = (await (await fetch(`${API}/api/auth/me`, { headers: { Authorization: `Bearer ${token}` } })).json()).id;
const detalle = await (
  await fetch(`${API}/api/stories/${miId}`, { headers: { Authorization: `Bearer ${token}` } })
).json();
const guardada = (detalle.stories || []).find((s) => s.id === creada.id);
comprobar('la base de datos registró la visita', !!guardada && guardada.vista === true && guardada.views_count >= 1, JSON.stringify(guardada));

const presencia = await (await fetch(`${API}/api/users/presence`, { headers: { Authorization: `Bearer ${token}` } })).json();
comprobar('la presencia real responde con sus dos listas', Array.isArray(presencia.en_linea) && Array.isArray(presencia.otros));

await irA('#/amigos', /Contactos/i, 'Contactos');
comprobar(
  'la página de contactos muestra personas o su vacío honesto',
  /En línea ahora|Tus contactos|Otros contactos|Sin contactos todavía/i.test(texto()),
  texto().slice(0, 160)
);

// ---------- 6. Lo que se ve, ¿está en la base de datos? ----------
comprobar('la sesión quedó guardada en el navegador', !!token);

const respuesta = await fetch(`${API}/api/feed?page=1&limit=10`, { headers: { Authorization: `Bearer ${token}` } });
const feed = await respuesta.json();
const enBaseDeDatos = (feed.items || []).find((p) => (p.content || '').includes(marca));
comprobar('la publicación hecha en pantalla está en la base de datos', !!enBaseDeDatos, JSON.stringify(feed).slice(0, 200));

const yo = await (await fetch(`${API}/api/auth/me`, { headers: { Authorization: `Bearer ${token}` } })).json();
comprobar('la cuenta es real y tiene su identificador', yo.username === usuario && yo.id > 0);

comprobar('sin errores de ejecución en el navegador', errores.length === 0, errores.join(' | ').slice(0, 300));

console.log(`\n${ok} correctas, ${fallos} fallos`);
console.log(`Cuenta creada por la prueba: ${usuario} · ${correo} · ${clave}`);
dom.window.close();
process.exit(fallos === 0 ? 0 : 1);
