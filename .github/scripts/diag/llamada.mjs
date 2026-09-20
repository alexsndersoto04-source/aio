// Verificación de llamadas con DOS navegadores de verdad que se llaman entre
// ellos (micrófono y cámara simulados por Chrome). Comprueba la llamada de voz
// y la de video de punta a punta, una videollamada larga con un corte de senal
// en medio, y deja capturas.
//
// Uso: node llamada.mjs <web> <sesiones.json> <carpeta-de-capturas> <salida.json>
//
// Con esta prueba se encontro el motivo de que las llamadas se cortaran solas:
// el reloj del timbre (35 s) seguia armado despues de contestar, y al llegar a
// esa hora el que habia contestado mandaba un «rechazo» de su propia llamada.
// Ya esta apagado al contestar, y el servidor ignora un rechazo en curso.
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

async function versionDeLaApi() {
  try {
    const r = await fetch(`${API}/api/health`);
    const d = await r.json();
    return { commit: d.commit || null, canario: d.canario || null, motor: d.motor || null };
  } catch {
    return null;
  }
}

async function comoVaElSonido(pagina) {
  // Se mira el reproductor de la voz: si está sonando, el navegador va
  // decodificando audio (el contador sube). También se mira la pista remota.
  return pagina.evaluate(() => {
    const audio = document.querySelector('audio') || document.querySelector('.video-remoto');
    const video = document.querySelector('.video-remoto');
    return {
      estado: document.querySelector('.llamada')?.dataset.conexion || '',
      visible: !!document.querySelector('.llamada'),
      hayReproductor: !!audio,
      pausado: audio ? audio.paused : null,
      bytesDeAudio: audio && audio.webkitAudioDecodedByteCount != null ? audio.webkitAudioDecodedByteCount : -1,
      pistaViva: !!(audio && audio.srcObject && audio.srcObject.getAudioTracks
        && audio.srcObject.getAudioTracks()[0] && audio.srcObject.getAudioTracks()[0].readyState === 'live'),
      frames: video && video.getVideoPlaybackQuality ? video.getVideoPlaybackQuality().totalVideoFrames : -1,
    };
  });
}

async function comoVaElVideo(pagina) {
  // Mira la imagen que llega: si hay cuadro y si los cuadros van subiendo
  // (subiendo = la imagen corre; quieto = quedó congelada).
  return pagina.evaluate(() => {
    const v = document.querySelector('.video-remoto');
    if (!v) return { hayVideoRemoto: false, ancho: 0, alto: 0, cuadros: -1, corriendo: false };
    let cuadros = -1;
    try {
      if (v.getVideoPlaybackQuality) cuadros = v.getVideoPlaybackQuality().totalVideoFrames;
      else if (v.webkitDecodedFrameCount != null) cuadros = v.webkitDecodedFrameCount;
    } catch { cuadros = -1; }
    return {
      hayVideoRemoto: true,
      ancho: v.videoWidth || 0,
      alto: v.videoHeight || 0,
      cuadros,
      corriendo: !v.paused && !v.ended,
    };
  });
}

// La imagen corre de verdad solo si hay cuadro y los cuadros suben entre las dos
// medidas (con 3 s de diferencia).
const imagenCorre = (antes, despues) => !!(antes && despues)
  && antes.ancho > 0 && despues.ancho > 0
  && antes.cuadros >= 0 && despues.cuadros > antes.cuadros;

