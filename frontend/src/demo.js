// Moon — Modo demostración
// ============================================================
// Permite abrir la aplicación sin servidor: intercepta las llamadas
// `/api/...` y las responde con datos de ejemplo que viven únicamente en la
// memoria del navegador. Nada se guarda, nada sale del dispositivo.
//
// Se activa de tres formas:
//   · `window.MOON_DEMO = true`  (lo pone la página de demostración)
//   · `?demo` en la dirección
//   · `VITE_DEMO=1` al compilar
//
// En la aplicación real nunca se activa: `instalarDemo()` sale enseguida.

const USUARIO = {
  id: 1,
  username: 'alice',
  display_name: 'Alice Márquez',
  email: 'alice@moon.test',
  role: 'admin',
  avatar_url: '',
  cover_url: '',
  is_verified: true,
  is_private: false,
  bio: 'Diseño interfaces y colecciono atardeceres. Aquí comparto lo que aprendo.',
  location: 'Maracaibo, Venezuela',
  created_at: '2024-03-11T10:00:00.000Z',
  followers_count: 1284,
  following_count: 312,
  posts_count: 214,
  is_following: false,
  is_blocked: false,
  is_private: false,
  dm_privacy: 'all',
  who_can_comment: 'all',
  show_online: true,
  searchable: true,
  who_can_see_follows: true,
};

export function esDemo() {
  if (typeof window === 'undefined') return false;
  if (window.MOON_DEMO === true) return true;
  try {
    if (new URLSearchParams(window.location.search).has('demo')) return true;
  } catch { /* sin parámetros */ }
  try {
    if (import.meta.env && import.meta.env.VITE_DEMO === '1') return true;
  } catch { /* sin entorno */ }
  return false;
}

// ---------- Utilidades ----------

