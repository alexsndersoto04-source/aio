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

// Columnas que traen, además del mensaje, el mensaje al que responde.
export const COLUMNAS_CON_RESPUESTA = `m.id, m.conversation_id, m.sender_id, m.content, m.image_url,
  m.audio_url, m.duracion_ms, m.status, m.reaction, m.read_at, m.created_at::text AS created_at, m.kind,
  m.edited_at::text AS edited_at, m.post_id, m.reply_to_id,
  r.content AS cita_texto, r.sender_id AS cita_de, r.status AS cita_estado,
  ru.display_name AS cita_nombre, ru.username AS cita_usuario,
  pc.content AS post_texto, pc.status AS post_estado, pc.user_id AS post_autor_id,
  pu.username AS post_usuario, pu.display_name AS post_nombre,
  (SELECT pi.original_url FROM post_images pi WHERE pi.post_id = pc.id ORDER BY pi.position LIMIT 1) AS post_imagen`;

function mensajePublico(m) {
  return {
    id: Number(m.id),
    conversation_id: Number(m.conversation_id),
    sender_id: Number(m.sender_id),
    content: m.status === 'deleted' ? '' : m.content,
    image_url: m.image_url || '',
    audio_url: m.status === 'deleted' ? '' : (m.audio_url || ''),
    duracion_ms: Number(m.duracion_ms || 0),
    // «kind» distingue una llamada (voz o video) de un mensaje normal.
    kind: m.kind || '',
    status: m.status,
    reaction: m.reaction || null,
    read_at: m.read_at ? String(m.read_at) : null,
    edited_at: m.edited_at ? String(m.edited_at) : null,
    created_at: String(m.created_at),
    // Publicación compartida al chat: una tarjeta con lo justo.
    post: m.post_id
      ? {
        id: Number(m.post_id),
        content: m.post_estado === 'deleted' ? '' : (m.post_texto || ''),
        autor: m.post_nombre || m.post_usuario || '',
        username: m.post_usuario || '',
        imagen: m.post_imagen || '',
      }
      : null,
    // Respuesta citada: lo justo para pintarla (quién y qué decía).
    reply_to: m.reply_to_id
      ? {
        id: Number(m.reply_to_id),
        sender_id: m.cita_de ? Number(m.cita_de) : null,
        autor: m.cita_nombre || m.cita_usuario || '',
        content: m.cita_estado === 'deleted' ? '' : (m.cita_texto || ''),
      }
      : null,
  };
}

