// Comprueba que una página de un solo archivo se ejecuta y pinta contenido,
// sin navegador (DOM simulado con jsdom). Uso, desde `frontend/`:
//
//   npm i --no-save jsdom
//   node scripts/prueba-render.mjs ../demo/moon.html --entrar --rutas
//   node scripts/prueba-render.mjs ../demo/diseno.html
//
// Opciones: --entrar (pulsa «Entrar» y comprueba que carga la aplicación)
//           --rutas  (recorre todas las pantallas)
import { readFileSync } from 'node:fs';
import { JSDOM, VirtualConsole } from 'jsdom';

const archivo = process.argv[2];
const html = readFileSync(archivo, 'utf8');

const errores = [];
const mensajes = [];
const consola = new VirtualConsole();
consola.on('jsdomError', (e) => errores.push(`jsdomError: ${e.message}\n${(e.stack || '').split('\n').slice(0, 4).join('\n')}`));
consola.on('error', (...a) => errores.push(`console.error: ${a.map(String).join(' ').slice(0, 300)}`));
consola.on('warn', (...a) => mensajes.push(`warn: ${a.map(String).join(' ').slice(0, 200)}`));
consola.on('log', (...a) => mensajes.push(`log: ${a.map(String).join(' ').slice(0, 200)}`));

const dom = new JSDOM(html, {
  runScripts: 'dangerously',
  pretendToBeVisual: true,
  url: 'https://ejemplo.test/moon.html',
  virtualConsole: consola,
  beforeParse(window) {
    // Piezas que jsdom no trae y que el navegador sí.
    window.matchMedia = (consulta) => ({
      matches: /dark/.test(consulta) ? false : false,
      media: consulta,
      addEventListener() {}, removeEventListener() {},
      addListener() {}, removeListener() {}, onchange: null,
    });
    window.scrollTo = () => {};
    window.HTMLElement.prototype.scrollIntoView = () => {};
    window.requestIdleCallback = (fn) => setTimeout(fn, 1);
  },
});

await new Promise((r) => setTimeout(r, 1500));

const { document } = dom.window;
const root = document.getElementById('root');
const texto = (document.body.textContent || '').replace(/\s+/g, ' ').trim();

console.log('=== RESULTADO ===');
console.log('longitud de #root:', root ? root.innerHTML.length : '(no existe #root)');
console.log('texto visible:', texto.length, 'caracteres');
console.log('primeros 300:', texto.slice(0, 300));
console.log('elementos:', document.querySelectorAll('*').length);
console.log('hojas de estilo:', document.styleSheets.length);
console.log('\n=== ERRORES ===');
console.log(errores.length ? errores.join('\n') : '(ninguno)');
if (mensajes.length) {
  console.log('\n=== MENSAJES ===');
  console.log(mensajes.slice(0, 10).join('\n'));
}

// Lista un resumen de lo pintado (etiquetas principales)
if (root) {
  const resumen = [];
  for (const el of root.querySelectorAll('h1,h2,h3,button,a,input,textarea')) {
    const t = (el.textContent || el.placeholder || el.id || '').replace(/\s+/g, ' ').trim().slice(0, 40);
    if (t) resumen.push(`${el.tagName.toLowerCase()}: ${t}`);
  }
  console.log('\n=== CONTROLES VISIBLES (' + resumen.length + ') ===');
  console.log(resumen.slice(0, 40).join('\n'));
}

// ---- Segundo paso: entrar y comprobar que la aplicación carga datos ----
if (process.argv.includes('--entrar')) {
  console.log('\n=== PASO 2: pulsar «Entrar» ===');
  const boton = [...document.querySelectorAll('button')].find((b) => /entrar/i.test(b.textContent));
  if (!boton) {
    console.log('FALLA: no encuentro el botón «Entrar»');
    errores.push('sin botón Entrar');
  } else {
    boton.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true, cancelable: true }));
    await new Promise((r) => setTimeout(r, 2500));
    const t2 = (document.body.textContent || '').replace(/\s+/g, ' ').trim();
    console.log('longitud de #root tras entrar:', root.querySelectorAll('*').length, 'elementos');
    console.log('menú lateral presente:', !!document.querySelector('.left-nav, aside, nav'));
    const titulos = [...document.querySelectorAll('h1,h2,h3')].map((h) => h.textContent.trim()).slice(0, 12);
    console.log('títulos:', JSON.stringify(titulos));
    console.log('publicaciones pintadas:', document.querySelectorAll('.post, .post-card, article').length);
    console.log('texto (400):', t2.slice(0, 400));
    if (!/Moon|Inicio|Para ti|Feed|Publicar/i.test(t2)) {
      console.log('AVISO: el texto no parece la aplicación cargada');
      errores.push('tras entrar no se ve la aplicación');
    }
  }
  console.log('\n=== ERRORES (tras entrar) ===');
  console.log(errores.length ? errores.join('\n') : '(ninguno)');
}

// ---- Tercer paso: recorrer todas las pantallas ----
if (process.argv.includes('--rutas')) {
  console.log('\n=== PASO 3: recorrer las pantallas ===');
  const rutas = [
    ['#/feed', /Para ti|Tendencias|Inicio/],
    ['#/explore', /Explorar|Buscar|Tendencias/],
    ['#/notifications', /Notificaciones|le gustó|empezó a seguirte/],
    ['#/messages', /Mensajes|Conversaciones|escrib/i],
    ['#/profile', /Alice|Perfil|guardad/i],
    ['#/user/carla', /Carla Ríos|@carla|Siguiendo|Seguir/],
    ['#/post/101', /Publicación|Comentarios|Órbita/],
    ['#/settings', /Cuenta|Ajustes|Perfil|Seguridad/],
    ['#/admin', /Administración|Usuarios|Reportes|Resumen/],
  ];
  for (const [ruta, patron] of rutas) {
    dom.window.location.hash = ruta;
    await new Promise((r) => setTimeout(r, 1200));
    const t = (document.body.textContent || '').replace(/\s+/g, ' ');
    const n = document.querySelectorAll('#root *').length;
    const bien = patron.test(t) && n > 60;
    console.log(`${bien ? 'ok   ' : 'FALLA'} ${ruta.padEnd(16)} ${n} elementos | ${t.slice(300, 380).trim()}`);
    if (!bien) errores.push(`ruta ${ruta} no pintó lo esperado`);
  }
  console.log('\n=== ERRORES (recorrido) ===');
  console.log(errores.length ? errores.join('\n') : '(ninguno)');
}

dom.window.close();
process.exit(errores.length ? 1 : 0);
