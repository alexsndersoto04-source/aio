// Verificación de llamadas con DOS navegadores de verdad que se llaman entre
// ellos (micrófono y cámara simulados por Chrome). Comprueba la llamada de voz
// y la de video de punta a punta y deja capturas.
//
// Uso: node llamada.mjs <web> <sesiones.json> <carpeta-de-capturas> <salida.json>
//
// Nota: la web que se prueba se compila en el momento, asi que todo sale tal
// como quedo en el codigo: el timbre (altavoz que se prepara al primer toque y
// se reutiliza en cada llamada) y el repaso del aviso si la conexion parpadea
// justo al empezar a llamar.

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
// Teléfono de mentira, solo para la prueba del aviso (aquí no hay servicio de
// avisos de verdad). Se borra al terminar.
const TELEFONO_FALSO = `https://aviso-de-prueba.moon/${Date.now()}`;

const informe = {
  conversacion: convId,
  quienLlama: { usuario: A?.user?.username, id: A?.user?.id },
  quienRecibe: { usuario: B?.user?.username, id: B?.user?.id },
  direccionWeb: WEB,
  pasos: [],
};
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

async function limpiarRastros(etiqueta) {
  // El servidor no deja borrar lo del otro: cada fila se borra con su token.
  const hilo = await llamarAlApi('GET', `/api/messages/conversations/${convId}`, A.access_token);
  const mensajes = (hilo && (hilo.messages || hilo.items)) || [];
  const rastros = mensajes.filter((m) => m.kind && m.status !== 'deleted');
  let borrados = 0;
  for (const m of rastros) {
    const tok = Number(m.sender_id) === Number(A.user.id) ? A.access_token : B.access_token;
    const r = await fetch(`${API}/api/messages/${m.id}`, { method: 'DELETE', headers: { Authorization: 'Bearer ' + tok } });
    if (r.ok) borrados += 1;
  }
  anota(`limpieza ${etiqueta}`, { rastros: rastros.length, borrados });
  const despues = await llamarAlApi('GET', `/api/messages/conversations/${convId}`, A.access_token);
  const quedan = (((despues && (despues.messages || despues.items)) || []).filter((m) => m.kind && m.status !== 'deleted')).length;
  return { borrados, quedan };
}