// ---------- Llamadas: la dirección del «puente» ----------
// El servidor solo presenta a los dos teléfonos; la voz viaja entre ellos.
// «STUN» es la guía de direcciones (gratis, de Google). «TURN» es el puente
// que repite la voz cuando la operadora no deja que los dos teléfonos se vean:
// si algún día se pone un puente propio, basta con rellenar TURN_URLS,
// TURN_USUARIO y TURN_CLAVE en el panel del servidor, sin tocar el código.
function servidoresDeConexion() {
  const lista = [
    { urls: ['stun:stun.l.google.com:19302', 'stun:stun1.l.google.com:19302'] },
  ];
  const urls = String(process.env.TURN_URLS || '').split(',').map((u) => u.trim()).filter(Boolean);
  const usuario = String(process.env.TURN_USUARIO || '').trim();
  const clave = String(process.env.TURN_CLAVE || '').trim();
  if (urls.length > 0 && usuario) {
    lista.push({ urls, username: usuario, credential: clave });
  } else {
    // Puente público de siempre: funciona en port 80 y 443 (atraviesa
    // cortafuegos). Si algún día deja de responder, las llamadas siguen
    // funcionando cuando los dos teléfonos se ven directo.
    lista.push({
      urls: ['turn:openrelay.metered.ca:80', 'turn:openrelay.metered.ca:443', 'turn:openrelay.metered.ca:443?transport=tcp'],
      username: 'openrelayproject',
      credential: 'openrelayproject',
    });
  }
  return lista;
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
              COALESCE(pr.no_leida, FALSE) AS no_leida,
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
      no_leida: !!f.no_leida,
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
    if (!Number.isInteger(objetivo) || objetivo <= 0) throw new ApiErr('Indica a quién le escribes', 400, 'sin_destino');
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
    const idConv = Number(c.params.id);
    if (!Number.isInteger(idConv) || idConv <= 0) throw new ApiErr('Conversación no encontrada', 404);
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [idConv]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (![Number(conv.user_a), Number(conv.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Conversación no encontrada', 404);
    }
    const otroId = Number(conv.user_a) === Number(yo.id) ? Number(conv.user_b) : Number(conv.user_a);
    const otro = await uno(c.pool, 'SELECT * FROM users WHERE id = $1', [otroId]);
    const { limit } = paginacion(c.req, 50, 100);
    const mensajes = await c.pool.query(
      `SELECT * FROM (
         SELECT ${COLUMNAS_CON_RESPUESTA}
           FROM messages m
           LEFT JOIN messages r ON r.id = m.reply_to_id
           LEFT JOIN users ru ON ru.id = r.sender_id
           LEFT JOIN posts pc ON pc.id = m.post_id
           LEFT JOIN users pu ON pu.id = pc.user_id
          WHERE m.conversation_id = $1 ORDER BY m.created_at DESC LIMIT ${limit}
       ) AS recientes ORDER BY created_at ASC`,
      [conv.id]
    );
    // Mensaje fijado (si lo hay): se manda aparte para pintarlo arriba.
    let fijado = null;
    if (conv.pinned_message_id) {
      const f = await uno(
        c.pool,
        `SELECT ${COLUMNAS_CON_RESPUESTA} FROM messages m
           LEFT JOIN messages r ON r.id = m.reply_to_id
           LEFT JOIN users ru ON ru.id = r.sender_id
           LEFT JOIN posts pc ON pc.id = m.post_id
           LEFT JOIN users pu ON pu.id = pc.user_id
          WHERE m.id = $1`,
        [conv.pinned_message_id]
      );
      if (f) fijado = mensajePublico(f);
    }
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
      pinned: fijado,
      typing: false,
    };
  });

  // Direcciones de conexión para una llamada (lo consulta la app al abrir el chat).
  router.get('/api/llamadas/config', async (c) => {
    await c.exigir();
    return { iceServers: servidoresDeConexion() };
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
    const respondeA = Number(b.reply_to_id) > 0 ? Number(b.reply_to_id) : null;
    // Publicación compartida al chat (llega como tarjeta, no como enlace suelto).
    const publicacion = Number(b.post_id) > 0 ? Number(b.post_id) : null;
    // Registro de una llamada en el chat: el texto lo pone el servidor, para
    // que nadie pueda escribir cualquier cosa en su lugar.
    const tiposLlamada = { llamada_voz: 'Llamada de voz', llamada_video: 'Videollamada', llamada_perdida: 'Llamada perdida' };
    const clase = Object.prototype.hasOwnProperty.call(tiposLlamada, b.kind) ? String(b.kind) : '';
    const esLlamada = !!clase;
    if (!contenido && !imagen && !audio && !publicacion && !esLlamada) {
      throw new ApiErr('Escribe un mensaje o manda una nota de voz', 400);
    }

    const idConv = Number(c.params.id);
    if (!Number.isInteger(idConv) || idConv <= 0) throw new ApiErr('Conversación no encontrada', 404);
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [idConv]);
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

    // Solo se puede responder a un mensaje de esta misma conversación.
    if (respondeA) {
      const objetivo = await uno(c.pool, 'SELECT id, conversation_id FROM messages WHERE id = $1', [respondeA]);
      if (!objetivo || Number(objetivo.conversation_id) !== Number(conv.id)) {
        throw new ApiErr('No puedes responder a ese mensaje', 400, 'cita_invalida');
      }
    }

    if (publicacion) {
      const existe = await uno(c.pool, "SELECT id FROM posts WHERE id = $1 AND status = 'active'", [publicacion]);
      if (!existe) throw new ApiErr('Esa publicación ya no está disponible', 404, 'post_no_existe');
    }

    const creado = await uno(
      c.pool,
      `INSERT INTO messages (conversation_id, sender_id, content, image_url, audio_url, duracion_ms, reply_to_id, post_id, kind)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *`,
      [conv.id, yo.id, esLlamada ? tiposLlamada[clase] : contenido, imagen, audio,
        audio || esLlamada ? duracion : 0, respondeA, publicacion, clase]
    );
    const resumen = esLlamada
      ? `${clase === 'llamada_video' ? '📹' : '📞'} ${tiposLlamada[clase]}`
      : (contenido || (publicacion ? '📎 Publicación compartida' : audio ? '🎤 Nota de voz' : '📷 Imagen'));
    await c.pool.query(
      'UPDATE conversations SET last_message = $1, last_sender_id = $2, last_message_at = NOW() WHERE id = $3',
      [resumen.slice(0, 200), yo.id, conv.id]
    );
    await sumarEstadistica(c.pool, 'new_messages');

    // Se relee con la cita ya unida para que el receptor la vea igual.
    const conCita = await uno(
      c.pool,
      `SELECT ${COLUMNAS_CON_RESPUESTA} FROM messages m
         LEFT JOIN messages r ON r.id = m.reply_to_id
         LEFT JOIN users ru ON ru.id = r.sender_id
         LEFT JOIN posts pc ON pc.id = m.post_id
         LEFT JOIN users pu ON pu.id = pc.user_id
        WHERE m.id = $1`,
      [creado.id]
    );
    const publico = mensajePublico(conCita || creado);
    enviarA(otroId, { type: 'message', conversation_id: Number(conv.id), message: publico });
    // Las filas de llamada se avisan también a quien las escribió: su teléfono
    // no recibe sus propios mensajes, y así la fila le sale igual que al otro
    // lado, sin depender de nada más.
    if (esLlamada) {
      enviarA(Number(yo.id), { type: 'message', conversation_id: Number(conv.id), message: publico });
    }

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
      texto: contenido ? contenido.slice(0, 140)
        : publicacion ? 'Te compartió una publicación'
        : audio ? 'Te envió una nota de voz' : 'Te envió una imagen',
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
    await c.pool.query(
      `INSERT INTO conversation_prefs (conversation_id, user_id, no_leida, updated_at)
       VALUES ($1, $2, FALSE, NOW())
       ON CONFLICT (conversation_id, user_id) DO UPDATE SET no_leida = FALSE, updated_at = NOW()`,
      [conv.id, yo.id]
    ).catch(() => {});
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

  // ---------- Editar un mensaje enviado ----------
  router.patch('/api/messages/:id', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    if (!Number.isInteger(id) || id <= 0) throw new ApiErr('Mensaje no encontrado', 404);
    const b = await c.cuerpo();
    const contenido = texto(b.content || '', { min: 1, max: 2000, campo: 'mensaje' }).trim();
    if (!contenido) throw new ApiErr('El mensaje no puede quedar vacío', 400);

    const m = await uno(c.pool, 'SELECT * FROM messages WHERE id = $1', [id]);
    if (!m) throw new ApiErr('Mensaje no encontrado', 404);
    if (Number(m.sender_id) !== Number(yo.id)) throw new ApiErr('Solo puedes editar tus mensajes', 403, 'no_es_tuyo');
    if (m.status === 'deleted') throw new ApiErr('Ese mensaje está eliminado', 400, 'eliminado');

    await c.pool.query('UPDATE messages SET content = $1, edited_at = NOW() WHERE id = $2', [contenido, id]);
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [m.conversation_id]);
    const otroId = Number(conv.user_a) === Number(yo.id) ? Number(conv.user_b) : Number(conv.user_a);

    const completo = await uno(
      c.pool,
      `SELECT ${COLUMNAS_CON_RESPUESTA} FROM messages m
         LEFT JOIN messages r ON r.id = m.reply_to_id
         LEFT JOIN users ru ON ru.id = r.sender_id
         LEFT JOIN posts pc ON pc.id = m.post_id
         LEFT JOIN users pu ON pu.id = pc.user_id
        WHERE m.id = $1`,
      [id]
    );
    const publico = mensajePublico(completo);
    // El último mensaje de la lista cambia si era este.
    await c.pool.query(
      `UPDATE conversations SET last_message = $1 WHERE id = $2 AND last_sender_id = $3 AND last_message_at IS NOT NULL`,
      [contenido.slice(0, 200), conv.id, yo.id]
    ).catch(() => {});
    enviarA(otroId, { type: 'message_edited', conversation_id: Number(conv.id), message: publico });
    return publico;
  });

  // ---------- Reenviar a otra conversación ----------
  router.post('/api/messages/:id/forward', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const b = await c.cuerpo();
    const destino = Number(b.conversation_id);
    if (!Number.isInteger(destino) || destino <= 0) throw new ApiErr('Elige una conversación', 400, 'sin_destino');

    const m = await uno(c.pool, 'SELECT * FROM messages WHERE id = $1', [id]);
    if (!m) throw new ApiErr('Mensaje no encontrado', 404);
    if (m.status === 'deleted') throw new ApiErr('Ese mensaje está eliminado', 400, 'eliminado');
    const origen = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [m.conversation_id]);
    const soyDeOrigen = [Number(origen.user_a), Number(origen.user_b)].includes(Number(yo.id));
    if (!soyDeOrigen) throw new ApiErr('Mensaje no encontrado', 404);

    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [destino]);
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
    if (bloqueo) throw new ApiErr('No puedes escribir en esa conversación', 403, 'blocked');

    const creado = await uno(
      c.pool,
      `INSERT INTO messages (conversation_id, sender_id, content, image_url, audio_url, duracion_ms, post_id)
       VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *`,
      [conv.id, yo.id, m.content || '', m.image_url || '', m.audio_url || '', Number(m.duracion_ms || 0), m.post_id || null]
    );
    const resumen = m.content || (m.post_id ? '📎 Publicación compartida' : m.audio_url ? '🎤 Nota de voz' : '📷 Imagen');
    await c.pool.query(
      'UPDATE conversations SET last_message = $1, last_sender_id = $2, last_message_at = NOW() WHERE id = $3',
      [String(resumen).slice(0, 200), yo.id, conv.id]
    );
    await c.pool.query(
      `INSERT INTO conversation_prefs (conversation_id, user_id, oculta, archivada, updated_at)
       VALUES ($1, $2, FALSE, FALSE, NOW())
       ON CONFLICT (conversation_id, user_id) DO UPDATE SET oculta = FALSE, archivada = FALSE, updated_at = NOW()`,
      [conv.id, otroId]
    ).catch(() => {});

    const completo = await uno(
      c.pool,
      `SELECT ${COLUMNAS_CON_RESPUESTA} FROM messages m
         LEFT JOIN messages r ON r.id = m.reply_to_id
         LEFT JOIN users ru ON ru.id = r.sender_id
         LEFT JOIN posts pc ON pc.id = m.post_id
         LEFT JOIN users pu ON pu.id = pc.user_id
        WHERE m.id = $1`,
      [creado.id]
    );
    const publico = mensajePublico(completo);
    enviarA(otroId, { type: 'message', conversation_id: Number(conv.id), message: publico });
    await auditar(c.pool, Number(yo.id), 'mensaje_reenviado', `#${id} → #${conv.id}`, c.ip).catch(() => {});
    return publico;
  });

  // ---------- Fijar (o soltar) un mensaje de la conversación ----------
  router.post('/api/messages/:id/pin', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const m = await uno(c.pool, 'SELECT * FROM messages WHERE id = $1', [id]);
    if (!m) throw new ApiErr('Mensaje no encontrado', 404);
    if (m.status === 'deleted') throw new ApiErr('Ese mensaje está eliminado', 400, 'eliminado');
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [m.conversation_id]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (![Number(conv.user_a), Number(conv.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Conversación no encontrada', 404);
    }
    const fijar = Number(conv.pinned_message_id) !== id;
    await c.pool.query('UPDATE conversations SET pinned_message_id = $2 WHERE id = $1', [conv.id, fijar ? id : null]);
    const otroId = Number(conv.user_a) === Number(yo.id) ? Number(conv.user_b) : Number(conv.user_a);
    enviarA(otroId, { type: 'message_pinned', conversation_id: Number(conv.id), message_id: fijar ? id : null });
    return { ok: true, fijado: fijar, message_id: fijar ? id : null };
  });

  // ---------- Marcar la conversación como no leída ----------
  router.post('/api/messages/conversations/:id/no-leida', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [id]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (![Number(conv.user_a), Number(conv.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Conversación no encontrada', 404);
    }
    const b = await c.cuerpo().catch(() => ({}));
    const noLeida = b.no === undefined ? true : !!b.no;
    await c.pool.query(
      `INSERT INTO conversation_prefs (conversation_id, user_id, no_leida, oculta, updated_at)
       VALUES ($1, $2, $3, FALSE, NOW())
       ON CONFLICT (conversation_id, user_id)
       DO UPDATE SET no_leida = $3, oculta = FALSE, updated_at = NOW()`,
      [id, yo.id, noLeida]
    );
    return { ok: true, no_leida: noLeida };
  });

  // ---------- Borrar la conversación (solo para mí) ----------
  router.del('/api/messages/conversations/:id', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const conv = await uno(c.pool, 'SELECT * FROM conversations WHERE id = $1', [id]);
    if (!conv) throw new ApiErr('Conversación no encontrada', 404);
    if (![Number(conv.user_a), Number(conv.user_b)].includes(Number(yo.id))) {
      throw new ApiErr('Conversación no encontrada', 404);
    }
    // No se borra nada de la otra persona: solo desaparece de tu lista.
    await c.pool.query(
      `INSERT INTO conversation_prefs (conversation_id, user_id, oculta, no_leida, updated_at)
       VALUES ($1, $2, TRUE, FALSE, NOW())
       ON CONFLICT (conversation_id, user_id)
       DO UPDATE SET oculta = TRUE, no_leida = FALSE, updated_at = NOW()`,
      [id, yo.id]
    );
    await auditar(c.pool, Number(yo.id), 'conversacion_borrada', `#${id}`, c.ip).catch(() => {});
    return { ok: true };
  });
}
