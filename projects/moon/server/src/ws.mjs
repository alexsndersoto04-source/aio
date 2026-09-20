// Moon — Tiempo real (WebSocket)
// ============================================================
// Un hub con las conexiones abiertas por usuario. Empuja notificaciones,
// mensajes nuevos, «escribiendo…», reacciones y borrados, y responde a la
// petición de contadores (`sync`). Autenticación por token en la URL, igual
// que el servidor Titan.

import { WebSocketServer } from 'ws';
import { verificarJwt } from './auth.mjs';
import { uno } from './db.mjs';
import { empujarSiQuiere } from './empuje.mjs';
import { demasiadoRapido } from './limites.mjs';

const conexiones = new Map(); // userId -> Set(socket)

// Llamadas en curso, por id: { de, para, tipo }. Sirve para avisar si alguien
// se queda sin conexión y para no encimar dos llamadas a la misma persona.
const llamadas = new Map();

const TIPOS_LLAMADA = new Set(['call_start', 'call_accept', 'call_reject', 'call_end', 'call_signal']);

// Cuánto aguanta una llamada esperando a que el otro abra Moon y conteste
// (le suena el teléfono con el aviso).
const VIDA_LLAMADA_SIN_TELEFONO_MS = 90_000;

// ¿Estas dos personas tienen una conversación abierta? Solo entre ellas se
// permite pasar sobres de llamada (nadie puede llamar a un desconocido).
async function sonPareja(pool, uno_, otro) {
  if (!Number.isInteger(Number(otro)) || Number(otro) <= 0) return false;
  const fila = await uno(
    pool,
    `SELECT 1 FROM conversations
      WHERE (user_a = $1 AND user_b = $2) OR (user_a = $2 AND user_b = $1)`,
    [Number(uno_), Number(otro)]
  );
  return !!fila;
}

