// Moon — Tiempo real (WebSocket)
// ============================================================
// Un hub con las conexiones abiertas por usuario. Empuja notificaciones,
// mensajes nuevos, «escribiendo…», reacciones y borrados, y responde a la
// petición de contadores (`sync`). Autenticación por token en la URL, igual
// que el servidor Titan.

import { WebSocketServer } from 'ws';
import { verificarJwt } from './auth.mjs';
import { uno } from './db.mjs';

const conexiones = new Map(); // userId -> Set(socket)

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
      if (msg.type === 'typing' && msg.conversation_id) {
        const conv = await uno(
          pool,
          'SELECT user_a, user_b FROM conversations WHERE id = $1',
          [Number(msg.conversation_id)]
        );
        if (!conv) return;
        const otro = Number(conv.user_a) === uid ? Number(conv.user_b) : Number(conv.user_a);
        enviarA(otro, { type: 'typing', conversation_id: Number(msg.conversation_id), user_id: uid });
      }
    });

    ws.on('close', () => {
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
