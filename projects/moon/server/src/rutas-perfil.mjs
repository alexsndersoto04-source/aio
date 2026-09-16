// Moon — El perfil completo (tanda 4)
// ============================================================
// · Listas de seguidores y seguidos, abiertas y con nombre y cara.
// · Cuenta privada: quien quiere seguirte pide permiso (solicitudes).
// · Mis me gusta y mis comentarios, para volver a encontrarlos.
// · Historial de búsqueda, que se puede borrar entero o de a poco.
// · Seguir etiquetas (#diseño) y verlas en tu rincón.
// · Ajustes: idioma, oscuro por horario, ahorro de datos, PIN y
//   exportar todo lo mío en un archivo.
//
// Nada se guarda en claro que pueda servir para entrar: el PIN se
// guarda con hash (el mismo del resto de contraseñas).

import { ApiErr, paginacion, texto } from './util.mjs';
import { uno, filas } from './db.mjs';
import { auditar } from './db.mjs';
import { perfilPublico } from './rutas-social.mjs';
import { hashPassword, verificarPassword } from './auth.mjs';

/** Idiomas que la interfaz sabe mostrar. */
const IDIOMAS = new Set(['es', 'en', 'pt']);

/** Los ajustes de una persona, creando la fila la primera vez. */
async function ajustesDe(pool, userId) {
  const fila = await uno(
    pool,
    `INSERT INTO user_settings (user_id) VALUES ($1)
     ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
     RETURNING user_id, idioma, tema_auto, tema_desde, tema_hasta, ahorro_datos,
               avisos_tipos, bloqueo_activo, pin_hash, pin_hash <> '' AS tiene_pin`,
    [userId]
  );
  return fila;
}

function aAjustes(a) {
  return {
    idioma: a.idioma || 'es',
    tema_auto: !!a.tema_auto,
    tema_desde: a.tema_desde || '20:00',
    tema_hasta: a.tema_hasta || '07:00',
    ahorro_datos: !!a.ahorro_datos,
    avisos_tipos: a.avisos_tipos || {},
    bloqueo_activo: !!a.bloqueo_activo,
    tiene_pin: !!a.tiene_pin,
  };
}