async function esperarTubo(quien, etiqueta, limiteMs = 45000) {
  // El tubo (WebSocket) abierto es la señal de que una llamada puede sonar.
  const desde = Date.now();
  while (Date.now() - desde < limiteMs) {
    const abiertos = [...quien.tubos].filter((ws) => !ws.isClosed());
    if (abiertos.length) {
      anota(`${etiqueta} enganchado al tubo`, { tubos: abiertos.length, esperaMs: Date.now() - desde });
      return true;
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  anota(`${etiqueta} no se enganchó al tubo`, { cerrados: quien.seCerraron.slice(-3), esperaMs: limiteMs });
  return false;
}

async function leerFilas(pagina, minimo = 1, limiteMs = 15000) {
  // Las filas pueden tardar un poco en pintarse: se esperan hasta 15 s.
  const desde = Date.now();
  let filas = [];
  while (Date.now() - desde < limiteMs) {
    filas = await pagina.$$eval('.fila-llamada', (nodos) => nodos.map((n) => n.textContent.trim()));
    if (filas.length >= minimo) return filas;
    await pagina.waitForTimeout(1000);
  }
  return filas;
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
  // Se vigilan los tubos del navegador: sin WebSocket abierto, la llamada del
  // otro lado no puede llegar.
  const tubos = new Set();
  const seCerraron = [];
  pagina.on('websocket', (ws) => {
    tubos.add(ws);
    console.log(`[${etiqueta}] tubo abierto: ${ws.url().slice(0, 70)}`);
    ws.on('close', () => {
      tubos.delete(ws);
      seCerraron.push(String(ws.url()).slice(0, 70));
      console.log(`[${etiqueta}] tubo cerrado: ${ws.url().slice(0, 70)}`);
    });
  });
  pagina.on('pageerror', (e) => console.log(`[${etiqueta}] error en la página:`, String(e).slice(0, 160)));
  await pagina.goto(`${WEB}/#/messages/${convId}`, { waitUntil: 'domcontentloaded' });
  // Espera a que el chat esté en pantalla (el hilo tarda en cargar). Si la API
  // viene despertando, la pantalla se queda en blanco: se recarga y se repite.
  let pista = null;
  for (let intento = 1; intento <= 3; intento += 1) {
    try {
      await pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 60000 });
      pista = null;
      break;
    } catch (e) {
      pista = await pagina.evaluate(() => ({
        ruta: location.hash,
        direccion: location.href,
        titulo: document.title,
        texto: (document.body.innerText || '').replace(/\s+/g, ' ').slice(0, 300),
        hayHilo: !!document.querySelector('.chat-thread'),
        hayEntrada: !!document.querySelector('input[type="password"]'),
        conversaciones: document.querySelectorAll('.fila-conv, .chat-list > *').length,
      })).catch(() => null);
      console.log(`[${etiqueta}] intento ${intento} sin hilo:`, JSON.stringify(pista));
      if (intento === 3) break;
      await pagina.reload({ waitUntil: 'domcontentloaded' });
      await pagina.waitForTimeout(2000);
    }
  }
  if (pista) throw new Error(`sin hilo de chat: ${JSON.stringify(pista)}`);
  await pagina.waitForTimeout(1200);
  return { contexto, pagina, tubos, seCerraron };
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
  let b = await abrirNavegador(B, 'B');
  anota('chats abiertos', { a: !!a.pagina, b: !!b.pagina });

  // Nada de pruebas anteriores a la vista: si el hilo trae filas de llamadas
  // viejas, lo que se mire despues no probaria nada.
  const inicio = await limpiarRastros('antes de empezar');
  if (inicio.quedan) throw new Error(`el hilo no quedo limpio: quedan ${inicio.quedan} filas`);
  if (inicio.borrados) {
    // El borrado avisa al otro lado, no a quien borró: se recargan las dos
    // pantallas para que no quede a la vista ninguna fila vieja.
    await a.pagina.reload({ waitUntil: 'domcontentloaded' });
    await b.pagina.reload({ waitUntil: 'domcontentloaded' });
    await a.pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 60000 });
    await b.pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 60000 });
  }

  if (!(await esperarTubo(a, 'A')) || !(await esperarTubo(b, 'B'))) {
    // Un repaso: se recargan las dos pantallas y se vuelve a esperar.
    anota('repaso de las dos pantallas', { intento: 2 });
    await a.pagina.reload({ waitUntil: 'domcontentloaded' });
    await b.pagina.reload({ waitUntil: 'domcontentloaded' });
    await a.pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 45000 });
    await b.pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 45000 });
    if (!(await esperarTubo(a, 'A', 30000)) || !(await esperarTubo(b, 'B', 30000))) {
      throw new Error('los dos navegadores no quedaron enganchados al tubo');
    }
  }

  // ---------- Llamada de voz ----------
  let sonoEnB = false;
  for (let intento = 1; intento <= 3 && !sonoEnB; intento += 1) {
    if (intento > 1) {
      // Se cuelga lo que quedo del intento anterior, se borra del hilo y se
      // recargan las dos pantallas: la fila «perdida» del intento fallido
      // seguia pintada (el borrado solo avisa al otro lado), y el tubo suele
      // quedar recien reconectado.
      await a.pagina.click('.control.colgar').catch(() => {});
      await a.pagina.waitForTimeout(1500);
      await limpiarRastros(`antes del intento ${intento}`);
      await a.pagina.reload({ waitUntil: 'domcontentloaded' });
      await b.pagina.reload({ waitUntil: 'domcontentloaded' });
      await a.pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 60000 });
      await b.pagina.waitForSelector('.chat-thread .head .boton-llamar', { timeout: 60000 });
      await esperarTubo(a, `A (intento ${intento})`, 30000);
      await esperarTubo(b, `B (intento ${intento})`, 30000);
    }
    await a.pagina.click('.chat-thread .head .boton-llamar[title="Llamada de voz"]');
    await a.pagina.waitForSelector('.llamada[data-llamada="saliendo"]', { timeout: 10000 });
    anota(`A llama por voz (intento ${intento})`, { estado: 'saliendo' });
    sonoEnB = await b.pagina.waitForSelector('.llamada[data-llamada="entrando"]', { timeout: 20000 })
      .then(() => true).catch(() => false);
    if (!sonoEnB) {
      anota(`el intento ${intento} no sono en el otro lado`, {
        tubosA: a.tubos.size, tubosB: b.tubos.size,
        cerrados: [...a.seCerraron, ...b.seCerraron].slice(-3),
      });
    }
  }
  if (!sonoEnB) throw new Error('el otro navegador no recibio el aviso de la llamada en 3 intentos');
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
  const filasB = await leerFilas(b.pagina, 1);
  const filasA = await leerFilas(a.pagina, 1);
  anota('anotada en el chat', { enA: filasA, enB: filasB });
  const cuantas = (filas, texto) => filas.filter((t) => t.includes(texto)).length;
  if (filasA.length !== 1 || filasB.length !== 1
      || cuantas(filasA, 'Llamada de voz') !== 1 || cuantas(filasB, 'Llamada de voz') !== 1) {
    throw new Error(`la fila de la llamada salio mal: A=${JSON.stringify(filasA)} B=${JSON.stringify(filasB)}`);
  }
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

  // La fila de la videollamada tambien queda en los dos lados.
  await a.pagina.waitForTimeout(1500);
  const videoA = await leerFilas(a.pagina, 2);
  const videoB = await leerFilas(b.pagina, 2);
  anota('videollamada anotada', { enA: videoA, enB: videoB });
  const cuantasV = (filas, texto) => filas.filter((t) => t.includes(texto)).length;
  if (videoA.length !== 2 || videoB.length !== 2
      || cuantasV(videoA, 'Videollamada') !== 1 || cuantasV(videoB, 'Videollamada') !== 1
      || cuantasV(videoA, 'Llamada de voz') !== 1 || cuantasV(videoB, 'Llamada de voz') !== 1) {
    throw new Error(`las filas de las llamadas salieron mal: A=${JSON.stringify(videoA)} B=${JSON.stringify(videoB)}`);
  }

  // ---------- Llamada con el otro teléfono sin Moon abierto ----------
  // Se le apunta a B un teléfono para el aviso (uno de mentira: aquí no hay
  // servicio de avisos de verdad) y se cierra su pantalla. Es lo que vive una
  // persona cuando le llaman con Moon cerrado.
  await fetch(`${API}/api/push/subscribe`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + B.access_token },
    body: JSON.stringify({
      endpoint: TELEFONO_FALSO,
      keys: { p256dh: 'B'.repeat(40), auth: 'A'.repeat(22) },
      device: 'prueba',
    }),
  }).then((r) => r.status).catch(() => 0);
  const estadoAvisos = await llamarAlApi('GET', '/api/push/estado', B.access_token);
  anota('B apunta un teléfono para el aviso', { telefonos: estadoAvisos?.dispositivos ?? null });

  await b.contexto.close();
  await a.pagina.waitForTimeout(2000); // que el servidor note que se fue

  await a.pagina.click('.chat-thread .head .boton-llamar[title="Llamada de voz"]');
  await a.pagina.waitForSelector('.llamada[data-llamada="saliendo"]', { timeout: 10000 });
  const hayAviso = await a.pagina.waitForSelector('.llamada-error', { timeout: 25000 }).then(() => true).catch(() => false);
  const textoAviso = ((await a.pagina.textContent('.llamada-error').catch(() => '')) || '').trim();
  anota('aviso en la pantalla del que llama', { hayAviso, texto: textoAviso });
  if (!/tel[eé]fono/i.test(textoAviso)) {
    throw new Error(`no avisó de que le está sonando el teléfono: ${textoAviso}`);
  }
  await a.pagina.screenshot({ path: path.join(FOTOS, '7-sonando-en-su-telefono.png') });

  // La llamada tiene que seguir viva: no se cae sola mientras suena el teléfono.
  await a.pagina.waitForTimeout(12000);
  const sigueViva = await a.pagina.isVisible('.llamada').catch(() => false);
  anota('la llamada sigue esperando', { sigueViva });
  if (!sigueViva) throw new Error('la llamada se cerró en vez de esperar a que abriera Moon');

  // Abre Moon: le tiene que timbrar en el momento.
  b = await abrirNavegador(B, 'B2');
  await b.pagina.waitForSelector('.llamada[data-llamada="entrando"]', { timeout: 25000 });
  anota('al abrir Moon le timbra', { pantallaEntrante: true });
  await b.pagina.screenshot({ path: path.join(FOTOS, '8-entrante-al-abrir-moon.png') });
  await b.pagina.click('.boton-llamada.verde');
  await a.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 40000 });
  await b.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 40000 });
  anota('voz conectada tras abrir Moon', { conexion: 'conectada' });

  await a.pagina.click('.control.colgar');
  await a.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 15000 });
  await b.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 15000 });
  await a.pagina.waitForTimeout(2000);
  const tresA = await leerFilas(a.pagina, 3);
  const tresB = await leerFilas(b.pagina, 3);
  const cuantas3 = (filas, texto) => filas.filter((t) => t.includes(texto)).length;
  anota('las tres llamadas anotadas', { enA: tresA, enB: tresB });
  if (tresA.length !== 3 || tresB.length !== 3
      || cuantas3(tresA, 'Llamada de voz') !== 2 || cuantas3(tresB, 'Llamada de voz') !== 2
      || cuantas3(tresA, 'Videollamada') !== 1 || cuantas3(tresB, 'Videollamada') !== 1) {
    throw new Error(`las tres llamadas no quedaron anotadas igual: A=${JSON.stringify(tresA)} B=${JSON.stringify(tresB)}`);
  }

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
    const fin = await limpiarRastros('al terminar');
    if (fin.quedan > 0) {
      informe.ok = false;
      informe.error = `quedaron ${fin.quedan} filas de llamada a la vista en el hilo de prueba`;
    }
  } catch (e) {
    anota('limpieza con problema', { detalle: String(e).slice(0, 160) });
  }
  // El teléfono de mentira se quita: ni rastro de la prueba.
  try {
    const r = await fetch(`${API}/api/push/subscribe`, {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + B.access_token },
      body: JSON.stringify({ endpoint: TELEFONO_FALSO }),
    });
    anota('teléfono de prueba quitado', { ok: r.ok });
  } catch (e) {
    anota('no se pudo quitar el teléfono de prueba', { detalle: String(e).slice(0, 120) });
  }
  await navegador.close();
  fs.writeFileSync(SALIDA, JSON.stringify(informe, null, 1));
}