async function nivelDeLaVoz(pagina, milisegundos = 1800) {
  // Se escucha lo que llega (el altavoz de la llamada) y se mide su nivel, igual
  // que poner el oído: si hay voz, el nivel sube; si está mudo, queda en cero.
  return pagina.evaluate(async (ms) => {
    const audio = document.querySelector('audio') || document.querySelector('.video-remoto');
    if (!audio || !audio.srcObject) return { nivel: -1, pistaViva: false, motivo: 'sin reproductor' };
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return { nivel: -1, pistaViva: false, motivo: 'sin audio' };
    let ctx;
    try { ctx = new AC(); } catch { return { nivel: -1, pistaViva: false, motivo: 'sin audio' }; }
    let pico = 0;
    try {
      const fuente = ctx.createMediaStreamSource(audio.srcObject);
      const analizador = ctx.createAnalyser();
      analizador.fftSize = 2048;
      fuente.connect(analizador);
      const datos = new Float32Array(analizador.fftSize);
      const fin = performance.now() + ms;
      while (performance.now() < fin) {
        analizador.getFloatTimeDomainData(datos);
        for (let i = 0; i < datos.length; i += 8) {
          const v = Math.abs(datos[i]);
          if (v > pico) pico = v;
        }
        await new Promise((r) => setTimeout(r, 80));
      }
    } catch (e) {
      return { nivel: -1, pistaViva: false, motivo: String(e).slice(0, 80) };
    } finally {
      try { ctx.close(); } catch { /* ya cerrado */ }
    }
    const pista = audio.srcObject.getAudioTracks && audio.srcObject.getAudioTracks()[0];
    return { nivel: Number(pico.toFixed(4)), pistaViva: !!pista && pista.readyState === 'live' };
  }, milisegundos);
}

// ¿Se oye la voz? Se acepta cualquiera de las dos señales: el nivel medido o
// el contador de audio del navegador (sube mientras decodifica).
const haySonido = (m) => (m && m.nivel > 0.004) || (m && m.bytesDeAudio > 0);

// Mira cómo está la pantalla de la llamada (para saber qué pasó si algo falla).
async function comoEstaLaPantalla(pagina) {
  return pagina.evaluate(() => {
    const caja = document.querySelector('.llamada');
    return {
      hayPantalla: !!caja,
      estado: caja ? caja.dataset.llamada : '',
      conexion: caja ? caja.dataset.conexion : '',
      tipo: caja ? caja.dataset.tipo : '',
      hayAudio: !!document.querySelector('audio'),
      hayVideoRemoto: !!document.querySelector('.video-remoto'),
      texto: (document.body.innerText || '').replace(/\s+/g, ' ').slice(0, 120),
    };
  });
}

async function esperarReproductor(pagina, limiteMs = 30000) {
  // Tras un corte, el reproductor puede tardar en volver: se espera con calma.
  const desde = Date.now();
  while (Date.now() - desde < limiteMs) {
    const hay = await pagina.evaluate(() => !!document.querySelector('audio') || !!document.querySelector('.video-remoto'));
    if (hay) return true;
    await pagina.waitForTimeout(1000);
  }
  return false;
}

