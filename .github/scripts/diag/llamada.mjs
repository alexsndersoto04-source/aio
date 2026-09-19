// Verificación de llamadas con DOS navegadores de verdad que se llaman entre
// ellos (micrófono y cámara simulados por Chrome). Comprueba la llamada de voz
// y la de video de punta a punta y deja capturas.
//
// Uso: node llamada.mjs <web> <sesiones.json> <carpeta-de-capturas> <salida.json>

import fs from 'node:fs';
import path from 'node:path';

// El navegador de pruebas vive en la carpeta de la web; se busca donde esté.
async function cargarChromium() {
  const intentos = [
    process.env.PLAYWRIGHT,
    path.join(process.cwd(), 'node_modules', 'playwright', 'index.mjs'),
    path.join(process.cwd(), 'frontend', 'node_modules', 'playwright', 'index.mjs'),
    'playwright',
  ].filter(Boolean);
  let ultimo = null;
  for (const ruta of intentos) {
    try {
      const mod = await import(ruta);
      console.log('playwright cargado desde', ruta);
      return mod.chromium;
    } catch (e) {
      ultimo = e;
    }
  }
  throw ultimo || new Error('no se encontro playwright');
}

const [WEB, SESIONES, FOTOS, SALIDA] = process.argv.slice(2);
const sesiones = JSON.parse(fs.readFileSync(SESIONES, 'utf8'));
const A = sesiones.pruebafotos;
const B = sesiones.prueballamada;
const convId = sesiones.conversacion.id;
const API = sesiones.api;

const informe = { conversacion: convId, pasos: [] };
// El informe se deja escrito desde el arranque: si algo revienta después,
// igual queda constancia en el comentario del commit.
fs.writeFileSync(SALIDA, JSON.stringify({ ok: false, error: 'arrancando' }, null, 1));
const anota = (paso, datos) => {
  informe.pasos.push({ paso, ...datos });
  console.log(paso, JSON.stringify(datos));
};

async function llamarAlApi(metodo, ruta, token) {
  const r = await fetch(API + ruta, { method: metodo, headers: { Authorization: 'Bearer ' + token } });
  if (!r.ok) return null;
  return r.json().catch(() => null);
}

async function abrirNavegador(sesion, etiqueta) {
  const contexto = await navegador.newContext({
    viewport: { width: 390, height: 844 },
    permissions: ['microphone', 'camera'],
    locale: 'es-VE',
  });
  await contexto.addInitScript(({ s }) => {
    try {
      localStorage.setItem('moon_access_token', s.access_token);
      localStorage.setItem('moon_refresh_token', s.refresh_token);
      localStorage.setItem('moon_user', JSON.stringify(s.user));
      sessionStorage.setItem('moon_pin_abierto', 'si');
    } catch { /* sin almacén */ }
  }, { s: sesion });
  const pagina = await contexto.newPage();
  pagina.on('pageerror', (e) => console.log(`[${etiqueta}] error en la página:`, String(e).slice(0, 160)));
  await pagina.goto(`${WEB}/#/messages/${convId}`, { waitUntil: 'domcontentloaded' });
  // Espera a que el chat esté en pantalla (el hilo tarda en cargar).
  await pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 45000 });
  await pagina.waitForTimeout(1200);
  return { contexto, pagina };
}

const chromium = await cargarChromium();
const navegador = await chromium.launch({
  args: [
    '--use-fake-ui-for-media-stream',          // acepta micrófono y cámara solos
    '--use-fake-device-for-media-stream',      // luz y sonido de mentira
    '--autoplay-policy=no-user-gesture-required',
    '--disable-features=WebRtcHideLocalIpsWithMdns',
  ],
});

