// Moon — Historias (24 horas) y presencia
// ============================================================
// Historias reales: una imagen con pie de texto que caduca a las 24 horas,
// con su registro de visitas. Y presencia real: quién está conectado ahora
// mismo, según las conexiones WebSocket abiertas.

import { ApiErr, texto } from './util.mjs';
import { uno, filas } from './db.mjs';
import { auditar } from './db.mjs';
import { usuariosConectados } from './ws.mjs';

export function registrarRutasHistorias(router) {
  // Las historias caducan a las 24 horas exactas y, al cumplirse, se borran
  // de la base (no solo se ocultan): esta limpieza corre en cada lectura.
  const purgarVencidas = (pool) => pool.query('DELETE FROM stories WHERE expires_at <= NOW()').catch(() => {});

  // ---------- Historias ----------
  router.get('/api/stories', async (c) => {
    const yo = await c.exigir();
    purgarVencidas(c.pool);
    // Se ven las historias propias y las de a quienes sigo.
    const grupos = await filas(
      c.pool,
      `SELECT s.user_id, u.username, u.display_name, u.avatar_url, u.is_verified,
              MAX(s.created_at)::text AS ultima,
              COUNT(*)::int AS total,
              COUNT(*) FILTER (WHERE sv.viewer_id IS NULL)::int AS sin_ver,
              (SELECT s2.image_url FROM stories s2 WHERE s2.user_id = s.user_id AND s2.expires_at > NOW() ORDER BY s2.created_at DESC LIMIT 1) AS ultima_imagen,
              (SELECT s2.caption FROM stories s2 WHERE s2.user_id = s.user_id AND s2.expires_at > NOW() ORDER BY s2.created_at DESC LIMIT 1) AS ultimo_caption
         FROM stories s
         JOIN users u ON u.id = s.user_id
         LEFT JOIN story_views sv ON sv.story_id = s.id AND sv.viewer_id = $1
        WHERE s.expires_at > NOW()
          AND (s.user_id = $1 OR EXISTS (SELECT 1 FROM follows f WHERE f.follower_id = $1 AND f.following_id = s.user_id))
          AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.blocker_id = $1 AND b.blocked_id = s.user_id) OR (b.blocker_id = s.user_id AND b.blocked_id = $1))
        GROUP BY s.user_id, u.username, u.display_name, u.avatar_url, u.is_verified
        ORDER BY (MAX(s.created_at)) DESC`,
      [yo.id]
    );
    // Las propias van primero (como en las redes grandes).
    grupos.sort((a, b) => (Number(a.user_id) === Number(yo.id) ? -1 : Number(b.user_id) === Number(yo.id) ? 1 : 0));
    return grupos.map((g) => ({
      user_id: Number(g.user_id),
      username: g.username,
      display_name: g.display_name || g.username,
      avatar_url: g.avatar_url,
      is_verified: !!g.is_verified,
      total: Number(g.total),
      sin_ver: Number(g.sin_ver),
      mine: Number(g.user_id) === Number(yo.id),
      created_at: g.ultima,
      image_url: g.ultima_imagen,
      caption: g.ultimo_caption,
    }));
  });

  router.get('/api/stories/:userId', async (c) => {
    const yo = await c.exigir();
    purgarVencidas(c.pool);
    const objetivo = Number(c.params.userId);
    const lista = await filas(
      c.pool,
      `SELECT s.id, s.user_id, s.image_url, s.caption, s.views_count,
              COALESCE(s.music_title, '') AS music_title,
              COALESCE(s.music_url, '') AS music_url,
              COALESCE(s.music_start_sec, 0) AS music_start_sec,
              COALESCE(s.music_duration_sec, 15) AS music_duration_sec,
              s.created_at::text AS created_at,
              (SELECT COUNT(*)::int FROM story_views sv WHERE sv.story_id = s.id AND sv.viewer_id = $2) > 0 AS vista,
              (SELECT sr.emoji FROM story_reactions sr WHERE sr.story_id = s.id AND sr.user_id = $2 LIMIT 1) AS mi_reaccion,
              (SELECT COUNT(*)::int FROM story_reactions sr WHERE sr.story_id = s.id) AS reactions_count,
              (SELECT COUNT(*)::int FROM story_comments sc WHERE sc.story_id = s.id) AS comments_count
         FROM stories s
        WHERE s.user_id = $1 AND s.expires_at > NOW()
        ORDER BY s.created_at ASC`,
      [objetivo, yo.id]
    );
    const autor = await uno(c.pool, 'SELECT username, display_name, avatar_url, is_verified FROM users WHERE id = $1', [objetivo]);
    if (!autor) throw new ApiErr('Usuario no encontrado', 404);
    return {
      user: {
        id: objetivo,
        username: autor.username,
        display_name: autor.display_name || autor.username,
        avatar_url: autor.avatar_url,
        is_verified: !!autor.is_verified,
      },
      stories: lista.map((s) => ({
        id: Number(s.id),
        image_url: s.image_url,
        caption: s.caption,
        music_title: s.music_title || '',
        music_url: s.music_url || '',
        music_start_sec: Number(s.music_start_sec || 0),
        music_duration_sec: Number(s.music_duration_sec || 15),
        views_count: Number(s.views_count),
        reactions_count: Number(s.reactions_count || 0),
        comments_count: Number(s.comments_count || 0),
        mi_reaccion: s.mi_reaccion || null,
        created_at: s.created_at,
        vista: !!s.vista,
      })),
    };
  });

  router.post('/api/stories', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const imagen = texto(b.image_url || '', { min: 1, max: 500, campo: 'imagen' });
    const pie = typeof b.caption === 'string' ? b.caption.slice(0, 200) : '';
    const musicTitle = typeof b.music_title === 'string' ? b.music_title.slice(0, 150) : '';
    const musicUrl = typeof b.music_url === 'string' ? b.music_url.slice(0, 500) : '';
    const musicStart = Math.max(0, parseInt(b.music_start_sec || 0, 10) || 0);
    const musicDuration = Math.min(60, Math.max(5, parseInt(b.music_duration_sec || 15, 10) || 15));

    const creada = await uno(
      c.pool,
      `INSERT INTO stories (user_id, image_url, caption, music_title, music_url, music_start_sec, music_duration_sec, expires_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, NOW() + INTERVAL '24 hours')
       RETURNING id, created_at::text AS created_at, expires_at::text AS expires_at`,
      [yo.id, imagen, pie, musicTitle, musicUrl, musicStart, musicDuration]
    );
    await auditar(c.pool, Number(yo.id), 'historia_creada', `#${creada.id}`, c.ip);
    return {
      id: Number(creada.id),
      image_url: imagen,
      caption: pie,
      music_title: musicTitle,
      music_url: musicUrl,
      music_start_sec: musicStart,
      music_duration_sec: musicDuration,
      views_count: 0,
      reactions_count: 0,
      comments_count: 0,
      mi_reaccion: null,
      created_at: creada.created_at,
      expires_at: creada.expires_at,
      vista: false,
    };
  });

  // Reaccionar a una historia
  router.post('/api/stories/:id/react', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const b = await c.cuerpo();
    const emoji = texto(b.emoji || '❤️', { min: 1, max: 20, campo: 'emoji' });

    const historia = await uno(c.pool, 'SELECT id, user_id FROM stories WHERE id = $1', [id]);
    if (!historia) throw new ApiErr('Historia no encontrada', 404);

    await c.pool.query(
      `INSERT INTO story_reactions (story_id, user_id, emoji)
       VALUES ($1, $2, $3)
       ON CONFLICT (story_id, user_id) DO UPDATE SET emoji = EXCLUDED.emoji, created_at = NOW()`,
      [id, yo.id, emoji]
    );

    // Opcionalmente notificar al autor si no es él mismo
    if (Number(historia.user_id) !== Number(yo.id)) {
      await c.pool.query(
        `INSERT INTO notifications (user_id, actor_id, type, entity_id)
         VALUES ($1, $2, 'reaction', $3)
         ON CONFLICT DO NOTHING`,
        [historia.user_id, yo.id, id]
      ).catch(() => {});
    }

    return { ok: true, emoji };
  });

  // Quitar reacción de una historia
  router.del('/api/stories/:id/react', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    await c.pool.query('DELETE FROM story_reactions WHERE story_id = $1 AND user_id = $2', [id, yo.id]);
    return { ok: true };
  });

  // Listar comentarios de una historia
  router.get('/api/stories/:id/comments', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const historia = await uno(c.pool, 'SELECT id, user_id FROM stories WHERE id = $1', [id]);
    if (!historia) throw new ApiErr('Historia no encontrada', 404);

    const comentarios = await filas(
      c.pool,
      `SELECT sc.id, sc.story_id, sc.user_id, sc.content, sc.created_at::text AS created_at,
              u.username, u.display_name, u.avatar_url
         FROM story_comments sc
         JOIN users u ON u.id = sc.user_id
        WHERE sc.story_id = $1
        ORDER BY sc.created_at ASC`,
      [id]
    );

    return {
      items: comentarios.map((com) => ({
        id: Number(com.id),
        story_id: Number(com.story_id),
        user_id: Number(com.user_id),
        content: com.content,
        created_at: com.created_at,
        mine: Number(com.user_id) === Number(yo.id),
        user: {
          id: Number(com.user_id),
          username: com.username,
          display_name: com.display_name || com.username,
          avatar_url: com.avatar_url,
        },
      })),
    };
  });

  // Comentar en una historia
  router.post('/api/stories/:id/comments', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const b = await c.cuerpo();
    const contenido = texto(b.content || '', { min: 1, max: 500, campo: 'comentario' });

    const historia = await uno(c.pool, 'SELECT id, user_id FROM stories WHERE id = $1', [id]);
    if (!historia) throw new ApiErr('Historia no encontrada', 404);

    const creado = await uno(
      c.pool,
      `INSERT INTO story_comments (story_id, user_id, content)
       VALUES ($1, $2, $3)
       RETURNING id, story_id, user_id, content, created_at::text AS created_at`,
      [id, yo.id, contenido]
    );

    // Notificar al dueño de la historia
    if (Number(historia.user_id) !== Number(yo.id)) {
      await c.pool.query(
        `INSERT INTO notifications (user_id, actor_id, type, entity_id)
         VALUES ($1, $2, 'comment', $3)
         ON CONFLICT DO NOTHING`,
        [historia.user_id, yo.id, id]
      ).catch(() => {});
    }

    return {
      id: Number(creado.id),
      story_id: Number(creado.story_id),
      user_id: Number(creado.user_id),
      content: creado.content,
      created_at: creado.created_at,
      mine: true,
      user: {
        id: Number(yo.id),
        username: yo.username,
        display_name: yo.display_name || yo.username,
        avatar_url: yo.avatar_url,
      },
    };
  });

  router.post('/api/stories/:id/view', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const existe = await uno(c.pool, 'SELECT id, user_id FROM stories WHERE id = $1', [id]);
    if (!existe) throw new ApiErr('Historia no encontrada', 404);
    const r = await c.pool.query(
      'INSERT INTO story_views (story_id, viewer_id) VALUES ($1, $2) ON CONFLICT DO NOTHING',
      [id, yo.id]
    );
    if (r.rowCount > 0) {
      await c.pool.query('UPDATE stories SET views_count = views_count + 1 WHERE id = $1', [id]);
    }
    return { ok: true };
  });

  router.del('/api/stories/:id', async (c) => {
    const yo = await c.exigir();
    const r = await c.pool.query('DELETE FROM stories WHERE id = $1 AND user_id = $2', [Number(c.params.id), yo.id]);
    if (r.rowCount === 0) throw new ApiErr('Historia no encontrada', 404);
    return { ok: true };
  });

  // ---------- Presencia (quién está conectado ahora) ----------
  router.get('/api/users/presence', async (c) => {
    const yo = await c.exigir();
    const conectados = usuariosConectados();
    const conocidos = await filas(
      c.pool,
      `SELECT u.* FROM users u
        WHERE u.id <> $1 AND u.status = 'active' AND u.show_online <> FALSE
          AND (EXISTS (SELECT 1 FROM follows f WHERE f.follower_id = $1 AND f.following_id = u.id)
            OR EXISTS (SELECT 1 FROM follows f WHERE f.follower_id = u.id AND f.following_id = $1))
          AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.blocker_id = $1 AND b.blocked_id = u.id) OR (b.blocker_id = u.id AND b.blocked_id = $1))
        ORDER BY u.last_login_at DESC NULLS LAST LIMIT 40`,
      [yo.id]
    );
    const enLinea = [];
    const otros = [];
    for (const u of conocidos) {
      const ficha = {
        id: Number(u.id),
        username: u.username,
        display_name: u.display_name || u.username,
        avatar_url: u.avatar_url,
        is_verified: !!u.is_verified,
        online: conectados.includes(Number(u.id)),
      };
      if (ficha.online) enLinea.push(ficha);
      else otros.push(ficha);
    }
    // Si aún no sigues a nadie, se sugiere a quien esté conectado.
    if (enLinea.length === 0 && otros.length === 0) {
      const sugeridos = await filas(
        c.pool,
        `SELECT u.* FROM users u WHERE u.id <> $1 AND u.status = 'active' ORDER BY u.created_at DESC LIMIT 20`,
        [yo.id]
      );
      for (const u of sugeridos) {
        otros.push({
          id: Number(u.id),
          username: u.username,
          display_name: u.display_name || u.username,
          avatar_url: u.avatar_url,
          is_verified: !!u.is_verified,
          online: conectados.includes(Number(u.id)),
        });
      }
    }
    return { en_linea: enLinea, otros, total_en_linea: enLinea.length };
  });

  // ---------- Novedades (contadores globales que refrescan la interfaz) ----------
  router.get('/api/novedades', async (c) => {
    const yo = await c.exigir();
    const r = await uno(
      c.pool,
      `SELECT
        (SELECT COUNT(*)::int FROM notifications WHERE user_id = $1 AND is_read = FALSE) AS avisos,
        (SELECT COUNT(*)::int FROM messages m JOIN conversations cv ON cv.id = m.conversation_id
          WHERE (cv.user_a = $1 OR cv.user_b = $1) AND m.sender_id <> $1 AND m.read_at IS NULL AND m.status <> 'deleted') AS mensajes,
        (SELECT COUNT(*)::int FROM stories s WHERE s.expires_at > NOW()
          AND (s.user_id = $1 OR EXISTS (SELECT 1 FROM follows f WHERE f.follower_id = $1 AND f.following_id = s.user_id))) AS historias,
        (SELECT COUNT(*)::int FROM stories s WHERE s.expires_at > NOW() AND s.user_id = $1) AS historias_propias`,
      [yo.id]
    );
    return {
      avisos: Number(r.avisos),
      mensajes: Number(r.mensajes),
      historias: Number(r.historias),
      historias_propias: Number(r.historias_propias),
    };
  });
}
