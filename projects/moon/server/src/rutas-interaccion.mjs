// Moon — Interacción rica
// ============================================================
// Lo que hace que cada cosa tenga más de una salida:
//   · reacciones variadas en publicaciones y comentarios
//   · fijar una publicación (en tu perfil) y un comentario (si es tu post)
//   · «no me interesa»: desaparece de tu inicio sin bloquear a nadie
//   · quién votó en tu encuesta
// Todo con SQL real; nada se guarda solo en el navegador.
// ============================================================

import { ApiErr } from './util.mjs';
import { uno, auditar } from './db.mjs';
import { notificar } from './ws.mjs';
import { TIPOS, tipoValido, conReacciones, conReaccionesComentarios } from './reacciones.mjs';
import { SQL_POST, aPublicacion, conContenido } from './rutas-social.mjs';

// Cuántas reacciones hay de cada tipo: la lista de quién reaccionó se pide aparte.
import { paginacion } from './util.mjs';

export function registrarRutasInteraccion(router) {
  /** Devuelve la publicación completa (con imágenes, encuesta y reacciones). */
  async function publicacionCompleta(pool, yoId, postId) {
    const f = await uno(pool, `${SQL_POST} WHERE p.id = $2`, [yoId, postId]);
    if (!f || f.status !== 'active') throw new ApiErr('Publicación no encontrada', 404);
    const [completa] = await conContenido(pool, [aPublicacion(f, yoId)], yoId);
    return completa;
  }

  // ---------- Reacciones variadas ----------
  router.post('/api/posts/:id/react', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const b = await c.cuerpo();
    const tipo = String(b.tipo || 'me_gusta');
    if (!tipoValido(tipo)) throw new ApiErr(`Reacción no válida (${TIPOS.join(', ')})`, 400, 'reaccion_invalida');

    const post = await uno(c.pool, "SELECT id, user_id FROM posts WHERE id = $1 AND status = 'active'", [postId]);
    if (!post) throw new ApiErr('Publicación no encontrada', 404);

    // Cambiar de reacción no suma al contador: es la misma persona.
    const previa = await uno(c.pool, 'SELECT tipo FROM likes WHERE user_id = $1 AND post_id = $2', [yo.id, postId]);
    await c.pool.query(
      `INSERT INTO likes (user_id, post_id, tipo) VALUES ($1, $2, $3)
       ON CONFLICT (user_id, post_id) DO UPDATE SET tipo = $3`,
      [yo.id, postId, tipo]
    );
    if (!previa) {
      await c.pool.query('UPDATE posts SET likes_count = likes_count + 1 WHERE id = $1', [postId]);
      await notificar(c.pool, {
        userId: Number(post.user_id), tipo: 'like', deUserId: Number(yo.id), postId,
        contenido: 'reaccionó a tu publicación',
      });
    }
    return publicacionCompleta(c.pool, yo.id, postId);
  });

  router.del('/api/posts/:id/react', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const r = await c.pool.query('DELETE FROM likes WHERE user_id = $1 AND post_id = $2', [yo.id, postId]);
    if (r.rowCount > 0) {
      await c.pool.query('UPDATE posts SET likes_count = GREATEST(0, likes_count - 1) WHERE id = $1', [postId]);
    }
    return publicacionCompleta(c.pool, yo.id, postId);
  });

  /** Quiénes reaccionaron (para ver la lista al tocar el resumen). */
  router.get('/api/posts/:id/reacciones', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const { limit } = paginacion(c.req, 50, 100);
    const filas = await c.pool.query(
      `SELECT l.tipo, u.id, u.username, u.display_name, u.avatar_url, u.is_verified
         FROM likes l JOIN users u ON u.id = l.user_id
        WHERE l.post_id = $1
        ORDER BY l.created_at DESC LIMIT ${limit}`,
      [postId]
    );
    const porTipo = {};
    for (const f of filas.rows) porTipo[f.tipo] = (porTipo[f.tipo] || 0) + 1;
    return {
      total: filas.rows.length,
      por_tipo: porTipo,
      personas: filas.rows.map((f) => ({
        id: Number(f.id),
        username: f.username,
        display_name: f.display_name || f.username,
        avatar_url: f.avatar_url,
        is_verified: !!f.is_verified,
        tipo: f.tipo,
        is_mine: Number(f.id) === Number(yo.id),
      })),
    };
  });

  // ---------- Fijar una publicación ----------
  router.post('/api/posts/:id/pin', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const post = await uno(c.pool, "SELECT id, user_id, pinned_at FROM posts WHERE id = $1 AND status = 'active'", [postId]);
    if (!post) throw new ApiErr('Publicación no encontrada', 404);
    if (Number(post.user_id) !== Number(yo.id)) throw new ApiErr('Solo puedes fijar tus publicaciones', 403, 'no_es_tuya');

    const fijar = !post.pinned_at;
    if (fijar) {
      // Una sola fijada por persona: la anterior se suelta.
      await c.pool.query('UPDATE posts SET pinned_at = NULL WHERE user_id = $1 AND pinned_at IS NOT NULL', [yo.id]);
    }
    await c.pool.query('UPDATE posts SET pinned_at = $2 WHERE id = $1', [postId, fijar ? new Date() : null]);
    return publicacionCompleta(c.pool, yo.id, postId);
  });

  // ---------- «No me interesa» ----------
  router.post('/api/posts/:id/interesa', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const b = await c.cuerpo().catch(() => ({}));
    const noInteresa = b.no === undefined ? true : !!b.no;
    if (noInteresa) {
      await c.pool.query(
        'INSERT INTO post_hidden (user_id, post_id) VALUES ($1, $2) ON CONFLICT DO NOTHING',
        [yo.id, postId]
      );
      await auditar(c.pool, Number(yo.id), 'publicacion_oculta', `#${postId}`, c.ip);
    } else {
      await c.pool.query('DELETE FROM post_hidden WHERE user_id = $1 AND post_id = $2', [yo.id, postId]);
    }
    return { ok: true, no_interesa: noInteresa };
  });

  // ---------- Encuesta: quién votó (solo quien la creó) ----------
  router.get('/api/posts/:id/poll/votos', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const post = await uno(c.pool, 'SELECT id, user_id FROM posts WHERE id = $1', [postId]);
    if (!post) throw new ApiErr('Publicación no encontrada', 404);
    const encuesta = await uno(c.pool, 'SELECT opciones FROM polls WHERE post_id = $1', [postId]);
    if (!encuesta) throw new ApiErr('Esta publicación no tiene encuesta', 404);
    // La encuesta es anónima para todos menos para quien la creó.
    if (Number(post.user_id) !== Number(yo.id)) throw new ApiErr('Solo quien creó la encuesta ve los votos', 403, 'no_es_tuya');

    const opciones = Array.isArray(encuesta.opciones) ? encuesta.opciones : [];
    const votos = (await c.pool.query(
      `SELECT pv.opcion, u.id, u.username, u.display_name, u.avatar_url, u.is_verified
         FROM poll_votes pv JOIN users u ON u.id = pv.user_id
        WHERE pv.post_id = $1 ORDER BY pv.created_at DESC`,
      [postId]
    )).rows;
    return {
      total: votos.length,
      opciones: opciones.map((texto, i) => {
        const deOpcion = votos.filter((v) => Number(v.opcion) === i);
        return {
          indice: i,
          texto,
          votos: deOpcion.length,
          personas: deOpcion.map((v) => ({
            id: Number(v.id),
            username: v.username,
            display_name: v.display_name || v.username,
            avatar_url: v.avatar_url,
            is_verified: !!v.is_verified,
          })),
        };
      }),
    };
  });

  // ---------- Comentarios: reacciones y fijado ----------
  router.post('/api/comments/:id/react', async (c) => {
    const yo = await c.exigir();
    const comentarioId = Number(c.params.id);
    const b = await c.cuerpo();
    const tipo = String(b.tipo || 'me_gusta');
    if (!tipoValido(tipo)) throw new ApiErr('Reacción no válida', 400, 'reaccion_invalida');
    const comentario = await uno(c.pool, "SELECT id, post_id, user_id FROM comments WHERE id = $1 AND status = 'active'", [comentarioId]);
    if (!comentario) throw new ApiErr('Comentario no encontrado', 404);

    const previa = await uno(c.pool, 'SELECT tipo FROM comment_likes WHERE comment_id = $1 AND user_id = $2', [comentarioId, yo.id]);
    await c.pool.query(
      `INSERT INTO comment_likes (comment_id, user_id, tipo) VALUES ($1, $2, $3)
       ON CONFLICT (comment_id, user_id) DO UPDATE SET tipo = $3`,
      [comentarioId, yo.id, tipo]
    );
    if (!previa) {
      await notificar(c.pool, {
        userId: Number(comentario.user_id), tipo: 'like', deUserId: Number(yo.id),
        postId: Number(comentario.post_id), commentId: comentarioId,
        contenido: 'reaccionó a tu comentario',
      });
    }
    const [unoSolo] = await conReaccionesComentarios(c.pool, [{ id: comentarioId }], yo.id);
    return unoSolo;
  });

  router.del('/api/comments/:id/react', async (c) => {
    const yo = await c.exigir();
    const comentarioId = Number(c.params.id);
    await c.pool.query('DELETE FROM comment_likes WHERE comment_id = $1 AND user_id = $2', [comentarioId, yo.id]);
    const [unoSolo] = await conReaccionesComentarios(c.pool, [{ id: comentarioId }], yo.id);
    return unoSolo;
  });

  router.post('/api/comments/:id/pin', async (c) => {
    const yo = await c.exigir();
    const comentarioId = Number(c.params.id);
    const filaComentario = await uno(
      c.pool,
      `SELECT cm.id, cm.pinned_at, p.user_id AS autor_del_post
         FROM comments cm JOIN posts p ON p.id = cm.post_id
        WHERE cm.id = $1 AND cm.status = 'active'`,
      [comentarioId]
    );
    if (!filaComentario) throw new ApiErr('Comentario no encontrado', 404);
    if (Number(filaComentario.autor_del_post) !== Number(yo.id)) {
      throw new ApiErr('Solo quien publicó puede fijar un comentario', 403, 'no_es_tuya');
    }
    const fijar = !filaComentario.pinned_at;
    if (fijar) {
      await c.pool.query(
        'UPDATE comments SET pinned_at = NULL WHERE post_id = (SELECT post_id FROM comments WHERE id = $1) AND pinned_at IS NOT NULL',
        [comentarioId]
      );
    }
    await c.pool.query('UPDATE comments SET pinned_at = $2 WHERE id = $1', [comentarioId, fijar ? new Date() : null]);
    return { ok: true, fijado: fijar };
  });
}