async function esperarConectada(pagina, limiteMs = 60000) {
  const desde = Date.now();
  while (Date.now() - desde < limiteMs) {
    const lista = await pagina.evaluate(() => document.querySelector('.llamada')?.dataset.conexion || '');
    if (lista === 'conectada') return true;
    await pagina.waitForTimeout(1000);
  }
  return false;
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
  const marcos = []; // lo que va y viene por el tubo (para poder mirar qué pasó)
  const apuntarMarco = (direccion, carga) => {
    const texto = String(carga || '').slice(0, 160);
    if (!/call_|connected/.test(texto)) return;
    marcos.push({ direccion, texto });
    if (marcos.length > 80) marcos.shift();
  };
  pagina.on('websocket', (ws) => {
    tubos.add(ws);
    console.log(`[${etiqueta}] tubo abierto: ${ws.url().slice(0, 70)}`);
    ws.on('framereceived', (f) => apuntarMarco('entra', f.payload));
    ws.on('framesent', (f) => apuntarMarco('sale', f.payload));
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
  return { contexto, pagina, tubos, seCerraron, marcos };
}

// Se guardan las dos pantallas aquí para poder mirarlas si algo falla.
const pantallas = { a: null, b: null };

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
  pantallas.a = a;
  pantallas.b = b;
  anota('chats abiertos', { a: !!a.pagina, b: !!b.pagina });
  anota('versión de la API', await versionDeLaApi());

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

  // ---------- La pantalla de inicio: el logo ----------
  // Se abre Moon sin sesión, como quien llega por primera vez: se mira que el
  // logo esté (media luna del color de la marca, aro de líneas girando y
  // subrayado) y se deja captura de escritorio y de móvil.
  for (const [nombre, vista] of [
    ['0-inicio-escritorio', { width: 1280, height: 800 }],
    ['0-inicio-movil', { width: 390, height: 844 }],
  ]) {
    const limpio = await navegador.newContext({ viewport: vista, locale: 'es-VE' });
    const pagina = await limpio.newPage();
    await pagina.goto(`${WEB}/#/login`, { waitUntil: 'domcontentloaded' });
    await pagina.waitForSelector('.logo-moon', { timeout: 25000 });
    await pagina.waitForTimeout(1500); // que el aro haya girado un poco
    await pagina.screenshot({ path: path.join(FOTOS, `${nombre}.png`) });
    const comoVa = await pagina.evaluate(() => {
      const svg = document.querySelector('.logo-moon');
      const aro = document.querySelector('.logo-moon .logo-moon__aro');
      const estilo = aro ? getComputedStyle(aro) : null;
      const caja = svg ? svg.getBoundingClientRect() : null;
      return {
        hayLogo: !!svg,
        tamano: caja ? `${Math.round(caja.width)}x${Math.round(caja.height)}` : '',
        colorDeLaLuna: svg ? getComputedStyle(svg).color : '',
        giro: estilo ? `${estilo.animationName} ${estilo.animationDuration}` : '',
        mediaLuna: !!document.querySelector('.logo-moon path, .logo-moon circle[mask]'),
        aroDeLineas: !!document.querySelector('.logo-moon .logo-moon__aro circle, .logo-moon .logo-moon__aro ellipse'),
        subrayado: !!document.querySelector('.logo-moon line'),
        sigueElOrbe: !!document.querySelector('.auth-orb'),
      };
    });
    anota(`pantalla de inicio (${nombre})`, comoVa);
    if (!comoVa.hayLogo || !comoVa.mediaLuna || !comoVa.aroDeLineas || !comoVa.subrayado
        || comoVa.giro.indexOf('logo-girar') !== 0 || comoVa.sigueElOrbe) {
      throw new Error(`el logo de la pantalla de inicio no quedó como debe: ${JSON.stringify(comoVa)}`);
    }
    await limpio.close();
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
  // Se espera a que la pantalla diga que le está sonando el teléfono.
  let textoAviso = '';
  for (let i = 0; i < 25; i += 1) {
    textoAviso = ((await a.pagina.textContent('.llamada-estado').catch(() => '')) || '').trim();
    if (/tel[eé]fono/i.test(textoAviso)) break;
    await a.pagina.waitForTimeout(1000);
  }
  const sigueEnPantalla = await a.pagina.isVisible('.llamada').catch(() => false);
  anota('aviso en la pantalla del que llama', { texto: textoAviso, sigueEnPantalla });
  if (!/tel[eé]fono/i.test(textoAviso)) {
    anota('lo que pasó por el tubo (A)', { marcos: a.marcos.slice(-10) });
    throw new Error(`no avisó de que le está sonando el teléfono (pantalla: ${sigueEnPantalla}): ${textoAviso}`);
  }
  await a.pagina.screenshot({ path: path.join(FOTOS, '7-sonando-en-su-telefono.png') });

  // La llamada tiene que seguir viva: no se cae sola mientras suena el teléfono.
  await a.pagina.waitForTimeout(12000);
  const sigueViva = await a.pagina.isVisible('.llamada').catch(() => false);
  anota('la llamada sigue esperando', { sigueViva });
  if (!sigueViva) throw new Error('la llamada se cerró en vez de esperar a que abriera Moon');

  // Abre Moon: le tiene que timbrar en el momento.
  b = await abrirNavegador(B, 'B2');
  pantallas.b = b;
  await b.pagina.waitForSelector('.llamada[data-llamada="entrando"]', { timeout: 25000 });
  anota('al abrir Moon le timbra', { pantallaEntrante: true });
  await b.pagina.screenshot({ path: path.join(FOTOS, '8-entrante-al-abrir-moon.png') });
  await b.pagina.click('.boton-llamada.verde');
  await a.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 40000 });
  await b.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 40000 });
  anota('voz conectada tras abrir Moon', { conexion: 'conectada' });
  await a.pagina.waitForTimeout(3000); // unos segundos de charla, como en las otras

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

  // ---------- Videollamada larga: que no se corte sola, que la imagen corra y
  // que el sonido no pare. Es el caso más exigente: voz e imagen a la vez.
  await a.pagina.click('.chat-thread .head .boton-llamar[title="Videollamada"]');
  await b.pagina.waitForSelector('.llamada[data-llamada="entrando"][data-tipo="video"]', { timeout: 25000 });
  await b.pagina.click('.boton-llamada.verde');
  await a.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 40000 });
  await b.pagina.waitForSelector('.llamada[data-conexion="conectada"]', { timeout: 40000 });
  anota('videollamada larga empezada', { conexion: 'conectada' });

  await a.pagina.waitForTimeout(6000);
  const sonido1A = await comoVaElSonido(a.pagina);
  const nivel1A = await nivelDeLaVoz(a.pagina);
  const nivel1B = await nivelDeLaVoz(b.pagina);
  anota('como va (1)', {
    enA: sonido1A, enB: await comoVaElSonido(b.pagina),
    nivelA: nivel1A, nivelB: nivel1B,
    videoA: await comoVaElVideo(a.pagina), videoB: await comoVaElVideo(b.pagina),
  });

  await a.pagina.waitForTimeout(10000);
  const sonido2A = await comoVaElSonido(a.pagina);
  const sonido2B = await comoVaElSonido(b.pagina);
  const nivel2A = await nivelDeLaVoz(a.pagina);
  const nivel2B = await nivelDeLaVoz(b.pagina);
  anota('como va (2)', {
    enA: sonido2A, enB: sonido2B, nivelA: nivel2A, nivelB: nivel2B,
    videoA: await comoVaElVideo(a.pagina), videoB: await comoVaElVideo(b.pagina),
  });
  const videoAntesA = await comoVaElVideo(a.pagina);
  const videoAntesB = await comoVaElVideo(b.pagina);
  anota('imagen antes del corte', { enA: videoAntesA, enB: videoAntesB });
  if (!videoAntesA.ancho || !videoAntesB.ancho) {
    throw new Error(`la imagen no llegaba antes del corte: A=${JSON.stringify(videoAntesA)} B=${JSON.stringify(videoAntesB)}`);
  }
  if (!haySonido(nivel1A) && !haySonido(nivel2A)) {
    throw new Error(`no se oye nada en A: ${JSON.stringify(nivel1A)} / ${JSON.stringify(nivel2A)}`);
  }
  if (!haySonido(nivel1B) && !haySonido(nivel2B)) {
    throw new Error(`no se oye nada en B: ${JSON.stringify(nivel1B)} / ${JSON.stringify(nivel2B)}`);
  }

  // Se le quita la señal a B siete segundos (como entrar a un ascensor) y se le
  // devuelve: la llamada NO se puede caer por eso.
  anota('se le quita la señal a B', { segundos: 7 });
  await b.contexto.setOffline(true);
  await a.pagina.waitForTimeout(7000);
  await b.contexto.setOffline(false);
  // Se mira la pantalla cada 2 segundos: así queda claro qué pasa si algo falla.
  for (let i = 0; i < 12; i += 1) {
    anota(`después del corte (${i})`, {
      enA: await comoEstaLaPantalla(a.pagina),
      enB: await comoEstaLaPantalla(b.pagina),
    });
    const listoA = await a.pagina.evaluate(() => document.querySelector('.llamada')?.dataset.conexion || '');
    const listoB = await b.pagina.evaluate(() => document.querySelector('.llamada')?.dataset.conexion || '');
    if (i >= 2 && listoA === 'conectada' && listoB === 'conectada') break;
    await a.pagina.waitForTimeout(2000);
  }

  const volvioA = await esperarConectada(a.pagina, 60000);
  const volvioB = await esperarConectada(b.pagina, 60000);
  anota('tras volver la señal', { enA: volvioA, enB: volvioB });
  if (!volvioA || !volvioB) throw new Error('la llamada no se recuperó después del corte de señal');

  const hayReproductorA = await esperarReproductor(a.pagina, 30000);
  const hayReproductorB = await esperarReproductor(b.pagina, 30000);
  anota('los reproductores tras el corte', { enA: hayReproductorA, enB: hayReproductorB });
  if (!hayReproductorA || !hayReproductorB) {
    anota('como quedaron las pantallas', {
      enA: await comoEstaLaPantalla(a.pagina), enB: await comoEstaLaPantalla(b.pagina),
    });
    anota('lo que dijo el tubo (A)', { marcos: a.marcos.slice(-12) });
    anota('lo que dijo el tubo (B)', { marcos: b.marcos.slice(-12) });
    throw new Error('el reproductor de la llamada no volvió después del corte de señal');
  }

  await a.pagina.waitForTimeout(5000);
  const nivel3A = await nivelDeLaVoz(a.pagina);
  const nivel3B = await nivelDeLaVoz(b.pagina);
  await a.pagina.waitForTimeout(4000);
  const nivel4A = await nivelDeLaVoz(a.pagina);
  const nivel4B = await nivelDeLaVoz(b.pagina);
  anota('como va el sonido (3)', { nivelA: nivel3A, nivelB: nivel3B });
  anota('como va el sonido (4)', { nivelA: nivel4A, nivelB: nivel4B });
  if (!haySonido(nivel4A) && !haySonido(nivel3A)) {
    throw new Error(`tras el corte no se oye nada en A: ${JSON.stringify(nivel3A)} / ${JSON.stringify(nivel4A)}`);
  }
  if (!haySonido(nivel4B) && !haySonido(nivel3B)) {
    throw new Error(`tras el corte no se oye nada en B: ${JSON.stringify(nivel3B)} / ${JSON.stringify(nivel4B)}`);
  }
  // La imagen tambien tiene que haber vuelto: se mira dos veces con 3 s de
  // diferencia y los cuadros tienen que subir (si no, quedaria congelada).
  const imagen1A = await comoVaElVideo(a.pagina);
  const imagen1B = await comoVaElVideo(b.pagina);
  await a.pagina.waitForTimeout(3000);
  const imagen2A = await comoVaElVideo(a.pagina);
  const imagen2B = await comoVaElVideo(b.pagina);
  anota('como va la imagen (1)', { enA: imagen1A, enB: imagen1B });
  anota('como va la imagen (2)', { enA: imagen2A, enB: imagen2B });
  if (!imagenCorre(imagen1A, imagen2A)) {
    throw new Error(`la imagen de A no volvio a correr tras el corte: ${JSON.stringify(imagen1A)} / ${JSON.stringify(imagen2A)}`);
  }
  if (!imagenCorre(imagen1B, imagen2B)) {
    throw new Error(`la imagen de B no volvio a correr tras el corte: ${JSON.stringify(imagen1B)} / ${JSON.stringify(imagen2B)}`);
  }
  await a.pagina.screenshot({ path: path.join(FOTOS, '9-videollamada-larga.png') });

  await a.pagina.click('.control.colgar');
  await a.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 20000 });
  await b.pagina.waitForSelector('.llamada', { state: 'detached', timeout: 20000 });
  await a.pagina.waitForTimeout(2000);
  const cuatroA = await leerFilas(a.pagina, 4);
  const cuatroB = await leerFilas(b.pagina, 4);
  anota('la videollamada larga quedó anotada', { enA: cuatroA, enB: cuatroB });
  if (cuatroA.length !== 4 || cuatroB.length !== 4
      || cuatroA.filter((t) => t.includes('Videollamada')).length !== 2
      || cuatroA.filter((t) => t.includes('Llamada de voz')).length !== 2
      || cuatroB.filter((t) => t.includes('Videollamada')).length !== 2
      || cuatroB.filter((t) => t.includes('Llamada de voz')).length !== 2) {
    throw new Error(`la videollamada larga no quedó anotada igual: A=${JSON.stringify(cuatroA)} B=${JSON.stringify(cuatroB)}`);
  }
  // Y con la duración de verdad (no unos segundos): duró toda la charla.
  const minutos = cuatroA.filter((t) => t.includes('Videollamada'))
    .map((t) => /(\d+):(\d+)/.exec(t)).filter(Boolean)
    .map((m) => Number(m[1]) * 60 + Number(m[2]));
  const masLarga = Math.max(...minutos, 0);
  anota('duración de la videollamada larga', { segundos: masLarga });
  if (masLarga < 30) throw new Error(`la videollamada larga duró solo ${masLarga} s: se cortó sola`);

  informe.ok = true;
} catch (e) {
  informe.ok = false;
  if (pantallas.a) anota('lo que dijo el tubo (A)', { marcos: pantallas.a.marcos.slice(-14) });
  if (pantallas.b) anota('lo que dijo el tubo (B)', { marcos: pantallas.b.marcos.slice(-14) });
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
# touch
# touch