export function montarWs(servidorHttp, pool, secreto) {
  const wss = new WebSocketServer({ noServer: true });

  servidorHttp.on('upgrade', async (req, socket, cabeza) => {
    let url;
    try {
      url = new URL(req.url, 'http://x');
    } catch {
      socket.destroy();
      return;
    }
    if (url.pathname !== '/ws') {
      socket.destroy();
      return;
    }
    const token = url.searchParams.get('token') || '';
    const claims = verificarJwt(decodeURIComponent(token), secreto);
    if (!claims || !claims.uid) {
      socket.write('HTTP/1.1 401 Unauthorized\r\n\r\n');
      socket.destroy();
      return;
    }
    const usuario = await uno(pool, 'SELECT id, status FROM users WHERE id = $1', [claims.uid]);
    if (!usuario || usuario.status === 'suspended') {
      socket.write('HTTP/1.1 403 Forbidden\r\n\r\n');
      socket.destroy();
      return;
    }
    req._uid = Number(usuario.id);
    wss.handleUpgrade(req, socket, cabeza, (ws) => wss.emit('connection', ws, req));
  });

  wss.on('connection', (ws, req) => {
    const uid = Number(req._uid);
    ws._pool = pool; // para las consultas de las llamadas
    if (!conexiones.has(uid)) conexiones.set(uid, new Set());
    conexiones.get(uid).add(ws);
    ws.enviar = (evento) => {
      if (ws.readyState === ws.OPEN) ws.send(JSON.stringify(evento));
    };
    ws.enviar({ type: 'connected', user_id: uid });
    // Aviso de presencia: los demás refrescan la lista de contactos.
    setTimeout(() => {
      for (const otro of conexiones.keys()) {
        if (Number(otro) !== uid) enviarA(Number(otro), { type: 'presence', user_id: uid, online: true });
      }
    }, 50);

    // ¿Le estaba entrando una llamada mientras no tenía Moon abierto? Ahora que
    // lo abrió, le timbra. Se manda dos veces (al segundo y a los tres
    // segundos) para que no se pierda si la app todavía está arrancando; la
    // pantalla del que recibe sabe ignorar el repetido.
    for (const retraso of [700, 3000]) {
      setTimeout(() => {
        void (async () => {
          for (const [id, datos] of llamadas) {
            if (datos.para !== uid) continue;
            if (Date.now() - Number(datos.creada || 0) > VIDA_LLAMADA_SIN_TELEFONO_MS) {
              llamadas.delete(id);
              continue;
            }
            const quien = await uno(pool, 'SELECT id, username, display_name, avatar_url FROM users WHERE id = $1', [datos.de]);
            timbrarA(uid, { ...datos, id }, quien);
          }
        })();
      }, retraso);
    }

    ws.on('message', async (datos) => {
      let msg;
      try {
        msg = JSON.parse(String(datos));
      } catch {
        return;
      }
      if (!msg || typeof msg.type !== 'string') return;

      if (msg.type === 'ping') {
        ws.enviar({ type: 'pong' });
        return;
      }
      if (msg.type === 'sync') {
        const noLeidas = await uno(
          pool,
          'SELECT COUNT(*)::int AS count FROM notifications WHERE user_id = $1 AND is_read = FALSE',
          [uid]
        );
        const noLeidos = await uno(
          pool,
          `SELECT COUNT(*)::int AS count FROM messages m
             JOIN conversations cv ON cv.id = m.conversation_id
            WHERE (cv.user_a = $1 OR cv.user_b = $1) AND m.sender_id <> $1
              AND m.read_at IS NULL AND m.status <> 'deleted'`,
          [uid]
        );
        ws.enviar({
          type: 'sync',
          data: {
            unread_notifications: Number(noLeidas?.count || 0),
            unread_messages: Number(noLeidos?.count || 0),
          },
        });
        return;
      }
      if (msg.type === 'typing' && msg.group_id) {
        // «Escribiendo…» dentro del chat de un grupo: solo ven los miembros.
        const dentro = await uno(
          pool,
          'SELECT 1 FROM group_members WHERE group_id = $1 AND user_id = $2',
          [Number(msg.group_id), uid]
        );
        if (!dentro) return;
        const miembros = await pool.query('SELECT user_id FROM group_members WHERE group_id = $1 LIMIT 200', [Number(msg.group_id)]);
        for (const m of miembros.rows) {
          const otro = Number(m.user_id);
          if (otro !== uid) enviarA(otro, { type: 'typing', group_id: Number(msg.group_id), user_id: uid });
        }
        return;
      }
      if (msg.type === 'typing' && msg.conversation_id) {
        const conv = await uno(
          pool,
          'SELECT user_a, user_b FROM conversations WHERE id = $1',
          [Number(msg.conversation_id)]
        );
        if (!conv) return;
        const otro = Number(conv.user_a) === uid ? Number(conv.user_b) : Number(conv.user_a);
        enviarA(otro, { type: 'typing', conversation_id: Number(msg.conversation_id), user_id: uid });
        return;
      }
      // ---------- Llamadas de voz y video ----------
      // El servidor no toca la voz ni la imagen: solo pasa los sobres entre los
      // dos teléfonos (el aviso de llamada, aceptar, rechazar, colgar y los
      // datos de conexión). La voz viaja directa entre ellos.
      if (TIPOS_LLAMADA.has(msg.type)) {
        await atenderLlamada(ws, uid, msg);
      }
    });

    ws.on('close', () => {
      // Si esta persona tenía una llamada en curso o timbrando, la otra se
      // entera al instante (se cerró la app, se cayó la conexión…).
      for (const [id, datos] of llamadas) {
        if (datos.de === uid) {
          enviarA(datos.para, { type: 'call_terminada', call_id: id, segundos: 0, motivo: 'se_fue' });
          llamadas.delete(id);
        } else if (datos.para === uid) {
          enviarA(datos.de, { type: 'call_terminada', call_id: id, segundos: 0, motivo: 'se_fue' });
          llamadas.delete(id);
        }
      }
      const conjunto = conexiones.get(uid);
      if (!conjunto) return;
      conjunto.delete(ws);
      if (conjunto.size === 0) {
        conexiones.delete(uid);
        for (const otro of conexiones.keys()) {
          enviarA(Number(otro), { type: 'presence', user_id: uid, online: false });
        }
      }
    });
    ws.on('error', () => {});
  });

  return wss;
}

/**
 * Avisa al teléfono de una llamada: manda la notificación (con tono de llamada)
 * y devuelve cuántos teléfonos tiene apuntados esa persona. Si no hay ninguno
 * (o tiene los avisos apagados), devuelve 0.
 */
async function avisarAlTelefono(pool, datos, quien) {
  const [telefonos, prefs] = await Promise.all([
    uno(pool, 'SELECT COUNT(*)::int AS n FROM push_subscriptions WHERE user_id = $1', [datos.para]),
    uno(pool, 'SELECT * FROM notification_prefs WHERE user_id = $1', [datos.para]),
  ]);
  const cuantos = Number(telefonos?.n || 0);
  if (cuantos === 0 || (prefs && prefs.message === false)) return 0;
  empujarSiQuiere(pool, datos.para, 'message', {
    titulo: `${quien?.display_name || quien?.username || 'Moon'} te está llamando`,
    texto: `${datos.tipo === 'video' ? 'Videollamada' : 'Llamada de voz'}. Toca para abrir Moon y contestar.`,
    url: datos.conversation_id ? `#/messages/${datos.conversation_id}` : '#/messages',
    etiqueta: `llamada-${datos.id}`,
    urgente: true,
    quedarse: true,
    vibrar: [400, 200, 400, 200, 400],
  }).catch(() => {});
  return cuantos;
}