const SVG = (contenido) =>
  `data:image/svg+xml;charset=utf-8,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 400">${contenido}</svg>`
  )}`;

// Imagen de ejemplo: degradado + formas suaves (se genera en el navegador).
function foto(tono1, tono2, semilla = 1) {
  const circulos = [
    `<circle cx="${120 + semilla * 40}" cy="90" r="70" fill="rgba(255,255,255,.18)"/>`,
    `<circle cx="${470 - semilla * 30}" cy="300" r="110" fill="rgba(255,255,255,.12)"/>`,
    `<circle cx="${360 + semilla * 20}" cy="120" r="46" fill="rgba(255,255,255,.22)"/>`,
  ].join('');
  return SVG(
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
      `<stop offset="0" stop-color="${tono1}"/><stop offset="1" stop-color="${tono2}"/>` +
      `</linearGradient></defs>` +
      `<rect width="640" height="400" fill="url(#g)"/>${circulos}`
  );
}

function retrato(iniciales, tono1, tono2) {
  return SVG(
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
      `<stop offset="0" stop-color="${tono1}"/><stop offset="1" stop-color="${tono2}"/>` +
      `</linearGradient></defs>` +
      `<rect width="640" height="400" fill="url(#g)"/>` +
      `<text x="50%" y="50%" dy=".35em" text-anchor="middle" fill="#fff" ` +
      `font-family="Inter,Segoe UI,sans-serif" font-size="170" font-weight="700">${iniciales}</text>`
  );
}

const AHORA = Date.now();
const hace = (minutos) => new Date(AHORA - minutos * 60000).toISOString();

// ---------- Datos de ejemplo ----------

const personas = [
  { id: 2, username: 'bruno', display_name: 'Bruno Salas', is_verified: false, iniciales: 'BS', color: ['#4f46e5', '#06b6d4'], bio: 'Fotógrafo urbano. Luz natural y aceras mojadas.', location: 'Caracas', followers_count: 842, following_count: 190, posts_count: 88 },
  { id: 3, username: 'carla', display_name: 'Carla Ríos', is_verified: true, iniciales: 'CR', color: ['#8b5cf6', '#ec4899'], bio: 'Ingeniera de datos. Escribo sobre rendimiento y equipos pequeños.', location: 'Bogotá', followers_count: 5321, following_count: 401, posts_count: 356 },
  { id: 4, username: 'diego', display_name: 'Diego Peña', is_verified: false, iniciales: 'DP', color: ['#0ea5e9', '#22c55e'], bio: 'Cocino y programo, en ese orden.', location: 'Maracaibo', followers_count: 233, following_count: 512, posts_count: 41 },
  { id: 5, username: 'elena', display_name: 'Elena Duarte', is_verified: true, iniciales: 'ED', color: ['#f59e0b', '#ef4444'], bio: 'Periodista. Historias de barrio y datos abiertos.', location: 'Lima', followers_count: 9124, following_count: 288, posts_count: 611 },
];

const perfiles = new Map();
for (const p of personas) {
  perfiles.set(p.id, {
    id: p.id,
    username: p.username,
    display_name: p.display_name,
    avatar_url: retrato(p.iniciales, p.color[0], p.color[1]),
    cover_url: '',
    bio: p.bio,
    location: p.location,
    created_at: '2024-06-02T12:00:00.000Z',
    is_verified: p.is_verified,
    is_private: false,
    is_following: p.id === 2 || p.id === 3,
    is_blocked: false,
    followers_count: p.followers_count,
    following_count: p.following_count,
    posts_count: p.posts_count,
  });
}
perfiles.set(1, {
  ...USUARIO,
  avatar_url: retrato('AM', '#4f46e5', '#8b5cf6'),
});

const publicaciones = [
  { id: 101, autor: 1, creada: 22, likes: 128, comentarios: 14, guardados: 9, meGusta: true, guardada: true, texto: 'Terminé el rediseño de Moon con el sistema «Órbita». Menos ruido, más foco y por fin un modo oscuro de verdad. ¿Qué os parece? #diseño #moon', imagenes: [foto('#4f46e5', '#06b6d4', 1)] },
  { id: 102, autor: 3, creada: 55, likes: 342, comentarios: 28, guardados: 40, meGusta: false, guardada: false, texto: 'Consejo del día: antes de optimizar, mide. Cambiamos una consulta y pasamos de 900 ms a 40 ms. El 80 % del trabajo estaba en índices mal puestos. #rendimiento', imagenes: [] },
  { id: 103, autor: 2, creada: 130, likes: 89, comentarios: 6, guardados: 12, meGusta: false, guardada: false, texto: 'Amanecer en la avenida. La ciudad se ve distinta cuando nadie la mira. #fotografía #ciudad', imagenes: [foto('#0ea5e9', '#8b5cf6', 2), foto('#f59e0b', '#ef4444', 3)] },
  { id: 108, autor: 4, creada: 90, likes: 62, comentarios: 11, guardados: 4, meGusta: false, guardada: false, texto: 'Necesito ayuda con una decisión importante para el grupo. ¿Qué hacemos el sábado? #cocina #encuesta', imagenes: [], encuesta: { pregunta: '¿Qué preparamos el sábado?', opciones: ['Pizza casera', 'Sushi', 'Asado', 'Lo que diga el grupo'], multiple: false, votos: [3, 1, 2, 4], miVoto: [] } },
  { id: 104, autor: 5, creada: 260, likes: 512, comentarios: 63, guardados: 88, meGusta: true, guardada: false, texto: 'Tres meses visitando el mercado de San Juan. Esto es lo que aprendí sobre contar historias con respeto: pregunta, escucha y no robes el protagonismo. #periodismo #datos', imagenes: [foto('#22c55e', '#0ea5e9', 4)] },
  { id: 105, autor: 4, creada: 400, likes: 41, comentarios: 3, guardados: 2, meGusta: false, guardada: false, texto: 'Pan de masa madre: 12 horas de espera para 900 g de felicidad. #cocina', imagenes: [] },
  { id: 106, autor: 1, creada: 640, likes: 76, comentarios: 9, guardados: 15, meGusta: false, guardada: false, texto: 'Recordatorio: los esqueletos de carga hacen que la aplicación se sienta más rápida aunque tarde lo mismo. La gente percibe el tiempo, no los milisegundos. #diseño #producto', imagenes: [] },
  { id: 107, autor: 3, creada: 1500, likes: 233, comentarios: 19, guardados: 52, meGusta: false, guardada: true, texto: 'Los equipos pequeños no ganan por trabajar más horas, ganan por tener menos cosas a la vez. Enfoque secuencial, entregas cortas. #equipos', imagenes: [] },
];

function publicacionJSON(p) {
  const autor = perfiles.get(p.autor);
  return {
    id: p.id,
    content: p.texto,
    created_at: hace(p.creada),
    edited_at: p.editada ? hace(Math.max(1, p.creada - 5)) : null,
    deleted: !!p.borrada,
    images: (p.imagenes || []).map((url, i) => ({ id: `${p.id}-${i}`, url, original_url: url })),
    likes_count: p.likes,
    comments_count: p.comentarios,
    saves_count: p.guardados,
    is_liked: !!p.meGusta,
    is_saved: !!p.guardada,
    is_mine: p.autor === USUARIO.id,
    author_username: autor.username,
    author_display_name: autor.display_name,
    author_avatar_url: autor.avatar_url,
    author_is_verified: autor.is_verified,
    author_is_private: !!autor.is_private,
    poll: p.encuesta ? encuestaJSON(p) : null,
  };
}

function encuestaJSON(p) {
  const e = p.encuesta;
  const votos = e.votos || [];
  const total = votos.reduce((n, v) => n + v, 0);
  return {
    pregunta: e.pregunta,
    multiple: !!e.multiple,
    cerrada: !!e.cerrada,
    total,
    mi_voto: e.miVoto || [],
    opciones: e.opciones.map((texto, i) => ({
      texto,
      votos: votos[i] || 0,
      porcentaje: total > 0 ? Math.round(((votos[i] || 0) / total) * 100) : 0,
    })),
  };
}

const comentarios = {
  101: [
    { id: 1, autor: 2, texto: 'El modo oscuro quedó impecable. ¿Lo probaste en móvil?', creada: 18 },
    { id: 2, autor: 3, texto: 'Se nota el trabajo en los detalles: los avisos, los esqueletos… 👏', creada: 12 },
    { id: 3, autor: 5, texto: 'Me gusta que el acento sea uno solo. Menos ruido visual.', creada: 4 },
  ],
  102: [
    { id: 4, autor: 4, texto: '¿Qué índice usaron al final?', creada: 30 },
  ],
};

const notificaciones = [
  { id: 11, tipo: 'like', de: 3, post: 101, creada: 6, leida: false, texto: 'le gustó tu publicación' },
  { id: 12, tipo: 'comment', de: 2, post: 101, creada: 18, leida: false, texto: 'comentó: «El modo oscuro quedó impecable»' },
  { id: 13, tipo: 'follow', de: 5, creada: 120, leida: false, texto: 'empezó a seguirte' },
  { id: 14, tipo: 'mention', de: 3, post: 107, creada: 700, leida: true, texto: 'te mencionó en una publicación' },
  { id: 15, tipo: 'like', de: 4, post: 106, creada: 1400, leida: true, texto: 'le gustó tu publicación' },
];

const conversaciones = [
  {
    id: 1,
    partnerId: 3,
    noLeidos: 2,
    mensajes: [
      { id: 1, de: 3, texto: '¿Ya subiste la versión con el nuevo diseño?', creada: 90, estado: 'read' },
      { id: 2, de: 1, texto: 'Sí, acabo de terminarlo. Se siente mucho más limpio y ordenado.', creada: 86, estado: 'read' },
      { id: 3, de: 3, texto: 'Se nota muchísimo en el modo oscuro.', creada: 60, estado: 'delivered' },
      { id: 4, de: 3, texto: 'Cuando quieras lo reviso con calma y te dejo notas.', creada: 12, estado: 'delivered' },
      { id: 5, de: 3, texto: '', audio: 'VOZ', duracion: 2600, creada: 4, estado: 'delivered' },
    ],
  },
  {
    id: 2,
    partnerId: 2,
    noLeidos: 0,
    mensajes: [
      { id: 5, de: 2, texto: 'Te paso las fotos del amanecer para el proyecto.', creada: 700, estado: 'read' },
      { id: 6, de: 1, texto: 'Perfecto, mándalas cuando puedas 👍', creada: 690, estado: 'read' },
    ],
  },
  {
    id: 3,
    partnerId: 4,
    noLeidos: 0,
    mensajes: [
      { id: 7, de: 4, texto: '¿El viernes te apuntas a cocinar?', creada: 2600, estado: 'read' },
    ],
  },
];

const palabrasProhibidas = ['spam', 'estafa', 'insulto'];

// Preferencias de avisos del modo demostración (solo en memoria).
const avisos = { follow: true, like: true, comment: true, reply: true, mention: true, message: true, system: true };

// ---------- Respuestas ----------

const json = (data, status = 200) => ({ status, data });

// Respuesta con la forma que espera `api.js` (ok, status, json, text).
function respuesta(data, status) {
  const texto = JSON.stringify(data === undefined ? null : data);
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => JSON.parse(texto),
    text: async () => texto,
  };
}

function paginado(items, ruta, extra = {}) {
  const url = new URL(ruta, 'http://demo.local');
  const page = Number(url.searchParams.get('page') || 1);
  const limit = Number(url.searchParams.get('limit') || 10);
  return {
    items: items.slice((page - 1) * limit, page * limit),
    total: items.length,
    page,
    limit,
    ...extra,
  };
}

// Sockets vivos de la aplicación: por aquí se empujan los eventos simulados.
const socketsAbiertos = new Set();

export function emitirDemo(ev) {
  for (const socket of [...socketsAbiertos]) {
    try { socket.recibir(ev); } catch { /* socket cerrado */ }
  }
}

function mensajeJSON(c, m) {
  return {
    id: m.id,
    conversation_id: c.id,
    sender_id: m.de,
    content: m.texto,
    image_url: m.imagen || '',
    audio_url: m.audio === 'VOZ' ? vozDeEjemplo((m.duracion || 2600) / 1000) : (m.audio || ''),
    duracion_ms: m.duracion || 0,
    created_at: hace(m.creada),
    status: m.estado || 'sent',
    reaction: m.reaccion || null,
    edited_at: null,
  };
}

/**
 * Nota de voz de ejemplo: se genera aquí mismo, sin red.
 * Un navegador puede reproducir WAV sin más, así que se arma la cabecera a
 * mano y se rellena con una onda que sube y baja como una voz hablando.
 */
let vozDemo = null;
function vozDeEjemplo(segundos = 3.4) {
  if (vozDemo) return vozDemo;
  const Hz = 8000;
  const n = Math.max(1, Math.floor(Hz * segundos));
  const datos = new DataView(new ArrayBuffer(44 + n));
  const txt = (pos, t) => { for (let i = 0; i < t.length; i++) datos.setUint8(pos + i, t.charCodeAt(i)); };
  txt(0, 'RIFF'); datos.setUint32(4, 36 + n, true); txt(8, 'WAVE');
  txt(12, 'fmt '); datos.setUint32(16, 16, true); datos.setUint16(20, 1, true);
  datos.setUint16(22, 1, true); datos.setUint32(24, Hz, true);
  datos.setUint32(28, Hz, true); datos.setUint16(32, 1, true); datos.setUint16(34, 8, true);
  txt(36, 'data'); datos.setUint32(40, n, true);
  for (let i = 0; i < n; i++) {
    const t = i / Hz;
    const envolvente = 0.35 + 0.65 * Math.abs(Math.sin(2 * Math.PI * (t / 1.05)));
    const tono = 150 + 55 * Math.sin(2 * Math.PI * 0.9 * t) + 30 * Math.sin(2 * Math.PI * 3.1 * t);
    datos.setUint8(44 + i, 128 + Math.round(58 * envolvente * Math.sin(2 * Math.PI * tono * t)));
  }
  try {
    vozDemo = URL.createObjectURL(new Blob([datos.buffer], { type: 'audio/wav' }));
  } catch {
    vozDemo = '';
  }
  return vozDemo;
}

// ---------- Datos del grupo de ejemplo ----------
const gruposDemo = [];

const grupoDemo = {
  id: 7,
  name: 'Fotografía nocturna',
  about: 'Salimos a fotografiar la ciudad cuando se apagan las luces. Comparte aquí tus tomas y tus ajustes.',
  privacy: 'public',
  miembros: 4,
  owner_id: 3,
  soy_miembro: true,
  es_mio: false,
  created_at: hace(14000),
};

const chatDelGrupo = [
  { id: 701, de: 3, texto: '¿Alguien sale esta noche? La luna está enorme y quiero probar el 35 mm.', creada: 260 },
  { id: 702, de: 2, texto: 'Yo me apunto. Llevo el trípode y el filtro de densidad.', creada: 240 },
  { id: 703, de: 4, texto: '', audio: 'VOZ', duracion: 3400, creada: 180 },
  { id: 704, de: 3, texto: 'Escuchado, salimos a las 8 desde la plaza.', creada: 170 },
];

// El grupo de ejemplo y dos para descubrir, en una lista que se puede tocar
// (renombrar, cambiar de privacidad o eliminar desde la demostración).
gruposDemo.push(grupoDemo);
gruposDemo.push({ ...grupoDemo, id: 8, name: 'Cocina de barrio', about: 'Recetas de todos los días, sin prisa.', miembros: 12, soy_miembro: false, es_mio: false });
gruposDemo.push({ ...grupoDemo, id: 9, name: 'Rendimiento web', about: 'Medir antes de optimizar.', miembros: 31, privacy: 'private', soy_miembro: false, es_mio: false });

function miembroJSON(id) {
  const p = perfiles.get(id);
  return { id, username: p.username, display_name: p.display_name, avatar_url: p.avatar_url, is_verified: p.is_verified };
}

function mensajeDeGrupoJSON(m) {
  const p = perfiles.get(m.de) || USUARIO;
  return {
    id: m.id,
    group_id: grupoDemo.id,
    user_id: m.de,
    content: m.texto || '',
    audio_url: m.audio === 'VOZ' ? vozDeEjemplo((m.duracion || 3400) / 1000) : (m.audio || ''),
    duracion_ms: m.duracion || 0,
    created_at: hace(m.creada),
    username: p.username,
    display_name: p.display_name,
    avatar_url: p.avatar_url,
  };
}

let siguienteId = 900;

export function responder(metodo, ruta, cuerpo) {
  const [camino, consulta = ''] = ruta.split('?');
  const partes = camino.replace(/^\/api\//, '').split('/').filter(Boolean);
  const q = new URLSearchParams(consulta);
  const [a, b, c, d] = partes;

  // ---- Avisos al teléfono (modo demostración) ----
  if (a === 'push') {
    if (b === 'public-key') return json({ key: 'DEMO-DEMO-DEMO' });
    if (b === 'estado') return json({ dispositivos: 1, ultimo: hace(30) });
    if (b === 'test') return json({ ok: true, enviados: 1 });
    return json({ ok: true, dispositivos: 1 });
  }

  // ---- Copias de seguridad (modo demostración) ----
  if (a === 'admin' && b === 'backup') {
    return json({ app: 'moon', fecha: new Date().toISOString(), resumen: { users: 1, posts: 8 }, tablas: {} });
  }

  // ---- Estado del servidor ----
  if (a === 'health') {
    return json({ status: 'ok', app: 'moon', time: new Date().toISOString(), db: true });
  }

  // ---- Sesión ----
  if (a === 'auth') {
    if (b === 'login' || b === 'register' || b === 'refresh') {
      return json({ access_token: 'demo', refresh_token: 'demo', user: USUARIO });
    }
    if (b === 'logout') return json({ ok: true });
    if (b === 'me') return json(USUARIO);
    if (b === 'update') {
      Object.assign(USUARIO, cuerpo || {});
      return json(USUARIO);
    }
    if (b === 'privacy') {
      Object.assign(USUARIO, {
        is_private: !!cuerpo?.is_private,
        dm_privacy: cuerpo?.dm_privacy || USUARIO.dm_privacy || 'all',
        who_can_comment: cuerpo?.who_can_comment || 'all',
        show_online: cuerpo?.show_online !== false,
        searchable: cuerpo?.searchable !== false,
        who_can_see_follows: cuerpo?.who_can_see_follows !== false,
      });
      return json(USUARIO);
    }
    if (b === 'change-password') return json({ ok: true });
    if (b === 'sessions') {
      if (metodo === 'DELETE') return json({ ok: true });
      return json([
        { id: 's1', user_agent: 'Chrome · Linux', ip: '190.0.0.2', created_at: hace(30), last_used_at: hace(2), current: true },
        { id: 's2', user_agent: 'Safari · iPhone', ip: '190.0.0.9', created_at: hace(900), last_used_at: hace(200), current: false },
      ]);
    }
    if (b === 'sessions-all') return json({ ok: true });
    if (b === 'account') return json({ ok: true });
    if (b === '2fa') {
      if (c === 'enable') return json({ ok: true, secret: 'DEMO-DEMO-DEMO', otpauth_url: 'otpauth://totp/Moon:alice?secret=DEMODEMODEMO&issuer=Moon' });
      return json({ ok: true });
    }
    if (b === 'recovery') return json({ ok: true });
    return json({ ok: true });
  }

  // ---- Publicaciones ----
  if (a === 'feed') {
    const tipo = q.get('tipo') || '';
    const lista = publicaciones.filter((p) => !p.borrada).filter((p) => {
      if (tipo === 'fotos') return (p.imagenes || []).length > 0;
      if (tipo === 'encuestas') return !!p.encuesta;
      if (tipo === 'texto') return (p.imagenes || []).length === 0 && !p.encuesta;
      return true;
    });
    const orden =
      b === 'trending' ? [...lista].sort((x, y) => y.likes - x.likes)
      : b === 'latest' ? [...lista].sort((x, y) => x.creada - y.creada)
      : lista;
    return json(paginado(orden.map(publicacionJSON), ruta));
  }

  if (a === 'posts') {
    if (!b && metodo === 'POST') {
      const nueva = {
        id: ++siguienteId,
        autor: USUARIO.id,
        creada: 0,
        likes: 0,
        comentarios: 0,
        guardados: 0,
        meGusta: false,
        guardada: false,
        texto: cuerpo?.content || '',
        imagenes: (cuerpo?.images || []).map((im) => im.url || im),
        encuesta: cuerpo?.poll && Array.isArray(cuerpo.poll.opciones) && cuerpo.poll.opciones.filter(Boolean).length >= 2
          ? {
            pregunta: String(cuerpo.poll.pregunta || '').slice(0, 160),
            opciones: cuerpo.poll.opciones.filter(Boolean).slice(0, 6),
            multiple: !!cuerpo.poll.multiple,
            votos: cuerpo.poll.opciones.filter(Boolean).map(() => 0),
            miVoto: [],
          }
          : null,
      };
      publicaciones.unshift(nueva);
      USUARIO.posts_count += 1;
      return json(publicacionJSON(nueva));
    }
    const id = Number(b);
    const post = publicaciones.find((p) => p.id === id);
    const accion = c;
    if (!post) return json({ error: 'Publicación no encontrada' }, 404);

    if (!accion) {
      if (metodo === 'DELETE') { post.borrada = true; return json({ ok: true }); }
      if (metodo === 'PATCH') { post.texto = cuerpo?.content ?? post.texto; post.editada = true; }
      return json(publicacionJSON(post));
    }
    if (accion === 'vote' && post.encuesta) {
      const opcion = Number(cuerpo?.opcion);
      const e = post.encuesta;
      if (!Number.isInteger(opcion) || opcion < 0 || opcion >= e.opciones.length) {
        return json({ error: 'Voto inválido' }, 400);
      }
      e.votos = e.votos || e.opciones.map(() => 0);
      e.miVoto = e.miVoto || [];
      if (!e.multiple) {
        for (const anterior of e.miVoto) e.votos[anterior] = Math.max(0, (e.votos[anterior] || 0) - 1);
        e.miVoto = [];
      }
      if (!e.miVoto.includes(opcion)) {
        e.votos[opcion] = (e.votos[opcion] || 0) + 1;
        e.miVoto.push(opcion);
      }
      return json(publicacionJSON(post));
    }
    if (accion === 'likes') {
      const gente = [...perfiles.values()].filter((u) => u.id !== USUARIO.id).slice(0, 3);
      return json({
        total: post.likes,
        items: gente.map((u) => ({
          id: u.id, username: u.username, display_name: u.display_name,
          avatar_url: u.avatar_url, is_verified: u.is_verified,
        })),
      });
    }
    if (accion === 'like') {
      const dando = metodo === 'POST';
      post.meGusta = dando;
      post.likes += dando ? 1 : -1;
      return json(publicacionJSON(post));
    }
    if (accion === 'save') {
      post.guardada = true;
      post.guardados += 1;
      return json(publicacionJSON(post));
    }
    if (accion === 'comments') {
      const lista = comentarios[id] || [];
      if (metodo === 'POST') {
        const autor = USUARIO;
        const nuevo = { id: ++siguienteId, autor: autor.id, texto: cuerpo?.content || '', creada: 0 };
        lista.push(nuevo);
        comentarios[id] = lista;
        post.comentarios += 1;
        return json({
          id: nuevo.id,
          content: nuevo.texto,
          created_at: hace(0),
          username: autor.username,
          display_name: autor.display_name,
          avatar_url: perfiles.get(1).avatar_url,
          is_verified: autor.is_verified,
          is_mine: true,
        });
      }
      return json(
        lista.map((com) => {
          const autor = perfiles.get(com.autor);
          return {
            id: com.id,
            content: com.texto,
            created_at: hace(com.creada),
            username: autor.username,
            display_name: autor.display_name,
            avatar_url: autor.avatar_url,
            is_verified: autor.is_verified,
            is_mine: com.autor === USUARIO.id,
          };
        })
      );
    }
  }

  if (a === 'users' && b === 'presence') {
    const gente = [...perfiles.values()].filter((u) => u.id !== USUARIO.id);
    const en_linea = gente.slice(0, 2).map((u) => ({
      id: u.id, username: u.username, display_name: u.display_name,
      avatar_url: u.avatar_url, is_verified: u.is_verified, online: true,
    }));
    const otros = gente.slice(2).map((u) => ({
      id: u.id, username: u.username, display_name: u.display_name,
      avatar_url: u.avatar_url, is_verified: u.is_verified, online: false,
    }));
    return json({ en_linea, otros, total_en_linea: en_linea.length });
  }

  if (a === 'notifications' && b === 'prefs') {
    if (metodo === 'PATCH' && cuerpo) Object.assign(avisos, cuerpo);
    return json(avisos);
  }

  if (a === 'me' && b === 'stats') {
    return json({
      posts: publicaciones.filter((x) => x.autor === USUARIO.id && !x.borrada).length,
      comentarios: 24,
      guardados: publicaciones.filter((x) => x.guardada).length,
      me_gusta_recibidos: publicaciones.filter((x) => x.autor === USUARIO.id).reduce((n, x) => n + x.likes, 0),
      reacciones: publicaciones.filter((x) => x.autor === USUARIO.id).reduce((n, x) => n + x.likes, 0),
      comentarios_recibidos: publicaciones.filter((x) => x.autor === USUARIO.id).reduce((n, x) => n + x.comentarios, 0),
      conversaciones: conversaciones.length,
      mensajes: conversaciones.reduce((n, c) => n + c.mensajes.length, 0),
      historias: 0,
      grupos: 0,
      seguidores: USUARIO.followers_count,
      siguiendo: USUARIO.following_count,
    });
  }

  if (a === 'me' && b === 'resumen') {
    const mias = publicaciones.filter((x) => x.autor === USUARIO.id && !x.borrada);
    const fotos = mias.reduce((n, x) => n + (x.imagenes || []).length, 0);
    return json({
      publicaciones: mias.length,
      comentarios: 24,
      fotos,
      megabytes: Math.round((fotos * 0.42) * 10) / 10,
      grupos: 3,
      mensajes: conversaciones.reduce((n, c) => n + c.mensajes.length, 0),
      archivos: fotos,
      siguiendo: USUARIO.following_count,
      seguidores: USUARIO.followers_count,
      imagenes_en_publicaciones: fotos,
    });
  }

  if (a === 'me' && b === 'blocked') {
    if (metodo === 'DELETE') return json({ ok: true });
    return json([]);
  }

  if (a === 'me' && b === 'saved') {
    const guardadas = publicaciones.filter((p) => p.guardada && !p.borrada);
    return json(paginado(guardadas.map(publicacionJSON), ruta));
  }

  // ---- Personas ----
  if (a === 'users') {
    if (b === 'suggestions') {
      return json(
        [...perfiles.values()]
          .filter((p) => p.id !== USUARIO.id)
          .map((p) => ({
            id: p.id, username: p.username, display_name: p.display_name,
            avatar_url: p.avatar_url, is_verified: p.is_verified,
            followers_count: p.followers_count,
          }))
      );
    }

    const perfil = perfiles.get(Number(b));
    if (!perfil) return json({ error: 'Usuario no encontrado' }, 404);
    if (c === 'posts') {
      const suyas = publicaciones.filter((p) => p.autor === perfil.id && !p.borrada).sort((x, y) => x.creada - y.creada);
      return json(paginado(suyas.map(publicacionJSON), ruta));
    }
    if (c === 'follow') {
      perfil.is_following = metodo === 'POST';
      perfil.followers_count += metodo === 'POST' ? 1 : -1;
      return json({ ok: true, is_following: perfil.is_following });
    }
    if (c === 'block') {
      perfil.is_blocked = metodo === 'POST';
      return json({ ok: true, is_blocked: perfil.is_blocked });
    }
    return json(perfil);
  }

  if (a === 'search') {
    const texto = (q.get('q') || '').toLowerCase();
    const tipo = q.get('type') || 'users';
    if (tipo === 'posts') {
      const elegidas = publicaciones.filter((p) => !p.borrada && (p.texto.toLowerCase().includes(texto) || !texto));
      const orden = q.get('orden') === 'populares'
        ? [...elegidas].sort((x, y) => y.likes - x.likes)
        : elegidas;
      return json(orden.map(publicacionJSON));
    }
    if (tipo === 'groups') {
      const todos = [grupoDemo, { ...grupoDemo, id: 8, name: 'Cocina de barrio', about: 'Recetas de todos los días, sin prisa.', miembros: 12 },
        { ...grupoDemo, id: 9, name: 'Rendimiento web', about: 'Medir antes de optimizar.', miembros: 31, privacy: 'private' }];
      return json(todos
        .filter((g) => !texto || g.name.toLowerCase().includes(texto) || (g.about || '').toLowerCase().includes(texto))
        .map((g) => ({ id: g.id, name: g.name, about: g.about, privacy: g.privacy, miembros: g.miembros, soy_miembro: g.id === grupoDemo.id })));
    }
    if (tipo === 'tags') {
      const usadas = ['diseno', 'moon', 'rendimiento', 'fotografia', 'equipos', 'periodismo', 'ciudad', 'cocina'];
      return json(usadas
        .filter((t) => !texto || t.includes(texto.replace('#', '')))
        .map((t, i) => ({ tag: t, posts_count: 128 - i * 17 })));
    }
    return json(
      [...perfiles.values()]
        .filter((p) => p.username.toLowerCase().includes(texto) || p.display_name.toLowerCase().includes(texto))
        .map((p) => ({ id: p.id, username: p.username, display_name: p.display_name, avatar_url: p.avatar_url, is_verified: p.is_verified, followers_count: p.followers_count, bio: p.bio }))
    );
  }

  // ---- Tendencias ----
  if (a === 'hashtags') {
    return json([
      { tag: 'diseno', posts_count: 128, last_used_at: hace(6) },
      { tag: 'moon', posts_count: 96, last_used_at: hace(22) },
      { tag: 'rendimiento', posts_count: 74, last_used_at: hace(55) },
      { tag: 'fotografia', posts_count: 61, last_used_at: hace(130) },
      { tag: 'equipos', posts_count: 48, last_used_at: hace(400) },
      { tag: 'periodismo', posts_count: 33, last_used_at: hace(260) },
    ]);
  }

  // ---- Notificaciones ----
  if (a === 'notifications') {
    if (b === 'read-all') {
      notificaciones.forEach((n) => { n.leida = true; });
      return json({ ok: true });
    }
    if (b && c === 'read') {
      const n = notificaciones.find((x) => x.id === Number(b));
      if (n) n.leida = true;
      return json({ ok: true });
    }
    const items = notificaciones.map((n) => {
      const de = perfiles.get(n.de);
      return {
        id: n.id,
        type: n.tipo,
        content: n.texto,
        created_at: hace(n.creada),
        is_read: n.leida,
        post_id: n.post || null,
        from_username: de.username,
        from_display_name: de.display_name,
        from_avatar_url: de.avatar_url,
      };
    });
    return json(paginado(items, ruta));
  }

  // ---- Mensajería ----
  if (a === 'messages') {
    if (b === 'conversations' && !c) {
      if (metodo === 'POST') {
        const objetivo = Number(cuerpo?.user_id);
        let conv = conversaciones.find((x) => x.partnerId === objetivo);
        if (!conv) {
          conv = { id: ++siguienteId, partnerId: objetivo, noLeidos: 0, mensajes: [] };
          conversaciones.push(conv);
        }
        return json({ conversation_id: conv.id });
      }
      const filtro = q.get('filtro') || 'todas';
      const fila = (conv) => {
        const socio = perfiles.get(conv.partnerId);
        const ultimo = conv.mensajes[conv.mensajes.length - 1];
        return {
          id: conv.id,
          username: socio.username,
          display_name: socio.display_name,
          avatar_url: socio.avatar_url,
          is_verified: socio.is_verified,
          last_message: ultimo ? (ultimo.texto || (ultimo.audio ? '🎤 Nota de voz' : '')) : '',
          updated_at: ultimo ? hace(ultimo.creada) : hace(9999),
          unread: conv.noLeidos,
          silenciada: !!conv.silenciada,
          archivada: !!conv.archivada,
          oculta: !!conv.oculta,
        };
      };
      let lista = conversaciones.map(fila);
      if (filtro === 'sin_leer') lista = lista.filter((c) => c.unread > 0 && !c.archivada);
      else if (filtro === 'archivadas') lista = lista.filter((c) => c.archivada);
      else lista = lista.filter((c) => !c.archivada && !c.oculta);
      return json(lista);
    }
    if (b === 'conversations' && c) {
      const conv = conversaciones.find((x) => x.id === Number(c));
      if (!conv) return json({ error: 'Conversación no encontrada' }, 404);
      const socio = perfiles.get(conv.partnerId);

      if (d === 'prefs' && metodo === 'POST') {
        if ('silenciada' in (cuerpo || {})) conv.silenciada = !!cuerpo.silenciada;
        if ('archivada' in (cuerpo || {})) conv.archivada = !!cuerpo.archivada;
        if ('oculta' in (cuerpo || {})) conv.oculta = !!cuerpo.oculta;
        return json({ silenciada: !!conv.silenciada, archivada: !!conv.archivada, oculta: !!conv.oculta });
      }

      if (d === 'read') { conv.noLeidos = 0; return json({ ok: true }); }

      if (d === 'messages' && metodo === 'POST') {
        const nuevo = {
          id: ++siguienteId,
          de: USUARIO.id,
          texto: cuerpo?.content || '',
          audio: cuerpo?.audio_url || '',
          duracion: Number(cuerpo?.duracion_ms || 0),
          creada: 0,
          estado: 'sent',
        };
        conv.mensajes.push(nuevo);
        // El contacto «responde» para que la demostración se sienta viva.
        setTimeout(() => emitirDemo({ type: 'typing', conversation_id: conv.id, user_id: socio.id }), 700);
        setTimeout(() => {
          const respuestas = [
            'Totalmente de acuerdo 👍',
            'Buena idea, lo anoto.',
            'Te cuento en un rato, estoy con algo.',
            '¡Qué bien! Se ve genial.',
          ];
          const respuesta = { id: ++siguienteId, de: socio.id, texto: respuestas[conv.mensajes.length % respuestas.length], creada: 0, estado: 'delivered' };
          conv.mensajes.push(respuesta);
          emitirDemo({ type: 'typing', conversation_id: conv.id, user_id: socio.id });
          emitirDemo({ type: 'message', conversation_id: conv.id, message: mensajeJSON(conv, respuesta) });
        }, 2200);
        return json(mensajeJSON(conv, nuevo));
      }
      return json({
        conversation_id: conv.id,
        partner: { id: socio.id, username: socio.username, display_name: socio.display_name, avatar_url: socio.avatar_url, is_verified: socio.is_verified },
        messages: conv.mensajes.map((m) => mensajeJSON(conv, m)),
        typing: false,
      });
    }
    // Reacciones y borrado de mensajes
    if (b && c === 'react') {
      const conv = conversaciones.find((x) => x.mensajes.some((m) => m.id === Number(b)));
      const msg = conv?.mensajes.find((m) => m.id === Number(b));
      if (msg) msg.reaccion = msg.reaccion === cuerpo?.reaction ? null : cuerpo?.reaction;
      return json({ ok: true });
    }
    if (b && !c && metodo === 'DELETE') {
      const conv = conversaciones.find((x) => x.mensajes.some((m) => m.id === Number(b)));
      const msg = conv?.mensajes.find((m) => m.id === Number(b));
      if (msg) { msg.texto = ''; msg.estado = 'deleted'; }
      return json({ ok: true });
    }
  }

  // ---- Grupos (modo demostración) ----
  if (a === 'groups') {
    if (b === 'messages') return json({ error: 'Falta el grupo' }, 404);
    if (!b && metodo === 'GET') {
      // Lista de grupos: «tus grupos» y «grupos para descubrir».
      const comoTarjeta = (g) => ({
        ...g,
        mi_papel: g.soy_miembro ? 'member' : '',
        owner_username: 'carla',
        owner_display_name: 'Carla Ríos',
      });
      void ruta;
      return json({
        mios: gruposDemo.filter((g) => g.soy_miembro).map(comoTarjeta),
        descubrir: gruposDemo.filter((g) => !g.soy_miembro).map(comoTarjeta),
      });
    }
    const gid = Number(b) || grupoDemo.id;

    // El chat del grupo: leer, escribir, borrar y contar.
    if (c === 'messages') {
      if (d === 'count') return json({ total: chatDelGrupo.length });
      if (metodo === 'DELETE') {
        const i = chatDelGrupo.findIndex((m) => m.id === Number(d));
        if (i >= 0) {
          chatDelGrupo.splice(i, 1);
          emitirDemo({ type: 'group_message_deleted', group_id: gid, message_id: Number(d) });
        }
        return json({ ok: true });
      }
      if (metodo === 'POST') {
        const nuevo = {
          id: ++siguienteId,
          de: USUARIO.id,
          texto: cuerpo?.content || '',
          audio: cuerpo?.audio_url || '',
          duracion: Number(cuerpo?.duracion_ms || 0),
          creada: 0,
        };
        chatDelGrupo.push(nuevo);
        // Alguien del grupo contesta, para que se sienta vivo.
        setTimeout(() => {
          const quien = perfiles.get(2);
          emitirDemo({ type: 'typing', group_id: gid, user_id: 2 });
          setTimeout(() => {
            const respuestas = [
              '¡Buena! Lo vemos esta noche 👍',
              'Anotado, llevo el objetivo largo.',
              'Escucho tu nota en un momento.',
              'Perfecto, nos vemos en la plaza.',
            ];
            const respuesta = {
              id: ++siguienteId,
              de: 2,
              texto: respuestas[chatDelGrupo.length % respuestas.length],
              creada: 0,
            };
            chatDelGrupo.push(respuesta);
            emitirDemo({ type: 'group_message', group_id: gid, message: mensajeDeGrupoJSON(respuesta) });
          }, 1900);
        }, 900);
        void quien;
        return json(mensajeDeGrupoJSON(nuevo));
      }
      return json({ mensajes: chatDelGrupo.map(mensajeDeGrupoJSON), hay_mas: false });
    }

    if (c === 'posts') {
      if (metodo === 'POST') {
        const nueva = {
          id: ++siguienteId, autor: USUARIO.id, creada: 0, likes: 0, comentarios: 0, guardados: 0,
          meGusta: false, guardada: false, texto: cuerpo?.content || '', imagenes: [],
        };
        publicaciones.unshift(nueva);
        return json(publicacionJSON(nueva));
      }
      return json(paginado([publicaciones[0], publicaciones[2]], ruta));
    }

    if (metodo === 'PATCH') {
      const g = gruposDemo.find((x) => x.id === gid);
      if (g) {
        if (typeof cuerpo?.name === 'string' && cuerpo.name.trim()) g.name = cuerpo.name.trim();
        if (typeof cuerpo?.about === 'string') g.about = cuerpo.about;
        if (['public', 'private'].includes(cuerpo?.privacy)) g.privacy = cuerpo.privacy;
        if (typeof cuerpo?.cover_url === 'string') g.cover_url = cuerpo.cover_url;
        return json({ ...g, soy_miembro: true, mi_papel: 'owner' });
      }
    }
    if (metodo === 'DELETE') {
      const i = gruposDemo.findIndex((x) => x.id === gid);
      if (i >= 0) gruposDemo.splice(i, 1);
      return json({ ok: true, eliminado: gid });
    }
    if (metodo === 'POST') return json({ ok: true });

    return json({
      ...grupoDemo,
      miembros_lista: [
        { ...miembroJSON(3), papel: 'owner' },
        { ...miembroJSON(4), papel: 'member' },
        { ...miembroJSON(2), papel: 'member' },
        { ...miembroJSON(1), papel: 'member' },
      ],
    });
  }

  // ---- Reportes ----
  if (a === 'reports') return json({ ok: true });

  // ---- Subidas (además del XHR simulado) ----
  if (a === 'upload') return json({ id: ++siguienteId, url: foto('#6366f1', '#22d3ee', 3) });

  // ---- Administración ----
  if (a === 'admin') {
    if (b === 'dashboard') {
      return json({
        users_total: 1284, users_new_today: 12, users_new_7d: 84, suspended_users: 3,
        posts_total: 9731, posts_today: 46, comments_total: 22140,
        follows_total: 15872, messages_total: 42118, reports_open: 3,
      });
    }
    if (b === 'stats') {
      // Últimos 30 días de actividad (mismo formato que el servidor real).
      return json(
        Array.from({ length: 30 }, (_, i) => {
          const dia = new Date(AHORA - (29 - i) * 86400000).toISOString().slice(0, 10);
          const base = 4 + ((i * 7) % 11);
          return {
            stat_date: dia,
            new_users: base,
            new_posts: base * 3 + (i % 5),
            new_messages: base * 9,
            new_likes: base * 12,
            new_comments: base * 2,
            new_follows: base * 5,
          };
        })
      );
    }
    if (b === 'users') {
      if (c) return json({ ok: true }); // suspender / activar / verificar
      const filtro = (q.get('q') || '').toLowerCase();
      const items = [...perfiles.values()]
        .filter((p) => !filtro || p.username.includes(filtro) || p.display_name.toLowerCase().includes(filtro))
        .map((p, i) => ({
          id: p.id, username: p.username, display_name: p.display_name, email: `${p.username}@moon.test`,
          is_verified: p.is_verified, is_suspended: false, role: p.id === 1 ? 'admin' : 'user',
          created_at: hace(1000 + i * 240), posts_count: p.posts_count, avatar_url: p.avatar_url,
        }));
      return json(paginado(items, ruta));
    }
    if (b === 'reports') {
      return json(paginado([
        { id: 1, target_type: 'post', target_id: 103, reason: 'spam', detail: '', status: 'open', created_at: hace(40), reporter_username: 'elena' },
        { id: 2, target_type: 'post', target_id: 105, reason: 'contenido sensible', detail: '', status: 'open', created_at: hace(300), reporter_username: 'bruno' },
        { id: 3, target_type: 'post', target_id: 102, reason: 'acoso', detail: '', status: 'resolved', created_at: hace(900), reporter_username: 'diego' },
      ], ruta));
    }
    if (b === 'words') {
      if (metodo === 'POST') { palabrasProhibidas.push((cuerpo?.word || '').toLowerCase()); return json({ ok: true }); }
      if (metodo === 'DELETE') return json({ ok: true });
      return json(palabrasProhibidas.map((w, i) => ({ id: i + 1, word: w, created_at: hace(2000) })));
    }
    return json({ items: [], total: 0, ok: true });
  }

  return json({ error: `Sin datos en la demostración para ${metodo} ${camino}` }, 404);
}

// ---------- Instalación ----------

class SocketDemo {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  constructor() {
    this.readyState = 1;
    this.onopen = null;
    this.onmessage = null;
    this.onclose = null;
    this.onerror = null;
    socketsAbiertos.add(this);
    setTimeout(() => { if (this.onopen) this.onopen({}); }, 40);
  }

  recibir(ev) {
    if (this.onmessage) this.onmessage({ data: JSON.stringify(ev) });
  }

  send(texto) {
    let msg = null;
    try { msg = JSON.parse(texto); } catch { return; }
    if (msg.type === 'sync') {
      setTimeout(() => this.recibir({
        type: 'sync',
        data: {
          unread_notifications: notificaciones.filter((n) => !n.leida).length,
          unread_messages: conversaciones.reduce((t, c2) => t + c2.noLeidos, 0),
        },
      }), 60);
    }
  }

  close() {
    this.readyState = 3;
    socketsAbiertos.delete(this);
    if (this.onclose) this.onclose({});
  }
}

class SubidaDemo {
  constructor() {
    this.upload = {};
    this.status = 0;
    this.responseText = '';
  }
  open() {}
  setRequestHeader() {}
  abort() {}
  send(formulario) {
    const archivo = formulario && formulario.get ? formulario.get('file') : null;
    const terminar = (url) => {
      this.status = 200;
      this.responseText = JSON.stringify({ id: ++siguienteId, url });
      if (this.upload.onprogress) this.upload.onprogress({ lengthComputable: true, loaded: 1, total: 1 });
      if (this.onload) this.onload();
    };
    if (archivo && typeof FileReader !== 'undefined') {
      const lector = new FileReader();
      lector.onload = () => terminar(lector.result);
      lector.onerror = () => terminar(foto('#6366f1', '#22d3ee', 2));
      lector.readAsDataURL(archivo);
    } else {
      setTimeout(() => terminar(foto('#6366f1', '#22d3ee', 2)), 300);
    }
  }
}

let instalado = false;

export function instalarDemo() {
  if (instalado || !esDemo()) return;
  instalado = true;

  // Algunos entornos no traen `fetch`; la demostración funciona igual.
  const fetchReal = typeof window.fetch === 'function' ? window.fetch.bind(window) : null;
  window.fetch = async (entrada, opciones = {}) => {
    const url = typeof entrada === 'string' ? entrada : (entrada && entrada.url) || '';
    const ruta = url.replace(/^https?:\/\/[^/]+/, '');
    if (!ruta.startsWith('/api/')) {
      if (fetchReal) return fetchReal(entrada, opciones);
      return respuesta({ error: 'Sin red en la demostración' }, 503);
    }

    let cuerpo = opciones.body;
    if (typeof cuerpo === 'string' && cuerpo) {
      try { cuerpo = JSON.parse(cuerpo); } catch { /* formulario */ }
    }
    await new Promise((r) => setTimeout(r, 140)); // latencia para ver los esqueletos
    const { status, data } = responder((opciones.method || 'GET').toUpperCase(), ruta, cuerpo);
    return respuesta(data, status);
  };

  window.XMLHttpRequest = SubidaDemo;
  window.WebSocket = SocketDemo;
}
