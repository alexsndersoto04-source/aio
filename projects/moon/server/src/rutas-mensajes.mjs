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
    audio_url: m.status === 'deleted' ? '' : (m.audio_url || ''),
    duracion_ms: Number(m.duracion_ms || 0),
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
  /**
   * Preferencias de una conversación para quien la pide: silenciada,
   * archivada u oculta. Si no hay fila, todo queda en falso.
   */
  async function prefsDe(pool, conversacionId, userId) {
    const p = await uno(
      pool,
      'SELECT silenciada, archivada, oculta FROM conversation_prefs WHERE conversation_id = $1 AND user_id = $2',
      [conversacionId, userId]
    );
    return {
      silenciada: !!p?.silenciada,
      archivada: !!p?.archivada,
      oculta: !!p?.oculta,
    };
  }

  router.get('/api/messages/conversations', async (c) => {
    const yo = await c.exigir();
    // todas (por defecto) · sin_leer · archivadas
    const filtro = String(c.query.get('filtro') || 'todas');
    const filas = await c.pool.query(
      `SELECT cv.id, cv.last_message, cv.last_message_at::text AS last_message_at,
              CASE WHEN cv.user_a = $1 THEN cv.user_b ELSE cv.user_a END AS otro_id,
              u.username, u.display_name, u.avatar_url, u.is_verified,
              COALESCE(pr.silenciada, FALSE) AS silenciada,
              COALESCE(pr.archivada, FALSE) AS archivada,
              COALESCE(pr.oculta, FALSE) AS oculta,
              (SELECT COUNT(*)::int FROM messages m
                WHERE m.conversation_id = cv.id AND m.sender_id <> $1
                  AND m.read_at IS NULL AND m.status <> 'deleted') AS unread
         FROM conversations cv
         JOIN users u ON u.id = (CASE WHEN cv.user_a = $1 THEN cv.user_b ELSE cv.user_a END)
         LEFT JOIN conversation_prefs pr ON pr.conversation_id = cv.id AND pr.user_id = $1
        WHERE (cv.user_a = $1 OR cv.user_b = $1)
          AND NOT EXISTS (SELECT 1 FROM blocks b
                           WHERE (b.blocker_id = $1 AND b.blocked_id = u.id)
                              OR (b.blocker_id = u.id AND b.blocked_id = $1))
        ORDER BY COALESCE(cv.last_message_at, cv.created_at) DESC LIMIT 100`,
      [yo.id]
    );
    const lista = filas.rows.map((f) => ({
      id: Number(f.id),
      username: f.username,
      display_name: f.display_name || f.username,
      avatar_url: f.avatar_url,
      is_verified: !!f.is_verified,
      last_message: f.last_message,
      updated_at: f.last_message_at,
      unread: Number(f.unread),
      silenciada: !!f.silenciada,
      archivada: !!f.archivada,
      oculta: !!f.oculta,
    }));
    if (filtro === 'sin_leer') return lista.filter((cv) => cv.unread > 0 && !cv.archivada);
    if (filtro === 'archivadas') return lista.filter((cv) => cv.archivada);
    return lista.filter((cv) => !cv.oculta && !cv.archivada);
  });

  // Silenciar, archivar u ocultar una conversación (solo cambia para quien lo pide).
  router.post('/api/messages/conversations/:id/prefs', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const conv = await uno(c.pool, 'SELECT id, user_a, user_b FROM conversations WHERE id = $1', [id]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (Number(conv.user_a) !== Number(yo.id) && Number(conv.user_b) !== Number(yo.id)) {
      throw new ApiErr('Esa conversación no es tuya', 403);
    }
    const b = await c.cuerpo();
    const antes = await prefsDe(c.pool, id, yo.id);
    const silenciada = 'silenciada' in b ? !!b.silenciada : antes.silenciada;
    const archivada = 'archivada' in b ? !!b.archivada : antes.archivada;
    const oculta = 'oculta' in b ? !!b.oculta : antes.oculta;
    await c.pool.query(
      `INSERT INTO conversation_prefs (conversation_id, user_id, silenciada, archivada, oculta, updated_at)
       VALUES ($1, $2, $3, $4, $5, NOW())
       ON CONFLICT (conversation_id, user_id)
       DO UPDATE SET silenciada = $3, archivada = $4, oculta = $5, updated_at = NOW()`,
      [id, yo.id, silenciada, archivada, oculta]
    );
    return { silenciada, archivada, oculta };
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
         SELECT id, conversation_id, sender_id, content, image_url, audio_url, duracion_ms,
                status, reaction, read_at, created_at::text AS created_at
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
    const audio = typeof b.audio_url === 'string' && /^\/api\/media\/[A-Za-z0-9._-]+$/.test(b.audio_url) ? b.audio_url : '';
    const duracion = Math.min(Math.max(Number(b.duracion_ms || 0), 0), 600_000);
    if (!contenido && !imagen && !audio) throw new ApiErr('Escribe un mensaje o manda una nota de voz', 400);

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
      `INSERT INTO messages (conversation_id, sender_id, content, image_url, audio_url, duracion_ms)
       VALUES ($1, $2, $3, $4, $5, $6) RETURNING *`,
      [conv.id, yo.id, contenido, imagen, audio, audio ? duracion : 0]
    );
    const resumen = contenido || (audio ? '🎤 Nota de voz' : '📷 Imagen');
    await c.pool.query(
      'UPDATE conversations SET last_message = $1, last_sender_id = $2, last_message_at = NOW() WHERE id = $3',
      [resumen.slice(0, 200), yo.id, conv.id]
    );
    await sumarEstadistica(c.pool, 'new_messages');

    const publico = mensajePublico(creado);
    enviarA(otroId, { type: 'message', conversation_id: Number(conv.id), message: publico });

    // Si el otro la había ocultado o archivado, un mensaje nuevo la devuelve a la lista.
    await c.pool.query(
      `INSERT INTO conversation_prefs (conversation_id, user_id, oculta, archivada, updated_at)
       VALUES ($1, $2, FALSE, FALSE, NOW())
       ON CONFLICT (conversation_id, user_id)
       DO UPDATE SET oculta = FALSE, archivada = FALSE, updated_at = NOW()`,
      [conv.id, otroId]
    ).catch(() => {});

    // Aviso al teléfono de quien recibe (llega con Moon cerrado).
    const quien = await uno(c.pool, 'SELECT display_name, username FROM users WHERE id = $1', [yo.id]);
    empujarSiQuiere(c.pool, otroId, 'message', {
      titulo: quien?.display_name || quien?.username || 'Moon',
      texto: contenido ? contenido.slice(0, 140) : (audio ? 'Te envió una nota de voz' : 'Te envió una imagen'),
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