// Le manda el timbrazo a alguien: lo usa la llamada normal y también el
// aviso que se repite cuando la persona abre Moon.
function timbrarA(uid, datos, quien) {
  enviarA(Number(uid), {
    type: 'call_ring',
    call_id: datos.id,
    tipo: datos.tipo,
    conversation_id: datos.conversation_id || null,
    de: {
      id: Number(datos.de),
      username: quien?.username || '',
      display_name: quien?.display_name || quien?.username || '',
      avatar_url: quien?.avatar_url || '',
    },
  });
}

// Atiende un sobre de llamada y lo pasa al otro teléfono.
//   call_start  → al otro: call_ring (con quién llama), call_avisando (se le
//                 avisó al teléfono y sigue sonando) o call_sin_conexion
//   call_accept → al otro: call_aceptada
//   call_reject → al otro: call_rechazada
//   call_end    → al otro: call_terminada
//   call_signal → al otro: call_senal (los datos de conexión de WebRTC)
async function atenderLlamada(ws, uid, msg) {
  const otro = Number(msg.to);
  const id = String(msg.call_id || '').slice(0, 60);
  if (!id) return;
  if (!(await sonPareja(ws._pool, uid, otro))) return;

  if (msg.type === 'call_start') {
    // El mismo aviso repetido (al reconectar) no se cuenta doble ni se
    // contesta con «ocupado»: la llamada ya está timbrando.
    if (llamadas.has(id)) return;
    if (demasiadoRapido(`llamada:${uid}`, 8, 60_000)) {
      ws.enviar({ type: 'call_rechazada', call_id: id, motivo: 'rapido' });
      return;
    }
    const tipo = msg.tipo === 'video' ? 'video' : 'voz';
    // ¿Ya hay una llamada con alguna de las dos partes? Se avisa y no se encima.
    for (const [otraId, datos] of llamadas) {
      if (datos.de === otro || datos.para === otro) {
        ws.enviar({ type: 'call_rechazada', call_id: id, motivo: 'ocupado' });
        return;
      }
      if (datos.de === uid || datos.para === uid) {
        ws.enviar({ type: 'call_rechazada', call_id: id, motivo: 'tu_llamada' });
        return;
      }
      void otraId;
    }
    const quien = await uno(ws._pool, 'SELECT id, username, display_name, avatar_url FROM users WHERE id = $1', [uid]);
    const conversacion = Number(msg.conversation_id) || null;
    // La llamada queda anotada aunque el otro no tenga Moon abierto: así, si lo
    // abre dentro de un rato, le timbra igual (ver el aviso al conectar).
    const anotada = { id, de: uid, para: otro, tipo, conversation_id: conversacion, creada: Date.now() };
    llamadas.set(id, anotada);

    const vivos = conexiones.get(otro);
    if (vivos && vivos.size > 0) {
      timbrarA(otro, anotada, quien);
      // Respaldo: si a los 8 segundos nadie contestó (puede tener Moon abierto
      // pero dormido en segundo plano), se le manda el aviso al teléfono.
      setTimeout(() => {
        void (async () => {
          const viva = llamadas.get(id);
          if (!viva || viva.aceptada || viva.avisado) return;
          if (Date.now() - Number(viva.creada || 0) > VIDA_LLAMADA_SIN_TELEFONO_MS) return;
          const suyo = await uno(ws._pool, 'SELECT id, username, display_name, avatar_url FROM users WHERE id = $1', [viva.de]);
          const cuantos = await avisarAlTelefono(ws._pool, viva, suyo);
          if (cuantos > 0) {
            viva.avisado = true;
            enviarA(viva.de, { type: 'call_avisando', call_id: id, dispositivos: cuantos });
          }
        })();
      }, 8000);
      return;
    }

    // No tiene Moon abierto: si tiene un teléfono apuntado (avisos encendidos),
    // se le manda el aviso y la llamada sigue viva esperando a que lo abra.
    // El aviso sale por detrás: el que llama no espera a que se entregue.
    const cuantosTelefonos = await avisarAlTelefono(ws._pool, anotada, quien);
    if (cuantosTelefonos > 0) {
      anotada.avisado = true;
      ws.enviar({ type: 'call_avisando', call_id: id, dispositivos: cuantosTelefonos });
      return;
    }

    // Sin teléfono apuntado (o con los avisos apagados) no hay forma de
    // avisarle: se dice tal cual y la llamada termina.
    llamadas.delete(id);
    ws.enviar({ type: 'call_sin_conexion', call_id: id });
    return;
  }

  const datos = llamadas.get(id);
  // Sobres sueltos de una llamada que ya no existe: se ignoran.
  if (!datos) {
    if (msg.type === 'call_end') enviarA(otro, { type: 'call_terminada', call_id: id, segundos: 0, motivo: 'tarde' });
    return;
  }
  if (datos.de !== uid && datos.para !== uid) return;

  if (msg.type === 'call_accept') {
    // El que contesta pasa a ser el segundo; la llamada sigue viva.
    datos.aceptada = true;
    enviarA(datos.de, { type: 'call_aceptada', call_id: id, de: uid });
    return;
  }
  if (msg.type === 'call_reject') {
    llamadas.delete(id);
    enviarA(datos.de === uid ? datos.para : datos.de, {
      type: 'call_rechazada', call_id: id, motivo: String(msg.motivo || 'rechazada').slice(0, 20),
    });
    return;
  }
  if (msg.type === 'call_end') {
    llamadas.delete(id);
    const segundos = Math.min(Math.max(Number(msg.segundos || 0), 0), 86400);
    enviarA(datos.de === uid ? datos.para : datos.de, {
      type: 'call_terminada', call_id: id, segundos, motivo: 'colgo',
    });
    return;
  }
  if (msg.type === 'call_signal') {
    // Datos de conexión: pasan tal cual, sin guardarse en ningún sitio.
    enviarA(datos.de === uid ? datos.para : datos.de, {
      type: 'call_senal', call_id: id, de: uid, sobre: msg.sobre || null,
    });
  }
}

