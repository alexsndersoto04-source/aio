// Moon — Mensajería
// ============================================================
// Conversaciones uno a uno con mensajes persistentes, no leídos, reacciones,
// borrado y entrega en vivo por WebSocket.

import { ApiErr, texto, paginacion } from './util.mjs';
import { fila, uno } from './db.mjs';
import { auditar, sumarEstadistica } from './db.mjs';
import { enviarA } from './ws.mjs';
import { empujarSiQuiere } from './empuje.mjs';
import { demasiadoRapido } from './limites.mjs';

function mensajePublico(m) {
  return {
    id: Number(m.id),
    conversation_id: Number(m.conversation_id),
    sender_id: Number(m.sender_id),
    content: m.status === 'deleted' ? '' : m.content,
    image_url: m.image_url || '',
    status: m.status,
    reaction: m.reaction || null,
    read_at: m.read_at ? String(m.read_at) : null,
    edited_at: null,
    created_at: String(m.created_at),
  };
}

// Una conversación por pareja, siempre con user_a < user_b.
async function buscarOCrearConversacion(pool, uno_, otro) {
  const [a, b] = Number(uno_) < Number(otro) ? [Number(uno_), Number(otro)] : [Number(otro), Number(uno_)];
  let conv = await uno(pool, 'SELECT * FROM conversations WHERE user_a = $1 AND user_b = $2', [a, b]);
  if (conv) return conv;
  conv = await uno(
    pool,
    'INSERT INTO conversations (user_a, user_b) VALUES ($1, $2) ON CONFLICT (user_a, user_b) DO UPDATE SET user_a = EXCLUDED.user_a RETURNING *',
    [a, b]
  );
  return conv;
}