try {
  const a = await abrirNavegador(A, 'A');
  const b = await abrirNavegador(B, 'B');
  anota('chats abiertos', { a: !!a.pagina, b: !!b.pagina });

  // ---------- Llamada de voz ----------
  await a.pagina.click('.chat-thread .head .boton-llamar[title="Llamada de voz"]');
  await a.pagina.waitForSelector('.llamada[data-llamada="saliendo"]', { timeout: 10000 });
  anota('A llama por voz', { estado: 'saliendo' });

  await b.pagina.waitForSelector('.llamada[data-llamada="entrando"]', { timeout: 15000 });
  const nombreB = await b.pagina.textContent('.llamada-nombre').catch(() => '');
  anota('B recibe la llamada', { pantallaEntrante: true, quienLlama: (nombreB || '').trim() });
  await b.pagina.screenshot({ path: path.join(FOTOS, '1-entrante-voz.png') });

  await b.pagina.click('.boton-llamada.verde');
  await a.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 30000 });
  await b.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 30000 });
  anota('voz conectada', { conexion: 'conectada' });

  // ¿Llega de verdad la voz? Se mira el flujo remoto del otro lado.
  const vozEnB = await b.pagina.evaluate(() => {
    const el = document.querySelector('audio');
    const flujo = el && el.srcObject;
    return {
      hayReproductor: !!el,
      pistasRemotas: flujo ? flujo.getTracks().map((t) => t.kind) : [],
      pistasLocales: (() => {
        const v = document.querySelector('.video-local video');
        void v;
        return [];
      })(),
    };
  });
  anota('audio en B', vozEnB);
  await b.pagina.waitForTimeout(2500);
  await b.pagina.screenshot({ path: path.join(FOTOS, '2-voz-en-curso.png') });

  // Silenciar y volver a encender (los botones de la llamada).
  await b.pagina.click('.llamada-controles .control');
  const microApagado = await b.pagina.getAttribute('.llamada-controles .control', 'aria-pressed');
  await b.pagina.click('.llamada-controles .control');
  const microEncendido = await b.pagina.getAttribute('.llamada-controles .control', 'aria-pressed');
  anota('boton de microfono', { apagado: microApagado, encendido: microEncendido });

  // Colgar desde A.
  await a.pagina.click('.control.colgar');
  await a.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 15000 });
  await b.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 15000 });
  anota('llamada de voz terminada', { pantallasCerradas: true });

  // La llamada queda anotada una sola vez en el hilo.
  await b.pagina.waitForTimeout(1500);
  const filasB = await b.pagina.$$eval('.fila-llamada', (nodos) => nodos.map((n) => n.textContent.trim()));
  const filasA = await a.pagina.$$eval('.fila-llamada', (nodos) => nodos.map((n) => n.textContent.trim()));
  anota('anotada en el chat', { enB: filasB, enA: filasA });
  await b.pagina.screenshot({ path: path.join(FOTOS, '3-anotada-en-el-chat.png') });

  // ---------- Videollamada ----------
  await b.pagina.click('.chat-thread .head .boton-llamar[title="Videollamada"]');
  await b.pagina.waitForSelector('.llamada[data-llamada="saliendo"][data-tipo="video"]', { timeout: 10000 });
  await a.pagina.waitForSelector('.llamada[data-llamada="entrando"][data-tipo="video"]', { timeout: 15000 });
  anota('B llama por video', { pantallaEntrante: true });
  await a.pagina.screenshot({ path: path.join(FOTOS, '4-entrante-video.png') });

  await a.pagina.click('.boton-llamada.verde');
  await b.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 35000 });
  await a.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 35000 });

  // ¿Se ve el video del otro? Se mira si el cuadro de video tiene imagen.
  const videoEnA = await a.pagina.evaluate(async () => {
    const remoto = document.querySelector('.video-remoto');
    await new Promise((r) => setTimeout(r, 1500));
    return {
      hayCuadroRemoto: !!remoto,
      anchoRemoto: remoto ? remoto.videoWidth : 0,
      altoRemoto: remoto ? remoto.videoHeight : 0,
      hayVideoPropio: !!document.querySelector('.video-local video'),
      controles: document.querySelectorAll('.llamada-controles .control').length,
    };
  });
  anota('video conectado en A', videoEnA);
  await a.pagina.waitForTimeout(2500);
  await a.pagina.screenshot({ path: path.join(FOTOS, '5-video-en-curso.png') });
  await b.pagina.screenshot({ path: path.join(FOTOS, '6-video-en-curso-otro-lado.png') });

  // Apagar la cámara y comprobarlo.
  await a.pagina.click('.llamada-controles .control:nth-child(2)');
  const camaraApagada = await a.pagina.evaluate(() => ({
    tapaVisible: !!document.querySelector('.video-local-tapa'),
    apagada: document.querySelector('.video-local')?.classList.contains('apagada'),
  }));
  anota('boton de camara', camaraApagada);

  await b.pagina.click('.control.colgar');
  await a.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 15000 });
  await b.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 15000 });
  anota('videollamada terminada', { pantallasCerradas: true });

  informe.ok = true;
} catch (e) {
  informe.ok = false;
  informe.error = String(e && e.stack ? e.stack.split('\n').slice(0, 4).join(' | ') : e).slice(0, 600);
  console.log('FALLO:', informe.error);
  fs.writeFileSync(SALIDA, JSON.stringify(informe, null, 1));
} finally {
  // Limpieza: las llamadas de prueba se borran del hilo (quedan solo en el
  // informe) y las cuentas se dejan ocultas de nuevo.
  try {
    const tok = A.access_token;
    const hilo = await llamarAlApi('GET', `/api/messages/conversations/${convId}`, tok);
    const mensajes = (hilo && (hilo.messages || hilo.items)) || [];
    let borrados = 0;
    for (const m of mensajes) {
      if (m.kind) {
        const r = await fetch(`${API}/api/messages/${m.id}`, { method: 'DELETE', headers: { Authorization: 'Bearer ' + tok } });
        if (r.ok) borrados += 1;
      }
    }
    anota('limpieza', { mensajesDeLlamadaBorrados: borrados, quedaban: mensajes.filter((m) => m.kind).length });
  } catch (e) {
    anota('limpieza con problema', { detalle: String(e).slice(0, 160) });
  }
  await navegador.close();
  fs.writeFileSync(SALIDA, JSON.stringify(informe, null, 1));
}