export function enviarA(uid, evento) {
  const conjunto = conexiones.get(Number(uid));
  if (!conjunto) return;
  for (const ws of conjunto) {
    try {
      ws.enviar(evento);
    } catch {
      /* conexión caída */
    }
  }
}

export function enviarATodos(evento) {
  for (const [, conjunto] of conexiones) {
    for (const ws of conjunto) {
      try {
        ws.enviar(evento);
      } catch {
        /* conexión caída */
      }
    }
  }
}

// Cuántas conexiones hay y QUÉ usuarios están conectados ahora mismo.
export const conectados = () => conexiones.size;
export const usuariosConectados = () => [...conexiones.keys()].map(Number);

// Crea la notificación (respetando las preferencias) y la empuja en vivo.
export async function notificar(pool, { userId, tipo, deUserId, postId = null, commentId = null, contenido = '' }) {
  if (Number(userId) === Number(deUserId)) return;
  const prefs = await uno(pool, 'SELECT * FROM notification_prefs WHERE user_id = $1', [userId]);
  const clave = tipo === 'mention' ? 'mention' : tipo === 'comment' ? 'comment' : tipo === 'like' ? 'like' : tipo === 'follow' ? 'follow' : 'system';
  if (prefs && prefs[clave] === false) return;

  const creada = await uno(
    pool,
    `INSERT INTO notifications (user_id, type, from_user_id, post_id, comment_id, content)
     VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, created_at::text AS created_at`,
    [userId, tipo, deUserId, postId, commentId, contenido]
  );
  const de = await uno(pool, 'SELECT username, display_name, avatar_url FROM users WHERE id = $1', [deUserId]);

  // Aviso al teléfono: llega aunque Moon esté cerrado.
  const textos = {
    follow: `${de?.display_name || de?.username || 'Alguien'} empezó a seguirte`,
    like: `${de?.display_name || de?.username || 'Alguien'} reaccionó a tu publicación`,
    comment: `${de?.display_name || de?.username || 'Alguien'} comentó tu publicación`,
    reply: `${de?.display_name || de?.username || 'Alguien'} respondió tu comentario`,
    mention: `${de?.display_name || de?.username || 'Alguien'} te mencionó`,
    system: 'Tienes un aviso nuevo en Moon',
  };
  const destino = postId ? `#/post/${postId}` : tipo === 'follow' ? `#/user/${deUserId}` : '#/notifications';
  empujarSiQuiere(pool, userId, clave, {
    titulo: 'Moon',
    texto: contenido || textos[tipo] || 'Tienes algo nuevo en Moon',
    url: destino,
    etiqueta: `${tipo}-${creada.id}`,
  }).catch(() => {});

  enviarA(userId, {
    type: 'notification',
    notification: {
      id: Number(creada.id),
      type: tipo,
      content: contenido,
      is_read: false,
      created_at: creada.created_at,
      post_id: postId,
      comment_id: commentId,
      from_user_id: Number(deUserId),
      from_username: de?.username,
      from_display_name: de?.display_name || de?.username,
      from_avatar_url: de?.avatar_url,
    },
  });
}