export function registrarRutasMensajes(router) {
  router.get('/api/messages/conversations', async (c) => {
    const yo = await c.exigir();
    const filas = await c.pool.query(
      `SELECT cv.id, cv.last_message, cv.last_message_at::text AS last_message_at,
              CASE WHEN cv.user_a = $1 THEN cv.user_b ELSE cv.user_a END AS otro_id,
              u.username, u.display_name, u.avatar_url, u.is_verified,
              (SELECT COUNT(*)::int FROM messages m
                WHERE m.conversation_id = cv.id AND m.sender_id <> $1
                  AND m.read_at IS NULL AND m.status <> 'deleted') AS unread
         FROM conversations cv
         JOIN users u ON u.id = (CASE WHEN cv.user_a = $1 THEN cv.user_b ELSE cv.user_a END)
        WHERE (cv.user_a = $1 OR cv.user_b = $1)
          AND NOT EXISTS (SELECT 1 FROM blocks b
                           WHERE (b.blocker_id = $1 AND b.blocked_id = u.id)
                              OR (b.blocker_id = u.id AND b.blocked_id = $1))
        ORDER BY COALESCE(cv.last_message_at, cv.created_at) DESC LIMIT 100`,
      [yo.id]
    );
    return filas.rows.map((f) => ({
      id: Number(f.id),
      username: f.username,
      display_name: f.display_name || f.username,
      avatar_url: f.avatar_url,
      is_verified: !!f.is_verified,
      last_message: f.last_message,
      updated_at: f.last_message_at,
      unread: Number(f.unread),
    }));
  });

  router.post('/api/messages/conversations', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const objetivo = Number(b.user_id);
    const otro = await uno(c.pool, 'SELECT * FROM users WHERE id = $1', [objetivo]);
    if (!otro) throw new ApiErr('Usuario no encontrado', 404);
    if (Number(otro.id) === Number(yo.id)) throw new ApiErr('No puedes abrir una conversación contigo', 400);
    const bloqueo = await uno(
      c.pool,
      'SELECT 1 FROM blocks WHERE (blocker_id = $1 AND blocked_id = $2) OR (blocker_id = $2 AND blocked_id = $1)',
      [yo.id, otro.id]
    );
    if (bloqueo) throw new ApiErr('No puedes escribir a esta persona', 403, 'blocked');

    // Privacidad de mensajes: respeta la preferencia del destinatario.
    if (otro.dm_privacy === 'nobody') throw new ApiErr('Esta persona no acepta mensajes', 403, 'dm_closed');
    if (otro.dm_privacy === 'following') {
      const leSigo = await uno(c.pool, 'SELECT 1 FROM follows WHERE follower_id = $1 AND following_id = $2', [yo.id, otro.id]);
      if (!leSigo) throw new ApiErr('Esta persona solo acepta mensajes de a quien sigue', 403, 'dm_following');
    }

    const conv = await buscarOCrearConversacion(c.pool, yo.id, otro.id);
    return { conversation_id: Number(conv.id) };
  });

  router.get('/api/messages/conversations/:id', async (c) => {
    const yo = await c.exigir();
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [Number(c.params.id)]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (![Number(conv.user_a), Number(conv.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Conversación no encontrada', 404);
    }
    const otroId = Number(conv.user_a) === Number(yo.id) ? Number(conv.user_b) : Number(conv.user_a);
    const otro = await uno(c.pool, 'SELECT * FROM users WHERE id = $1', [otroId]);
    const { limit } = paginacion(c.req, 50, 100);
    const mensajes = await c.pool.query(
      `SELECT * FROM (
         SELECT id, conversation_id, sender_id, content, image_url, status, reaction,
                read_at, created_at::text AS created_at
           FROM messages WHERE conversation_id = $1 ORDER BY created_at DESC LIMIT ${limit}
       ) AS recientes ORDER BY created_at ASC`,
      [conv.id]
    );
    return {
      conversation_id: Number(conv.id),
      partner: {
        id: Number(otro.id),
        username: otro.username,
        display_name: otro.display_name || otro.username,
        avatar_url: otro.avatar_url,
        is_verified: !!otro.is_verified,
      },
      messages: mensajes.rows.map(mensajePublico),
      typing: false,
    };
  });

  router.post('/api/messages/conversations/:id/messages', async (c) => {
    const yo = await c.exigir();
    if (demasiadoRapido(`msg:${yo.id}`, 30, 60_000)) {
      throw new ApiErr('Vas demasiado rápido: espera unos segundos', 429, 'rate_limit');
    }
    const b = await c.cuerpo();
    const contenido = texto(b.content || '', { min: 0, max: 2000, campo: 'mensaje' }).trim();
    const imagen = typeof b.image_url === 'string' ? b.image_url.slice(0, 500) : '';
    if (!contenido && !imagen) throw new ApiErr('Escribe un mensaje', 400);

    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [Number(c.params.id)]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (![Number(conv.user_a), Number(conv.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Conversación no encontrada', 404);
    }
    const otroId = Number(conv.user_a) === Number(yo.id) ? Number(conv.user_b) : Number(conv.user_a);
    const bloqueo = await uno(
      c.pool,
      'SELECT 1 FROM blocks WHERE (blocker_id = $1 AND blocked_id = $2) OR (blocker_id = $2 AND blocked_id = $1)',
      [yo.id, otroId]
    );
    if (bloqueo) throw new ApiErr('No puedes escribir en esta conversación', 403, 'blocked');

    const creado = await uno(
      c.pool,
      `INSERT INTO messages (conversation_id, sender_id, content, image_url) VALUES ($1, $2, $3, $4) RETURNING *`,
      [conv.id, yo.id, contenido, imagen]
    );
    await c.pool.query(
      'UPDATE conversations SET last_message = $1, last_sender_id = $2, last_message_at = NOW() WHERE id = $3',
      [(contenido || '📷 Imagen').slice(0, 200), yo.id, conv.id]
    );
    await sumarEstadistica(c.pool, 'new_messages');

    const publico = mensajePublico(creado);
    enviarA(otroId, { type: 'message', conversation_id: Number(conv.id), message: publico });

    // Aviso al teléfono de quien recibe (llega con Moon cerrado).
    const quien = await uno(c.pool, 'SELECT display_name, username FROM users WHERE id = $1', [yo.id]);
    empujarSiQuiere(c.pool, otroId, 'message', {
      titulo: quien?.display_name || quien?.username || 'Moon',
      texto: contenido ? contenido.slice(0, 140) : 'Te envió una imagen',
      url: `#/messages/${conv.id}`,
      etiqueta: `mensaje-${conv.id}`,
    }).catch(() => {});

    return publico;
  });

  router.post('/api/messages/conversations/:id/read', async (c) => {
    const yo = await c.exigir();
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [Number(c.params.id)]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (![Number(conv.user_a), Number(conv.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Conversación no encontrada', 404);
    }
    await c.pool.query(
      'UPDATE messages SET read_at = NOW(), status = $1 WHERE conversation_id = $2 AND sender_id <> $3 AND read_at IS NULL',
      ['read', conv.id, yo.id]
    );
    const otroId = Number(conv.user_a) === Number(yo.id) ? Number(conv.user_b) : Number(conv.user_a);
    enviarA(otroId, { type: 'messages_read', conversation_id: Number(conv.id), user_id: Number(yo.id) });
    return { ok: true };
  });

  router.post('/api/messages/:id/react', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const reaccion = typeof b.reaction === 'string' ? b.reaction.slice(0, 8) : '';
    const msg = await uno(
      c.pool,
      `SELECT m.*, cv.user_a, cv.user_b FROM messages m JOIN conversations cv ON cv.id = m.conversation_id WHERE m.id = $1`,
      [Number(c.params.id)]
    );
    if (!msg) throw new ApiErr('Mensaje no encontrado', 404);
    if (![Number(msg.user_a), Number(msg.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Mensaje no encontrado', 404);
    }
    const nueva = msg.reaction === reaccion ? '' : reaccion;
    await c.pool.query('UPDATE messages SET reaction = $1 WHERE id = $2', [nueva, msg.id]);
    const otroId = Number(msg.user_a) === Number(yo.id) ? Number(msg.user_b) : Number(msg.user_a);
    enviarA(otroId, { type: 'message_reacted', conversation_id: Number(msg.conversation_id), message_id: Number(msg.id), reaction: nueva || null });
    return { ok: true, reaction: nueva || null };
  });

  router.del('/api/messages/:id', async (c) => {
    const yo = await c.exigir();
    const msg = await uno(
      c.pool,
      `SELECT m.*, cv.user_a, cv.user_b FROM messages m JOIN conversations cv ON cv.id = m.conversation_id WHERE m.id = $1`,
      [Number(c.params.id)]
    );
    if (!msg) throw new ApiErr('Mensaje no encontrado', 404);
    if (Number(msg.sender_id) !== Number(yo.id)) throw new ApiErr('Solo puedes borrar tus mensajes', 403);
    await c.pool.query(`UPDATE messages SET status = 'deleted', content = '' WHERE id = $1`, [msg.id]);
    const otroId = Number(msg.user_a) === Number(yo.id) ? Number(msg.user_b) : Number(msg.user_a);
    enviarA(otroId, { type: 'message_deleted', conversation_id: Number(msg.conversation_id), message_id: Number(msg.id) });
    await auditar(c.pool, Number(yo.id), 'mensaje_borrado', `#${msg.id}`, c.ip);
    return { ok: true };
  });

  void fila;
}