export function registrarRutasPerfil(router) {
  // ============================================================
  // Listas: seguidores y seguidos
  // ============================================================
  router.get('/api/users/:id/followers', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const { limit, offset } = paginacion(c.query, 40, 100);
    const gente = await filas(
      c.pool,
      `SELECT u.*, (mi.follower_id IS NOT NULL) AS le_sigo
         FROM follows f
         JOIN users u ON u.id = f.follower_id
         LEFT JOIN follows mi ON mi.follower_id = $1 AND mi.following_id = u.id
        WHERE f.following_id = $2
        ORDER BY f.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id, id]
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS n FROM follows WHERE following_id = $1', [id]);
    return {
      total: Number(total?.n || 0),
      items: gente.map((u) => ({ ...perfilPublico(u), le_sigo: !!u.le_sigo, soy_yo: Number(u.id) === Number(yo.id) })),
    };
  });

  router.get('/api/users/:id/following', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const { limit, offset } = paginacion(c.query, 40, 100);
    const gente = await filas(
      c.pool,
      `SELECT u.*, (mi.follower_id IS NOT NULL) AS le_sigo
         FROM follows f
         JOIN users u ON u.id = f.following_id
         LEFT JOIN follows mi ON mi.follower_id = $1 AND mi.following_id = u.id
        WHERE f.follower_id = $2
        ORDER BY f.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id, id]
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS n FROM follows WHERE follower_id = $1', [id]);
    return {
      total: Number(total?.n || 0),
      items: gente.map((u) => ({ ...perfilPublico(u), le_sigo: !!u.le_sigo, soy_yo: Number(u.id) === Number(yo.id) })),
    };
  });

  // ============================================================
  // Cuenta privada: solicitudes para seguir
  // ============================================================
  // Lo que me pidieron a mí (solo yo lo veo).
  router.get('/api/me/solicitudes', async (c) => {
    const yo = await c.exigir();
    const lista = await filas(
      c.pool,
      `SELECT s.id AS solicitud_id, s.created_at::text AS pedida, u.*
         FROM follow_requests s JOIN users u ON u.id = s.solicitante_id
        WHERE s.destino_id = $1 AND s.estado = 'pendiente'
        ORDER BY s.created_at ASC LIMIT 100`,
      [yo.id]
    );
    return {
      solicitudes: lista.map((u) => ({
        ...perfilPublico(u),
        solicitud_id: Number(u.solicitud_id),
        created_at: u.pedida,
      })),
    };
  });

  router.post('/api/me/solicitudes/:id', async (c) => {
    const yo = await c.exigir();
    const solicitudId = Number(c.params.id);
    const b = await c.cuerpo().catch(() => ({}));
    const aceptar = b?.aceptar !== false;
    const s = await uno(
      c.pool,
      `SELECT id, solicitante_id, estado FROM follow_requests WHERE id = $1 AND destino_id = $2`,
      [solicitudId, yo.id]
    );
    if (!s) throw new ApiErr('Esa solicitud no existe', 404, 'solicitud_no_existe');
    if (s.estado !== 'pendiente') throw new ApiErr('Esa solicitud ya se resolvió', 409, 'ya_resuelta');

    await c.pool.query(
      `UPDATE follow_requests SET estado = $2, resolved_at = NOW() WHERE id = $1`,
      [solicitudId, aceptar ? 'aceptada' : 'rechazada']
    );
    if (aceptar) {
      const r = await c.pool.query(
        'INSERT INTO follows (follower_id, following_id) VALUES ($1, $2) ON CONFLICT DO NOTHING',
        [s.solicitante_id, yo.id]
      );
      if (r.rowCount > 0) {
        await c.pool.query('UPDATE users SET following_count = following_count + 1 WHERE id = $1', [s.solicitante_id]);
        await c.pool.query('UPDATE users SET followers_count = followers_count + 1 WHERE id = $1', [yo.id]);
      }
    }
    await auditar(c.pool, Number(yo.id), aceptar ? 'seguidor_aceptado' : 'seguidor_rechazado', `#${s.solicitante_id}`, c.ip).catch(() => {});
    return { ok: true, aceptada: aceptar };
  });

  // Borrar una solicitud pendiente sin más (por si me arrepiento de tenerla ahí).
  router.del('/api/me/solicitudes/:id', async (c) => {
    const yo = await c.exigir();
    await c.pool.query(
      `DELETE FROM follow_requests WHERE id = $1 AND destino_id = $2 AND estado = 'pendiente'`,
      [Number(c.params.id), yo.id]
    );
    return { ok: true };
  });

  // ============================================================
  // Mis me gusta y mis comentarios
  // ============================================================
  router.get('/api/me/likes', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.query, 20, 50);
    const publicaciones = await filas(
      c.pool,
      `SELECT p.id FROM likes l JOIN posts p ON p.id = l.post_id
        WHERE l.user_id = $1 AND p.status <> 'deleted'
        ORDER BY l.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id]
    );
    const total = await uno(
      c.pool,
      `SELECT COUNT(*)::int AS n FROM likes l JOIN posts p ON p.id = l.post_id WHERE l.user_id = $1 AND p.status <> 'deleted'`,
      [yo.id]
    );
    const { conContenido, aPublicacion, SQL_POST } = await import('./rutas-social.mjs');
    const ids = publicaciones.map((p) => Number(p.id));
    let items = [];
    if (ids.length > 0) {
      const conTodo = await filas(c.pool, `${SQL_POST} WHERE p.id = ANY($2::bigint[])`, [yo.id, ids]);
      const orden = new Map(ids.map((id, i) => [id, i]));
      const completas = await conContenido(c.pool, conTodo, yo.id);
      items = completas.map((p) => aPublicacion(p, yo.id)).sort((a, b) => orden.get(a.id) - orden.get(b.id));
    }
    return { items, total: Number(total?.n || 0), page, limit };
  });

  router.get('/api/me/comments', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.query, 20, 50);
    const lista = await filas(
      c.pool,
      `SELECT co.id, co.content, co.created_at::text AS created_at, co.post_id,
              p.content AS post_texto, p.user_id AS post_autor,
              u.username AS autor_username, u.display_name AS autor_nombre
         FROM comments co
         JOIN posts p ON p.id = co.post_id
         JOIN users u ON u.id = p.user_id
        WHERE co.user_id = $1 AND co.status <> 'deleted' AND p.status <> 'deleted'
        ORDER BY co.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id]
    );
    const total = await uno(
      c.pool,
      `SELECT COUNT(*)::int AS n FROM comments co JOIN posts p ON p.id = co.post_id
        WHERE co.user_id = $1 AND co.status <> 'deleted' AND p.status <> 'deleted'`,
      [yo.id]
    );
    return {
      total: Number(total?.n || 0),
      page,
      limit,
      items: lista.map((r) => ({
        id: Number(r.id),
        content: r.content || '',
        created_at: r.created_at,
        post_id: Number(r.post_id),
        post_autor: r.autor_nombre || r.autor_username || '',
        post_autor_username: r.autor_username || '',
        post_texto: (r.post_texto || '').slice(0, 160),
      })),
    };
  });

  // ============================================================
  // Historial de búsqueda
  // ============================================================
  router.get('/api/me/busquedas', async (c) => {
    const yo = await c.exigir();
    const lista = await filas(
      c.pool,
      `SELECT id, termino, tipo, created_at::text AS created_at FROM search_history
        WHERE user_id = $1 ORDER BY created_at DESC LIMIT 30`,
      [yo.id]
    );
    return {
      busquedas: lista.map((b) => ({
        id: Number(b.id), termino: b.termino, tipo: b.tipo, created_at: b.created_at,
      })),
    };
  });

  // Se guarda al buscar. Repetir un término lo sube arriba sin duplicarlo.
  router.post('/api/me/busquedas', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo().catch(() => ({}));
    const termino = texto(String(b?.termino || ''), { min: 1, max: 80, campo: 'búsqueda' }).trim();
    if (!termino) throw new ApiErr('Escribe qué buscaste', 400, 'sin_termino');
    const tipo = ['users', 'posts', 'tags'].includes(b?.tipo) ? b.tipo : 'users';
    await c.pool.query('DELETE FROM search_history WHERE user_id = $1 AND termino = $2', [yo.id, termino]);
    await c.pool.query(
      'INSERT INTO search_history (user_id, termino, tipo) VALUES ($1, $2, $3)',
      [yo.id, termino, tipo]
    );
    // Solo se guardan las últimas 40: el historial no crece sin fin.
    await c.pool.query(
      `DELETE FROM search_history WHERE user_id = $1 AND id NOT IN (
         SELECT id FROM search_history WHERE user_id = $1 ORDER BY created_at DESC LIMIT 40)`,
      [yo.id]
    );
    return { ok: true };
  });

  router.del('/api/me/busquedas/:id', async (c) => {
    const yo = await c.exigir();
    await c.pool.query('DELETE FROM search_history WHERE id = $1 AND user_id = $2', [Number(c.params.id), yo.id]);
    return { ok: true };
  });

  router.del('/api/me/busquedas', async (c) => {
    const yo = await c.exigir();
    await c.pool.query('DELETE FROM search_history WHERE user_id = $1', [yo.id]);
    return { ok: true, borradas: true };
  });

  // ============================================================
  // Seguir etiquetas
  // ============================================================
  const limpiarTag = (v) => String(v || '').replace(/[^0-9a-zA-ZáéíóúüñÁÉÍÓÚÜÑ_]/g, '').toLowerCase().slice(0, 60);

  router.get('/api/me/hashtags', async (c) => {
    const yo = await c.exigir();
    const lista = await filas(
      c.pool,
      `SELECT h.tag, h.created_at::text AS created_at,
              (SELECT COUNT(*)::int FROM posts p WHERE p.status <> 'deleted' AND p.content ILIKE '%#' || h.tag || '%') AS posts_count
         FROM hashtag_follows h WHERE h.user_id = $1 ORDER BY h.created_at DESC LIMIT 40`,
      [yo.id]
    );
    return {
      hashtags: lista.map((h) => ({ tag: h.tag, posts_count: Number(h.posts_count || 0), desde: h.created_at })),
    };
  });

  router.post('/api/hashtags/:tag/follow', async (c) => {
    const yo = await c.exigir();
    const tag = limpiarTag(c.params.tag);
    if (!tag) throw new ApiErr('Esa etiqueta no es válida', 400, 'tag_invalida');
    await c.pool.query(
      'INSERT INTO hashtag_follows (user_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING',
      [yo.id, tag]
    );
    return { ok: true, siguiendo: true, tag };
  });

  router.del('/api/hashtags/:tag/follow', async (c) => {
    const yo = await c.exigir();
    const tag = limpiarTag(c.params.tag);
    await c.pool.query('DELETE FROM hashtag_follows WHERE user_id = $1 AND tag = $2', [yo.id, tag]);
    return { ok: true, siguiendo: false, tag };
  });

  // El feed de mis etiquetas: lo último de las que sigo.
  router.get('/api/feed/etiquetas', async (c) => {
    const yo = await c.exigir();
    const { limit } = paginacion(c.query, 20, 50);
    const { conContenido, aPublicacion, SQL_POST } = await import('./rutas-social.mjs');
    const publicaciones = await filas(
      c.pool,
      `${SQL_POST}
        WHERE p.status <> 'deleted'
          AND EXISTS (
            SELECT 1 FROM hashtag_follows h
             WHERE h.user_id = $1 AND p.content ILIKE '%#' || h.tag || '%'
          )
        ORDER BY p.created_at DESC LIMIT ${limit}`,
      [yo.id]
    );
    const completas = await conContenido(c.pool, publicaciones, yo.id);
    return { items: completas.map((p) => aPublicacion(p, yo.id)) };
  });

  // ============================================================
  // Ajustes: idioma, oscuro por horario, ahorro de datos, PIN
  // ============================================================
  router.get('/api/me/ajustes', async (c) => {
    const yo = await c.exigir();
    return aAjustes(await ajustesDe(c.pool, yo.id));
  });

  router.patch('/api/me/ajustes', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const idioma = IDIOMAS.has(b.idioma) ? b.idioma : null;
    const horaValida = (v) => (typeof v === 'string' && /^([01]\d|2[0-3]):[0-5]\d$/.test(v) ? v : null);
    const desde = horaValida(b.tema_desde);
    const hasta = horaValida(b.tema_hasta);
    const tipos = b.avisos_tipos && typeof b.avisos_tipos === 'object' ? JSON.stringify(b.avisos_tipos) : null;

    const actualizado = await uno(
      c.pool,
      `UPDATE user_settings SET
          idioma = COALESCE($2, idioma),
          tema_auto = COALESCE($3, tema_auto),
          tema_desde = COALESCE($4, tema_desde),
          tema_hasta = COALESCE($5, tema_hasta),
          ahorro_datos = COALESCE($6, ahorro_datos),
          avisos_tipos = COALESCE($7::jsonb, avisos_tipos),
          updated_at = NOW()
        WHERE user_id = $1
        RETURNING user_id, idioma, tema_auto, tema_desde, tema_hasta, ahorro_datos,
                  avisos_tipos, bloqueo_activo, pin_hash <> '' AS tiene_pin`,
      [
        yo.id,
        idioma,
        typeof b.tema_auto === 'boolean' ? b.tema_auto : null,
        desde,
        hasta,
        typeof b.ahorro_datos === 'boolean' ? b.ahorro_datos : null,
        tipos,
      ]
    );
    await auditar(c.pool, Number(yo.id), 'ajustes_cambiados', '', c.ip).catch(() => {});
    return aAjustes(actualizado);
  });

  // PIN: se guarda con hash y nunca se devuelve.
  router.post('/api/me/ajustes/pin', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const pin = String(b.pin || '');
    if (pin && !/^\d{4,8}$/.test(pin)) throw new ApiErr('El PIN son de 4 a 8 números', 400, 'pin_invalido');
    await ajustesDe(c.pool, yo.id);
    if (!pin) {
      await c.pool.query(
        `UPDATE user_settings SET pin_hash = '', bloqueo_activo = FALSE, updated_at = NOW() WHERE user_id = $1`,
        [yo.id]
      );
      return { ok: true, tiene_pin: false, bloqueo_activo: false };
    }
    const actual = await ajustesDe(c.pool, yo.id);
    if (actual.pin_hash) {
      const correcto = await verificarPassword(actual.pin_hash, String(b.pin_actual || ''));
      if (!correcto) throw new ApiErr('El PIN actual no es ese', 403, 'pin_incorrecto');
    }
    await c.pool.query(
      `UPDATE user_settings SET pin_hash = $2, bloqueo_activo = TRUE, updated_at = NOW() WHERE user_id = $1`,
      [yo.id, await hashPassword(pin)]
    );
    return { ok: true, tiene_pin: true, bloqueo_activo: true };
  });

  router.post('/api/me/ajustes/pin/verificar', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const actual = await ajustesDe(c.pool, yo.id);
    if (!actual.pin_hash) return { ok: true, correcto: true, tiene_pin: false };
    const correcto = await verificarPassword(actual.pin_hash, String(b.pin || ''));
    if (!correcto) throw new ApiErr('PIN incorrecto', 403, 'pin_incorrecto');
    return { ok: true, correcto: true, tiene_pin: true };
  });

  router.post('/api/me/ajustes/pin/activar', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo().catch(() => ({}));
    await c.pool.query(
      'UPDATE user_settings SET bloqueo_activo = $2, updated_at = NOW() WHERE user_id = $1',
      [yo.id, b?.activo !== false]
    );
    return { ok: true, bloqueo_activo: b?.activo !== false };
  });

  // ============================================================
  // Exportar mis datos: un archivo con lo mío, sin lo de nadie más
  // ============================================================
  router.get('/api/me/exportar', async (c) => {
    const yo = await c.exigir();
    const [perfil, publicaciones, comentarios, mensajes, grupos, meGusta, seguidores, siguiendo, etiquetas, ajustes] = await Promise.all([
      uno(c.pool, 'SELECT id, username, display_name, email, bio, link, location, is_private, created_at::text AS created_at FROM users WHERE id = $1', [yo.id]),
      filas(c.pool, `SELECT id, content, created_at::text AS created_at FROM posts WHERE user_id = $1 AND status <> 'deleted' ORDER BY created_at DESC LIMIT 2000`, [yo.id]),
      filas(c.pool, `SELECT id, post_id, content, created_at::text AS created_at FROM comments WHERE user_id = $1 AND status <> 'deleted' ORDER BY created_at DESC LIMIT 2000`, [yo.id]),
      filas(c.pool, `SELECT m.id, m.conversation_id, m.content, m.created_at::text AS created_at FROM messages m JOIN conversations cv ON cv.id = m.conversation_id WHERE m.sender_id = $1 AND m.status <> 'deleted' ORDER BY m.created_at DESC LIMIT 2000`, [yo.id]),
      filas(c.pool, `SELECT g.id, g.name, g.privacy, gm.role FROM group_members gm JOIN groups g ON g.id = gm.group_id WHERE gm.user_id = $1`, [yo.id]),
      filas(c.pool, `SELECT post_id, created_at::text AS created_at FROM likes WHERE user_id = $1 ORDER BY created_at DESC LIMIT 2000`, [yo.id]),
      filas(c.pool, `SELECT u.username FROM follows f JOIN users u ON u.id = f.follower_id WHERE f.following_id = $1 LIMIT 2000`, [yo.id]),
      filas(c.pool, `SELECT u.username FROM follows f JOIN users u ON u.id = f.following_id WHERE f.follower_id = $1 LIMIT 2000`, [yo.id]),
      filas(c.pool, 'SELECT tag FROM hashtag_follows WHERE user_id = $1', [yo.id]),
      ajustesDe(c.pool, yo.id),
    ]);
    await auditar(c.pool, Number(yo.id), 'datos_exportados', '', c.ip).catch(() => {});
    return {
      aplicacion: 'Moon',
      exportado: new Date().toISOString(),
      perfil,
      publicaciones,
      comentarios,
      mensajes,
      grupos,
      me_gusta: meGusta,
      seguidores: seguidores.map((x) => x.username),
      siguiendo: siguiendo.map((x) => x.username),
      etiquetas: etiquetas.map((x) => x.tag),
      ajustes: aAjustes(ajustes),
    };
  });
}
