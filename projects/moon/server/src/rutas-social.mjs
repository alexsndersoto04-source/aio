// Moon — Rutas sociales
// ============================================================
// Inicio (feed), publicaciones, comentarios, reacciones, guardados, personas,
// búsqueda, tendencias, notificaciones y reportes. Todo con SQL real y
// contadores actualizados en la base de datos.

import { ApiErr, texto, paginacion, qs, hashtagsDe, mencionesDe } from './util.mjs';
import { fila, uno } from './db.mjs';
import { auditar, sumarEstadistica } from './db.mjs';
import { notificar } from './ws.mjs';
import { demasiadoRapido } from './limites.mjs';
import { conReacciones, conReaccionesComentarios, tipoValido } from './reacciones.mjs';

// ---------- Ayudas ----------

export const SQL_POST = `
  SELECT p.id, p.content, p.status, p.likes_count, p.comments_count, p.saves_count,
         p.created_at::text AS created_at, p.edited_at::text AS edited_at,
         p.pinned_at::text AS pinned_at, p.user_id,
         u.username AS author_username, u.display_name AS author_display_name,
         u.avatar_url AS author_avatar_url, u.is_verified AS author_is_verified,
         u.is_private AS author_is_private,
         (SELECT COUNT(*)::int FROM likes l WHERE l.post_id = p.id AND l.user_id = $1) > 0 AS is_liked,
         (SELECT COUNT(*)::int FROM saves s WHERE s.post_id = p.id AND s.user_id = $1) > 0 AS is_saved
    FROM posts p JOIN users u ON u.id = p.user_id`;

const SQL_IMAGENES = 'SELECT id, post_id, original_url, thumb_url, position FROM post_images WHERE post_id = ANY($1::bigint[]) ORDER BY position';

export async function conContenido(pool, filas, yoId) {
  const conIm = await conImagenes(pool, filas);
  await marcarConEncuesta(pool, conIm);
  const conEnc = await conEncuestas(pool, conIm, yoId);
  return conReacciones(pool, conEnc, yoId);
}

export async function conImagenes(pool, filas) {
  if (filas.length === 0) return filas;
  const ids = filas.map((f) => Number(f.id));
  const imagenes = (await pool.query(SQL_IMAGENES, [ids])).rows;
  const porPost = new Map();
  for (const im of imagenes) {
    if (!porPost.has(Number(im.post_id))) porPost.set(Number(im.post_id), []);
    const urlLimpia = (im.original_url || im.thumb_url || '').toLowerCase();
    const esVid = /\.(mp4|webm|mov|mkv|3gp|ogv)(\?.*)?$/i.test(urlLimpia) || urlLimpia.includes('video');
    porPost.get(Number(im.post_id)).push({
      id: im.id,
      url: im.original_url,
      original_url: im.original_url,
      thumb_url: im.thumb_url || im.original_url,
      kind: esVid ? 'video' : 'image',
    });
  }
  for (const f of filas) f.images = porPost.get(Number(f.id)) || [];
  return filas;
}

/** Encuestas: se pegan a las publicaciones que las tengan, con sus votos. */
export async function conEncuestas(pool, posts, yoId) {
  const con = posts.filter((p) => p && p.poll);
  if (con.length === 0) return posts;
  const ids = posts.map((p) => Number(p.id));
  const encuestas = (await pool.query(
    `SELECT post_id, pregunta, opciones, multiple, ends_at, (ends_at IS NOT NULL AND ends_at < NOW()) AS cerrada
       FROM polls WHERE post_id = ANY($1::bigint[])`,
    [ids]
  )).rows;
  const votos = (await pool.query(
    `SELECT post_id, opcion, COUNT(*)::int AS n FROM poll_votes WHERE post_id = ANY($1::bigint[]) GROUP BY post_id, opcion`,
    [ids]
  )).rows;
  const mios = (await pool.query(
    `SELECT post_id, opcion FROM poll_votes WHERE post_id = ANY($1::bigint[]) AND user_id = $2`,
    [ids, yoId || 0]
  )).rows;

  const porPost = new Map();
  for (const e of encuestas) {
    const total = votos.filter((v) => Number(v.post_id) === Number(e.post_id)).reduce((n, v) => n + Number(v.n), 0);
    const opciones = (Array.isArray(e.opciones) ? e.opciones : []).map((texto, i) => {
      const n = Number(votos.find((v) => Number(v.post_id) === Number(e.post_id) && Number(v.opcion) === i)?.n || 0);
      return { texto, votos: n, porcentaje: total > 0 ? Math.round((n / total) * 100) : 0 };
    });
    // Texto corto de cierre: «cierra en 5 h» / «cierra el 20 sep».
    let cierra = null;
    if (e.ends_at && !e.cerrada) {
      const fin = new Date(e.ends_at);
      const horas = Math.round((fin.getTime() - Date.now()) / 3_600_000);
      cierra = horas <= 1 ? 'cierra en menos de 1 h'
        : horas < 24 ? `cierra en ${horas} h`
        : `cierra el ${fin.toLocaleDateString('es-VE', { day: 'numeric', month: 'short' })}`;
    }
    porPost.set(Number(e.post_id), {
      pregunta: e.pregunta,
      cierra,
      opciones,
      multiple: !!e.multiple,
      total,
      cerrada: !!e.cerrada,
      ends_at: e.ends_at ? String(e.ends_at) : null,
      mi_voto: mios.filter((m) => Number(m.post_id) === Number(e.post_id)).map((m) => Number(m.opcion)),
    });
  }
  for (const p of posts) p.poll = porPost.get(Number(p.id)) || null;
  return posts;
}

/** Cuántas publicaciones tienen encuesta (se usa al listar). */
async function marcarConEncuesta(pool, posts) {
  if (posts.length === 0) return posts;
  const ids = posts.map((p) => Number(p.id));
  const con = (await pool.query('SELECT post_id FROM polls WHERE post_id = ANY($1::bigint[])', [ids])).rows;
  const set = new Set(con.map((r) => Number(r.post_id)));
  for (const p of posts) if (set.has(Number(p.id))) p.poll = true;
  return posts;
}

export function aPublicacion(f, yo) {
  const authorId = Number(f.user_id);
  const autorUsername = f.author_username || '';
  const autorNombre = f.author_display_name || autorUsername;
  const autorAvatar = f.author_avatar_url || '';
  const verificado = !!f.author_is_verified;
  return {
    id: Number(f.id),
    user_id: authorId,
    author_id: authorId,
    content: f.status === 'deleted' ? '' : f.content,
    deleted: f.status === 'deleted',
    created_at: f.created_at,
    edited_at: f.edited_at,
    images: f.images || [],
    likes_count: Number(f.likes_count),
    comments_count: Number(f.comments_count),
    saves_count: Number(f.saves_count),
    is_liked: !!f.is_liked,
    is_saved: !!f.is_saved,
    pinned: !!f.pinned_at,
    pinned_at: f.pinned_at || null,
    is_mine: authorId === Number(yo),
    author_username: autorUsername,
    author_display_name: autorNombre,
    author_avatar_url: autorAvatar,
    author_is_verified: verificado,
    author_is_private: !!f.author_is_private,
    user: {
      id: authorId,
      username: autorUsername,
      display_name: autorNombre,
      avatar_url: autorAvatar,
      is_verified: verificado,
      verified: verificado,
    },
  };
}

// Personas bloqueadas en cualquier sentido: ni sus publicaciones ni su perfil.
const SQL_NO_BLOQUEADOS = `
  NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.blocker_id = $1 AND b.blocked_id = p.user_id)
                                        OR (b.blocker_id = p.user_id AND b.blocked_id = $1))`;

// «No me interesa»: esa publicación no vuelve a aparecer en tu inicio.
const SQL_NO_OCULTOS = `NOT EXISTS (SELECT 1 FROM post_hidden ph WHERE ph.user_id = $1 AND ph.post_id = p.id)`;

async function conPagina(c, filas, total, page, limit) {
  return { items: filas, total, page, limit };
}

function contarPublicaciones(pool, where, args) {
  return uno(pool, `SELECT COUNT(*)::int AS count FROM posts p JOIN users u ON u.id = p.user_id WHERE ${where}`, args)
    .then((r) => Number(r?.count || 0));
}

export function perfilPublico(u, { siguiendo = false, bloqueado = false } = {}) {
    return {
      id: Number(u.id),
      username: u.username,
      display_name: u.display_name || u.username,
      avatar_url: u.avatar_url,
      cover_url: u.cover_url,
      bio: u.bio,
      link: u.link,
      location: u.location,
      is_verified: !!u.is_verified,
      is_private: !!u.is_private,
      followers_count: Number(u.followers_count),
      following_count: Number(u.following_count),
      posts_count: Number(u.posts_count),
      created_at: u.created_at ? String(u.created_at) : null,
      is_following: !!siguiendo,
      is_blocked: !!bloqueado,
    };
  }

export function registrarRutasSocial(router) {
  // ---------- Inicio ----------
  /**
   * Filtro por tipo de publicación: «fotos», «encuestas», «voz» o vacío (todo).
   * Se usa en las tres vistas del inicio (Para ti, Tendencias y Recientes).
   */
  function condicionDeTipo(tipo) {
    if (tipo === 'videos') return `(
      EXISTS (
        SELECT 1 FROM post_images pi
        WHERE pi.post_id = p.id
          AND (
            pi.original_url ILIKE '%.mp4%' OR pi.original_url ILIKE '%.webm%'
            OR pi.original_url ILIKE '%.mov%' OR pi.original_url ILIKE '%.mkv%'
            OR pi.original_url ILIKE '%.3gp%' OR pi.original_url ILIKE '%.ogv%'
            OR pi.original_url ILIKE '%video%'
            OR pi.thumb_url ILIKE '%.mp4%' OR pi.thumb_url ILIKE '%.webm%'
            OR pi.thumb_url ILIKE '%.mov%' OR pi.thumb_url ILIKE '%.mkv%'
            OR pi.thumb_url ILIKE '%.3gp%' OR pi.thumb_url ILIKE '%.ogv%'
            OR pi.thumb_url ILIKE '%video%'
          )
      )
      OR EXISTS (
        SELECT 1 FROM media m
        WHERE (m.kind = 'video' OR m.kind = ('post_' || p.id))
          AND (m.original_path ILIKE '%.mp4%' OR m.original_path ILIKE '%.webm%' OR m.original_path ILIKE '%.mov%' OR m.original_path ILIKE '%.mkv%' OR m.original_path ILIKE '%.3gp%')
      )
    )`;
    if (tipo === 'fotos') return 'EXISTS (SELECT 1 FROM post_images pi WHERE pi.post_id = p.id)';
    if (tipo === 'encuestas') return 'EXISTS (SELECT 1 FROM polls pl WHERE pl.post_id = p.id)';
    if (tipo === 'texto') return 'NOT EXISTS (SELECT 1 FROM post_images pi WHERE pi.post_id = p.id) AND NOT EXISTS (SELECT 1 FROM polls pl WHERE pl.post_id = p.id)';
    return '';
  }

  /** «Para ti» de verdad: primero quienes sigo, después el resto. */
  function ordenParaTi(yoId) {
    return `(CASE WHEN p.user_id IN (SELECT following_id FROM follows WHERE follower_id = ${Number(yoId) || 0}) OR p.user_id = ${Number(yoId) || 0} THEN 0 ELSE 1 END),
            p.created_at DESC`;
  }

  router.get('/api/feed', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.req, 10, 50);
    const tipo = condicionDeTipo(String(c.query.get('tipo') || ''));
    const condiciones = ["p.status = 'active'", SQL_NO_BLOQUEADOS, SQL_NO_OCULTOS];
    if (tipo) condiciones.push(tipo);
    const args = [yo.id];
    const total = await uno(
      c.pool,
      `SELECT COUNT(*)::int AS count FROM posts p WHERE ${condiciones.join(' AND ')}`,
      args
    );
    const filas = await c.pool.query(
      `${SQL_POST} WHERE ${condiciones.join(' AND ')}
        ORDER BY ${ordenParaTi(yo.id)} LIMIT ${limit} OFFSET ${offset}`,
      args
    );
    return conPagina(c, await conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yo.id)), yo.id), Number(total?.count || 0), page, limit);
  });

  router.get('/api/feed/trending', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.req, 10, 50);
    const tipoTrending = condicionDeTipo(String(c.query.get('tipo') || ''));
    const filas = await c.pool.query(
      `${SQL_POST} WHERE p.status = 'active' AND ${SQL_NO_BLOQUEADOS} AND ${SQL_NO_OCULTOS}
        AND p.created_at > NOW() - INTERVAL '7 days'
        ${tipoTrending ? `AND ${tipoTrending}` : ''}
        ORDER BY (p.likes_count * 3 + p.comments_count * 4) DESC, p.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id]
    );
    const total = await contarPublicaciones(
      c.pool,
      "p.status = 'active' AND " + SQL_NO_BLOQUEADOS + " AND " + SQL_NO_OCULTOS
        + " AND p.created_at > NOW() - INTERVAL '7 days'"
        + (tipoTrending ? ` AND ${tipoTrending}` : ''),
      [yo.id]
    );
    return conPagina(c, await conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yo.id)), yo.id), total, page, limit);
  });

  router.get('/api/feed/latest', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.req, 10, 50);
    const tipoNuevo = condicionDeTipo(String(c.query.get('tipo') || ''));
    const filas = await c.pool.query(
      `${SQL_POST} WHERE p.status = 'active' AND ${SQL_NO_BLOQUEADOS} AND ${SQL_NO_OCULTOS} ${tipoNuevo ? `AND ${tipoNuevo}` : ''}
        ORDER BY p.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id]
    );
    const total = await contarPublicaciones(c.pool, "p.status = 'active' AND " + SQL_NO_BLOQUEADOS + " AND " + SQL_NO_OCULTOS, [yo.id]);
    return conPagina(c, await conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yo.id)), yo.id), total, page, limit);
  });

  // ---------- Moon Watch (Videos de la comunidad) ----------
  router.get('/api/videos', async (c) => {
    const yo = await c.usuario();
    const yoId = yo ? Number(yo.id) : 0;
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const busqueda = (c.query.get('q') || '').trim();
    const categoria = (c.query.get('cat') || 'para_ti').trim();

    const condVideo = `(
      EXISTS (
        SELECT 1 FROM post_images pi
        WHERE pi.post_id = p.id
          AND (
            pi.original_url ILIKE '%.mp4%' OR pi.original_url ILIKE '%.webm%'
            OR pi.original_url ILIKE '%.mov%' OR pi.original_url ILIKE '%.mkv%'
            OR pi.original_url ILIKE '%.3gp%' OR pi.original_url ILIKE '%.ogv%'
            OR pi.original_url ILIKE '%video%'
            OR pi.thumb_url ILIKE '%.mp4%' OR pi.thumb_url ILIKE '%.webm%'
            OR pi.thumb_url ILIKE '%.mov%' OR pi.thumb_url ILIKE '%.mkv%'
            OR pi.thumb_url ILIKE '%.3gp%' OR pi.thumb_url ILIKE '%.ogv%'
            OR pi.thumb_url ILIKE '%video%'
          )
      )
      OR EXISTS (
        SELECT 1 FROM media m
        WHERE (m.kind = 'video' OR m.kind = ('post_' || p.id))
          AND (m.original_path ILIKE '%.mp4%' OR m.original_path ILIKE '%.webm%' OR m.original_path ILIKE '%.mov%' OR m.original_path ILIKE '%.mkv%' OR m.original_path ILIKE '%.3gp%')
      )
    )`;

    const condiciones = [
      "p.status = 'active'",
      "($1::bigint = $1::bigint)",
      condVideo,
    ];
    const args = [yoId];

    if (yoId) {
      condiciones.push(SQL_NO_BLOQUEADOS);
      condiciones.push(SQL_NO_OCULTOS);
    }

    if (categoria === 'siguiendo' && yoId) {
      condiciones.push(`p.user_id IN (SELECT following_id FROM follows WHERE follower_id = $1)`);
    } else if (categoria === 'mis_videos' && yoId) {
      condiciones.push(`p.user_id = $1`);
    }

    if (busqueda) {
      args.push(`%${busqueda.toLowerCase()}%`);
      const bIdx = args.length;
      condiciones.push(`(
        LOWER(p.content) LIKE $${bIdx}
        OR LOWER(u.username) LIKE $${bIdx}
        OR LOWER(u.display_name) LIKE $${bIdx}
      )`);
    }

    let orden = 'p.created_at DESC';
    if (categoria === 'tendencias') {
      orden = '(p.likes_count * 3 + p.comments_count * 4) DESC, p.created_at DESC';
    } else if (categoria === 'para_ti' && yoId) {
      orden = `${ordenParaTi(yoId)}`;
    }

    const where = condiciones.join(' AND ');
    const total = await uno(
      c.pool,
      `SELECT COUNT(*)::int AS count FROM posts p JOIN users u ON u.id = p.user_id WHERE ${where}`,
      args
    );

    const filas = await c.pool.query(
      `${SQL_POST} WHERE ${where}
        ORDER BY ${orden}
        LIMIT ${limit} OFFSET ${offset}`,
      args
    );

    const posts = await conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yoId)), yoId);
    return conPagina(c, posts, Number(total?.count || 0), page, limit);
  });

  // ---------- Publicaciones ----------
  router.post('/api/posts', async (c) => {
    const yo = await c.exigir();
    if (demasiadoRapido(`post:${yo.id}`, 10, 60_000)) {
      throw new ApiErr('Vas demasiado rápido: espera unos segundos', 429, 'rate_limit');
    }
    const b = await c.cuerpo();
    const contenido = texto(b.content || '', { min: 0, max: 2000, campo: 'contenido' }).trim();
    const imagenes = Array.isArray(b.images) ? b.images.slice(0, 4) : [];
    if (!contenido && imagenes.length === 0) throw new ApiErr('Escribe algo o adjunta una imagen', 400);

    // Palabras bloqueadas por moderación.
    const bloqueadas = (await c.pool.query('SELECT word FROM blocked_words')).rows.map((r) => r.word.toLowerCase());
    const enMinusculas = ` ${contenido.toLowerCase()} `;
    const encontrada = bloqueadas.find((w) => w && enMinusculas.includes(` ${w} `));
    if (encontrada) throw new ApiErr(`El texto contiene una palabra no permitida: «${encontrada}»`, 400, 'blocked_word');

    const creado = await uno(
      c.pool,
      `INSERT INTO posts (user_id, content) VALUES ($1, $2) RETURNING *`,
      [yo.id, contenido]
    );
    for (const [i, imRaw] of imagenes.entries()) {
      let im = imRaw;
      // Compatibilidad: si el cliente solo mandó el id del medio, se busca su url.
      if (im !== null && typeof im !== 'string' && !(im && (im.url || im.original_url)) && /^\d+$/.test(String(im))) {
        const m = await uno(c.pool, 'SELECT url FROM media WHERE id = $1', [Number(im)]);
        im = m ? { id: Number(im), url: m.url } : null;
      }
      const url = typeof im === 'string' ? im : (im && (im.url || im.original_url)) || '';
      if (!url) continue;
      await c.pool.query(
        'INSERT INTO post_images (post_id, position, original_url, thumb_url) VALUES ($1, $2, $3, $3)',
        [creado.id, i, url]
      );
      if (typeof im === 'object' && im.id && /^\d+$/.test(String(im.id))) {
        await c.pool.query('UPDATE media SET kind = $1 WHERE id = $2 AND user_id = $3', [`post_${creado.id}`, im.id, yo.id]);
      }
    }
    await c.pool.query('UPDATE users SET posts_count = posts_count + 1 WHERE id = $1', [yo.id]);

    // Etiquetas y menciones.
    for (const tag of hashtagsDe(contenido)) {
      const h = await uno(
        c.pool,
        `INSERT INTO hashtags (tag, posts_count, last_used_at) VALUES ($1, 1, NOW())
         ON CONFLICT (tag) DO UPDATE SET posts_count = hashtags.posts_count + 1, last_used_at = NOW()
         RETURNING id`,
        [tag]
      );
      await c.pool.query('INSERT INTO post_hashtags (post_id, hashtag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING', [creado.id, h.id]);
    }
    for (const nombre of mencionesDe(contenido)) {
      const mencionado = await uno(c.pool, 'SELECT id FROM users WHERE LOWER(username) = $1', [nombre]);
      if (!mencionado || Number(mencionado.id) === Number(yo.id)) continue;
      await notificar(c.pool, {
        userId: Number(mencionado.id),
        tipo: 'mention',
        deUserId: Number(yo.id),
        postId: Number(creado.id),
        contenido: 'te mencionó en una publicación',
      });
    }

    // Encuesta (opcional): una pregunta con dos a seis respuestas.
    const encuesta = b.poll && typeof b.poll === 'object' ? b.poll : null;
    if (encuesta) {
      const pregunta = String(encuesta.pregunta || '').trim().slice(0, 160);
      const opciones = (Array.isArray(encuesta.opciones) ? encuesta.opciones : [])
        .map((o) => String(o || '').trim().slice(0, 80))
        .filter(Boolean)
        .slice(0, 6);
      if (opciones.length >= 2) {
        const horas = Math.min(168, Math.max(1, Number(encuesta.horas || 24)));
        await c.pool.query(
          `INSERT INTO polls (post_id, pregunta, opciones, multiple, ends_at)
           VALUES ($1, $2, $3::jsonb, $4, NOW() + ($5 || ' hours')::interval)
           ON CONFLICT (post_id) DO NOTHING`,
          [creado.id, pregunta, JSON.stringify(opciones), !!encuesta.multiple, String(horas)]
        );
      }
    }

    await sumarEstadistica(c.pool, 'new_posts');
    await auditar(c.pool, Number(yo.id), 'publicacion_creada', `#${creado.id}`, c.ip);
    const filas = await c.pool.query(`${SQL_POST} WHERE p.id = $2`, [yo.id, creado.id]);
    const conIm = await conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yo.id)), yo.id);
    return conIm[0];
  });

  router.get('/api/posts/:id', async (c) => {
    const yo = await c.usuario();
    const yoId = yo ? Number(yo.id) : 0;
    const f = await fila(c.pool, `${SQL_POST} WHERE p.id = $2`, [yoId, Number(c.params.id)]);
    if (!f || f.status !== 'active') throw new ApiErr('Publicación no encontrada', 404);
    const [conIm] = await conContenido(c.pool, [aPublicacion(f, yoId)], yoId);
    return conIm;
  });

  // Votar en una encuesta. Si es de una sola respuesta, el voto reemplaza al
  // anterior; si es múltiple, se suma.
  router.post('/api/posts/:id/vote', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const b = await c.cuerpo();
    const opcion = Number(b.opcion);
    if (!Number.isInteger(opcion) || opcion < 0) throw new ApiErr('Voto inválido', 400);

    const encuesta = await uno(
      c.pool,
      `SELECT post_id, opciones, multiple, (ends_at IS NOT NULL AND ends_at < NOW()) AS cerrada FROM polls WHERE post_id = $1`,
      [postId]
    );
    if (!encuesta) throw new ApiErr('Esta publicación no tiene encuesta', 404);
    if (encuesta.cerrada) throw new ApiErr('La encuesta ya cerró', 400, 'poll_closed');
    const total = Array.isArray(encuesta.opciones) ? encuesta.opciones.length : 0;
    if (opcion >= total) throw new ApiErr('Esa opción no existe', 400);

    if (!encuesta.multiple) {
      await c.pool.query('DELETE FROM poll_votes WHERE post_id = $1 AND user_id = $2', [postId, yo.id]);
    }
    await c.pool.query(
      'INSERT INTO poll_votes (post_id, user_id, opcion) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING',
      [postId, yo.id, opcion]
    );
    const fila2 = await uno(c.pool, `${SQL_POST} WHERE p.id = $2`, [Number(yo.id), postId]);
    const [conTodo] = await conContenido(c.pool, [aPublicacion(fila2, yo.id)], yo.id);
    return conTodo;
  });

  router.patch('/api/posts/:id', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const contenido = texto(b.content || '', { min: 1, max: 2000, campo: 'contenido' });
    const r = await c.pool.query(
      `UPDATE posts SET content = $1, edited_at = NOW() WHERE id = $2 AND user_id = $3 AND status = 'active'`,
      [contenido, Number(c.params.id), yo.id]
    );
    if (r.rowCount === 0) throw new ApiErr('No puedes editar esta publicación', 404);
    const f = await fila(c.pool, `${SQL_POST} WHERE p.id = $2`, [yo.id, Number(c.params.id)]);
    const [conIm] = await conContenido(c.pool, [aPublicacion(f, yo.id)], yo.id);
    return conIm;
  });

  router.del('/api/posts/:id', async (c) => {
    const yo = await c.exigir();
    const r = await c.pool.query(
      `UPDATE posts SET status = 'deleted' WHERE id = $1 AND user_id = $2 AND status = 'active'`,
      [Number(c.params.id), yo.id]
    );
    if (r.rowCount === 0) throw new ApiErr('No puedes eliminar esta publicación', 404);
    await c.pool.query('UPDATE users SET posts_count = GREATEST(0, posts_count - 1) WHERE id = $1', [yo.id]);
    await auditar(c.pool, Number(yo.id), 'publicacion_eliminada', `#${c.params.id}`, c.ip);
    return { ok: true };
  });

  // ---------- Reacciones y guardados ----------
  async function alternar(c, tabla, activar, contador) {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    const post = await uno(
      c.pool,
      'SELECT id, user_id FROM posts WHERE id = $1 AND status = $2',
      [postId, 'active']
    );
    if (!post) throw new ApiErr('Publicación no encontrada', 404);

    if (activar) {
      const r = await c.pool.query(
        `INSERT INTO ${tabla} (user_id, post_id) VALUES ($1, $2) ON CONFLICT DO NOTHING`,
        [yo.id, postId]
      );
      if (r.rowCount > 0) {
        await c.pool.query(`UPDATE posts SET ${contador} = ${contador} + 1 WHERE id = $1`, [postId]);
        if (tabla === 'likes') {
          await sumarEstadistica(c.pool, 'new_likes');
          await notificar(c.pool, {
            userId: Number(post.user_id), tipo: 'like', deUserId: Number(yo.id), postId,
            contenido: 'le gustó tu publicación',
          });
        }
      }
    } else {
      const r = await c.pool.query(`DELETE FROM ${tabla} WHERE user_id = $1 AND post_id = $2`, [yo.id, postId]);
      if (r.rowCount > 0) {
        await c.pool.query(`UPDATE posts SET ${contador} = GREATEST(0, ${contador} - 1) WHERE id = $1`, [postId]);
      }
    }
    const f = await fila(c.pool, `${SQL_POST} WHERE p.id = $2`, [yo.id, postId]);
    const [conIm] = await conContenido(c.pool, [aPublicacion(f, yo.id)], yo.id);
    return conIm;
  }

  router.post('/api/posts/:id/like', (c) => alternar(c, 'likes', true, 'likes_count'));
  router.del('/api/posts/:id/like', (c) => alternar(c, 'likes', false, 'likes_count'));
  router.post('/api/posts/:id/save', (c) => alternar(c, 'saves', true, 'saves_count'));
  router.del('/api/posts/:id/save', (c) => alternar(c, 'saves', false, 'saves_count'));

  // ---------- Comentarios ----------
  router.get('/api/posts/:id/comments', async (c) => {
    const yo = await c.usuario();
    const yoId = yo ? Number(yo.id) : 0;
    const postId = Number(c.params.id);
    const orden = String(c.query.get('orden') || 'recientes');
    const autor = await uno(c.pool, 'SELECT user_id FROM posts WHERE id = $1', [postId]);
    const autorId = autor ? Number(autor.user_id) : 0;

    // «Mejores» = las reacciones primero, después lo más nuevo. El fijado
    // siempre va arriba, y luego el autor del comentario con más reacciones.
    const filas = await c.pool.query(
      `SELECT cm.id, cm.content, cm.created_at::text AS created_at, cm.user_id,
              cm.parent_id, cm.pinned_at::text AS pinned_at,
              (SELECT COUNT(*)::int FROM comment_likes cl WHERE cl.comment_id = cm.id) AS reacciones,
              u.username, u.display_name, u.avatar_url, u.is_verified
         FROM comments cm JOIN users u ON u.id = cm.user_id
        WHERE cm.post_id = $1 AND cm.status = 'active'
        ORDER BY cm.pinned_at DESC NULLS LAST,
                 ${orden === 'mejores' ? 'reacciones DESC, cm.created_at DESC' : 'cm.created_at ASC'}
        LIMIT 200`,
      [postId]
    );
    const lista = filas.rows.map((f) => ({
      id: Number(f.id),
      content: f.content,
      created_at: f.created_at,
      parent_id: f.parent_id ? Number(f.parent_id) : null,
      pinned: !!f.pinned_at,
      es_autor: Number(f.user_id) === autorId,
      username: f.username,
      display_name: f.display_name || f.username,
      avatar_url: f.avatar_url,
      is_verified: !!f.is_verified,
      is_mine: Number(f.user_id) === yoId,
    }));
    return conReaccionesComentarios(c.pool, lista, yoId);
  });

  router.post('/api/posts/:id/comments', async (c) => {
    const yo = await c.exigir();
    if (demasiadoRapido(`comentario:${yo.id}`, 20, 60_000)) {
      throw new ApiErr('Vas demasiado rápido: espera unos segundos', 429, 'rate_limit');
    }
    const b = await c.cuerpo();
    const contenido = texto(b.content, { min: 1, max: 1000, campo: 'comentario' });
    const postId = Number(c.params.id);
    const post = await uno(
      c.pool,
      `SELECT p.id, p.user_id, u.who_can_comment
         FROM posts p JOIN users u ON u.id = p.user_id
        WHERE p.id = $1 AND p.status = 'active'`,
      [postId]
    );
    if (!post) throw new ApiErr('Publicación no encontrada', 404);

    // Cada quien decide quién puede comentar lo suyo (Ajustes → Privacidad).
    const permiso = post.who_can_comment || 'all';
    if (Number(post.user_id) !== Number(yo.id)) {
      if (permiso === 'nobody') {
        throw new ApiErr('Esta persona decidió que nadie comente sus publicaciones', 403, 'comments_off');
      }
      if (permiso === 'following') {
        // «Quienes me siguen» = esta persona me sigue a mí.
        const meSigue = await uno(
          c.pool,
          'SELECT 1 FROM follows WHERE follower_id = $1 AND following_id = $2',
          [yo.id, post.user_id]
        );
        if (!meSigue) throw new ApiErr('Solo quienes siguen a esta persona pueden comentar', 403, 'comments_followers');
      }
    }

    const creado = await uno(
      c.pool,
      `INSERT INTO comments (post_id, user_id, parent_id, content) VALUES ($1, $2, $3, $4) RETURNING *`,
      [postId, yo.id, b.parent_id ? Number(b.parent_id) : null, contenido]
    );
    await c.pool.query('UPDATE posts SET comments_count = comments_count + 1 WHERE id = $1', [postId]);
    await sumarEstadistica(c.pool, 'new_comments');
    await notificar(c.pool, {
      userId: Number(post.user_id), tipo: 'comment', deUserId: Number(yo.id), postId, commentId: Number(creado.id),
      contenido: `comentó: «${contenido.slice(0, 80)}»`,
    });
    // Menciones dentro del comentario: aviso directo a quien se nombró.
    for (const nombre of mencionesDe(contenido)) {
      const mencionado = await uno(c.pool, 'SELECT id FROM users WHERE LOWER(username) = $1', [nombre]);
      if (!mencionado || Number(mencionado.id) === Number(yo.id)) continue;
      await notificar(c.pool, {
        userId: Number(mencionado.id), tipo: 'mention', deUserId: Number(yo.id),
        postId, commentId: Number(creado.id), contenido: 'te mencionó en un comentario',
      });
    }
    const [conReac] = await conReaccionesComentarios(c.pool, [{
      id: Number(creado.id),
      content: creado.content,
      created_at: String(creado.created_at),
      parent_id: creado.parent_id ? Number(creado.parent_id) : null,
      pinned: false,
      es_autor: Number(post.user_id) === Number(yo.id),
      username: yo.username,
      display_name: yo.display_name || yo.username,
      avatar_url: yo.avatar_url,
      is_verified: !!yo.is_verified,
      is_mine: true,
    }], Number(yo.id));
    return conReac;
  });

  router.del('/api/comments/:id', async (c) => {
    const yo = await c.exigir();
    const r = await c.pool.query(
      `UPDATE comments SET status = 'deleted' WHERE id = $1 AND user_id = $2 AND status = 'active'`,
      [Number(c.params.id), yo.id]
    );
    if (r.rowCount === 0) throw new ApiErr('No puedes eliminar este comentario', 404);
    await c.pool.query(
      'UPDATE posts SET comments_count = GREATEST(0, comments_count - 1) WHERE id = (SELECT post_id FROM comments WHERE id = $1)',
      [Number(c.params.id)]
    );
    return { ok: true };
  });

  // ---------- Personas ----------
  router.get('/api/users/suggestions', async (c) => {
    const yo = await c.exigir();
    const filas = await c.pool.query(
      `SELECT u.* FROM users u
        WHERE u.id <> $1 AND u.status = 'active' AND u.searchable <> FALSE
          AND NOT EXISTS (SELECT 1 FROM follows f WHERE f.follower_id = $1 AND f.following_id = u.id)
          AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.blocker_id = $1 AND b.blocked_id = u.id) OR (b.blocker_id = u.id AND b.blocked_id = $1))
        ORDER BY u.followers_count DESC, u.created_at DESC LIMIT 5`,
      [yo.id]
    );
    return filas.rows.map((u) =>
      perfilPublico(u, {
        // Van ordenados por popularidad y se sugiere seguirlos.
      })
    );
  });

  router.get('/api/users/:id', async (c) => {
    const yo = await c.exigir();
    const objetivo = await uno(c.pool, 'SELECT * FROM users WHERE id = $1', [Number(c.params.id)]);
    if (!objetivo) throw new ApiErr('Usuario no encontrado', 404);
    const bloqueo = await uno(
      c.pool,
      'SELECT 1 FROM blocks WHERE (blocker_id = $1 AND blocked_id = $2) OR (blocker_id = $2 AND blocked_id = $1)',
      [yo.id, objetivo.id]
    );
    if (bloqueo) throw new ApiErr('No puedes ver este perfil', 403, 'blocked');
    const siguiendo = await uno(c.pool, 'SELECT 1 FROM follows WHERE follower_id = $1 AND following_id = $2', [yo.id, objetivo.id]);
    if (objetivo.is_private && !siguiendo && Number(objetivo.id) !== Number(yo.id)) {
      const pedida = await uno(
        c.pool,
        `SELECT 1 AS pedida FROM follow_requests WHERE solicitante_id = $1 AND destino_id = $2 AND estado = 'pendiente'`,
        [yo.id, objetivo.id]
      );
      return {
        ...perfilPublico(objetivo, { siguiendo: false }),
        posts_count: 0,
        is_private: true,
        solicitud_enviada: !!pedida,
      };
    }
    return perfilPublico(objetivo, { siguiendo: !!siguiendo });
  });

  router.get('/api/users/:id/posts', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const objetivo = Number(c.params.id);
    const filas = await c.pool.query(
      `${SQL_POST} WHERE p.user_id = $2 AND p.status = 'active'
        ORDER BY p.pinned_at DESC NULLS LAST, p.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id, objetivo]
    );
    const total = await uno(
      c.pool,
      "SELECT COUNT(*)::int AS count FROM posts WHERE user_id = $1 AND status = 'active'",
      [objetivo]
    );
    return conPagina(
      c,
      await conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yo.id)), yo.id),
      Number(total?.count || 0), page, limit
    );
  });

  router.post('/api/users/:id/follow', async (c) => {
    const yo = await c.exigir();
    const objetivo = Number(c.params.id);
    if (objetivo === Number(yo.id)) throw new ApiErr('No puedes seguirte a ti mismo', 400);
    const existe = await uno(c.pool, 'SELECT id, username, display_name, is_private FROM users WHERE id = $1', [objetivo]);
    if (!existe) throw new ApiErr('Usuario no encontrado', 404);

    // Cuenta privada: no se entra por la puerta, se pide permiso.
    if (existe.is_private) {
      const ya = await uno(
        c.pool,
        `SELECT 1 AS ya FROM follows WHERE follower_id = $1 AND following_id = $2`,
        [yo.id, objetivo]
      );
      if (ya) return { ok: true, is_following: true, solicitado: false };
      const pedida = await uno(
        c.pool,
        `SELECT 1 AS pedida FROM follow_requests WHERE solicitante_id = $1 AND destino_id = $2 AND estado = 'pendiente'`,
        [yo.id, objetivo]
      );
      if (!pedida) {
        await c.pool.query(
          'INSERT INTO follow_requests (solicitante_id, destino_id) VALUES ($1, $2)',
          [yo.id, objetivo]
        );
        await notificar(c.pool, {
          userId: objetivo, tipo: 'follow', deUserId: Number(yo.id),
          contenido: 'quiere seguirte',
        }).catch(() => {});
      }
      return { ok: true, is_following: false, solicitado: true, privada: true };
    }

    const r = await c.pool.query('INSERT INTO follows (follower_id, following_id) VALUES ($1, $2) ON CONFLICT DO NOTHING', [yo.id, objetivo]);
    if (r.rowCount > 0) {
      await c.pool.query('UPDATE users SET following_count = following_count + 1 WHERE id = $1', [yo.id]);
      await c.pool.query('UPDATE users SET followers_count = followers_count + 1 WHERE id = $1', [objetivo]);
      await sumarEstadistica(c.pool, 'new_follows');
      await notificar(c.pool, { userId: objetivo, tipo: 'follow', deUserId: Number(yo.id), contenido: 'empezó a seguirte' });
    }
    return { ok: true, is_following: true };
  });

  router.del('/api/users/:id/follow', async (c) => {
    const yo = await c.exigir();
    const objetivo = Number(c.params.id);
    // Si había una solicitud esperando, se retira con el mismo gesto.
    await c.pool.query(
      `DELETE FROM follow_requests WHERE solicitante_id = $1 AND destino_id = $2 AND estado = 'pendiente'`,
      [yo.id, objetivo]
    );
    const r = await c.pool.query('DELETE FROM follows WHERE follower_id = $1 AND following_id = $2', [yo.id, objetivo]);
    if (r.rowCount > 0) {
      await c.pool.query('UPDATE users SET following_count = GREATEST(0, following_count - 1) WHERE id = $1', [yo.id]);
      await c.pool.query('UPDATE users SET followers_count = GREATEST(0, followers_count - 1) WHERE id = $1', [objetivo]);
    }
    return { ok: true, is_following: false };
  });

  router.post('/api/users/:id/block', async (c) => {
    const yo = await c.exigir();
    const objetivo = Number(c.params.id);
    if (objetivo === Number(yo.id)) throw new ApiErr('No puedes bloquearte a ti mismo', 400);
    await c.pool.query('INSERT INTO blocks (blocker_id, blocked_id) VALUES ($1, $2) ON CONFLICT DO NOTHING', [yo.id, objetivo]);
    // Bloquear corta el seguimiento en ambos sentidos.
    await c.pool.query('DELETE FROM follows WHERE (follower_id = $1 AND following_id = $2) OR (follower_id = $2 AND following_id = $1)', [yo.id, objetivo]);
    await auditar(c.pool, Number(yo.id), 'usuario_bloqueado', `#${objetivo}`, c.ip);
    return { ok: true, is_blocked: true };
  });

  router.del('/api/users/:id/block', async (c) => {
    const yo = await c.exigir();
    await c.pool.query('DELETE FROM blocks WHERE blocker_id = $1 AND blocked_id = $2', [yo.id, Number(c.params.id)]);
    return { ok: true, is_blocked: false };
  });

  // ---------- Búsqueda y tendencias ----------
  router.get('/api/search', async (c) => {
    const yo = await c.exigir();
    const consulta = texto(qs(c.req, 'q', ''), { min: 0, max: 80, campo: 'búsqueda' });
    const tipo = qs(c.req, 'type', 'users');
    // Lo que se busca queda en el historial (se puede borrar desde Ajustes).
    if (consulta && consulta.trim().length >= 2 && qs(c.req, 'historial', '1') !== '0') {
      const termino = consulta.trim();
      c.pool.query('DELETE FROM search_history WHERE user_id = $1 AND termino = $2', [yo.id, termino])
        .then(() => c.pool.query('INSERT INTO search_history (user_id, termino, tipo) VALUES ($1, $2, $3)', [yo.id, termino, tipo]))
        .catch(() => {});
    }
    if (!consulta) return [];

    if (tipo === 'posts') {
      const orden = qs(c.req, 'orden', 'recientes') === 'populares'
        ? '(p.likes_count * 3 + p.comments_count * 4) DESC, p.created_at DESC'
        : 'p.created_at DESC';
      const filas = await c.pool.query(
        `${SQL_POST} WHERE p.status = 'active' AND ${SQL_NO_OCULTOS} AND (p.content ILIKE $2 OR EXISTS (
            SELECT 1 FROM post_hashtags ph JOIN hashtags h ON h.id = ph.hashtag_id
             WHERE ph.post_id = p.id AND h.tag ILIKE $3))
           ORDER BY ${orden} LIMIT 40`,
        [yo.id, `%${consulta}%`, `%${consulta.replace(/^#/, '')}%`]
      );
      return conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yo.id)), yo.id);
    }

    // Grupos: se buscan por nombre o por su descripción.
    if (tipo === 'groups') {
      const resultado = await c.pool.query(
        `SELECT g.id, g.name, g.about, g.privacy, g.created_at::text AS created_at, g.owner_id,
                (SELECT COUNT(*)::int FROM group_members gm WHERE gm.group_id = g.id) AS miembros,
                (gm.user_id IS NOT NULL) AS soy_miembro
           FROM groups g
           LEFT JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $1
          WHERE g.name ILIKE $2 OR g.about ILIKE $2
          ORDER BY (SELECT COUNT(*) FROM group_members gm2 WHERE gm2.group_id = g.id) DESC
          LIMIT 30`,
        [yo.id, `%${consulta}%`]);
      return resultado.rows.map((g) => ({
        id: Number(g.id), name: g.name, about: g.about || '', privacy: g.privacy,
        miembros: Number(g.miembros || 0), soy_miembro: !!g.soy_miembro,
        created_at: String(g.created_at || ''),
      }));
    }

    // Etiquetas: lo que la gente usa, filtrado por lo que escribiste.
    if (tipo === 'tags') {
      const resultado = await c.pool.query(
        `SELECT h.tag, COUNT(ph.post_id)::int AS posts_count
           FROM hashtags h LEFT JOIN post_hashtags ph ON ph.hashtag_id = h.id
          WHERE h.tag ILIKE $1
          GROUP BY h.tag ORDER BY posts_count DESC, h.tag ASC LIMIT 30`,
        [`%${consulta.replace(/^#/, '')}%`]);
      return resultado.rows.map((t) => ({ tag: t.tag, posts_count: Number(t.posts_count || 0) }));
    }

    const filas = await c.pool.query(
      `SELECT u.* FROM users u
        WHERE u.status = 'active' AND u.searchable <> FALSE
          AND (u.username ILIKE $1 OR u.display_name ILIKE $1)
        ORDER BY (LOWER(u.username) = LOWER($2)) DESC, u.followers_count DESC LIMIT 40`,
      [`%${consulta}%`, consulta]
    );
    return filas.rows.map((u) => perfilPublico(u));
  });

  // Cuánto ocupa tu cuenta: para la pestaña «Datos» de Ajustes.
  router.get('/api/me/resumen', async (c) => {
    const yo = await c.exigir();
    const fila = await uno(c.pool,
      `SELECT
         (SELECT COUNT(*)::int FROM posts WHERE user_id = $1 AND status = 'active') AS publicaciones,
         (SELECT COUNT(*)::int FROM comments WHERE user_id = $1) AS comentarios,
         (SELECT COALESCE(SUM(images_count), 0)::int FROM users WHERE id = $1) AS fotos,
         (SELECT COALESCE(SUM(images_bytes), 0)::bigint FROM users WHERE id = $1) AS bytes,
         (SELECT COUNT(*)::int FROM group_members WHERE user_id = $1) AS grupos,
         (SELECT COUNT(*)::int FROM messages WHERE sender_id = $1) AS mensajes,
         (SELECT COUNT(*)::int FROM media WHERE user_id = $1) AS archivos,
         (SELECT COUNT(*)::int FROM follows WHERE follower_id = $1) AS siguiendo,
         (SELECT COUNT(*)::int FROM follows WHERE following_id = $1) AS seguidores,
         (SELECT COUNT(*)::int FROM post_images pi JOIN posts p ON p.id = pi.post_id WHERE p.user_id = $1) AS imagenes_en_publicaciones`,
      [yo.id]);
    return {
      publicaciones: Number(fila?.publicaciones || 0),
      comentarios: Number(fila?.comentarios || 0),
      fotos: Number(fila?.fotos || 0),
      megabytes: Math.round((Number(fila?.bytes || 0) / (1024 * 1024)) * 10) / 10,
      grupos: Number(fila?.grupos || 0),
      mensajes: Number(fila?.mensajes || 0),
      archivos: Number(fila?.archivos || 0),
      siguiendo: Number(fila?.siguiendo || 0),
      seguidores: Number(fila?.seguidores || 0),
      imagenes_en_publicaciones: Number(fila?.imagenes_en_publicaciones || 0),
    };
  });

  router.get('/api/hashtags', async (c) => {
    await c.exigir();
    const filas = await c.pool.query(
      `SELECT tag, posts_count, last_used_at::text AS last_used_at FROM hashtags
        ORDER BY posts_count DESC, last_used_at DESC LIMIT 6`
    );
    return filas.rows.map((f) => ({ tag: f.tag, posts_count: Number(f.posts_count), last_used_at: f.last_used_at }));
  });

  // ---------- Guardados ----------
  router.get('/api/me/saved', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const filas = await c.pool.query(
      `${SQL_POST} JOIN saves sv ON sv.post_id = p.id AND sv.user_id = $1
        WHERE p.status = 'active' ORDER BY sv.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id]
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS count FROM saves WHERE user_id = $1', [yo.id]);
    return conPagina(
      c,
      await conContenido(c.pool, filas.rows.map((f) => aPublicacion(f, yo.id)), yo.id),
      Number(total?.count || 0), page, limit
    );
  });

  // ---------- Notificaciones ----------
  router.get('/api/notifications', async (c) => {
    const yo = await c.exigir();
    const { page, limit, offset } = paginacion(c.req, 30, 50);
    const filas = await c.pool.query(
      `SELECT n.id, n.type, n.is_read, n.content, n.created_at::text AS created_at,
              n.from_user_id, n.post_id, n.comment_id,
              u.username AS from_username, u.display_name AS from_display_name, u.avatar_url AS from_avatar_url
         FROM notifications n LEFT JOIN users u ON u.id = n.from_user_id
        WHERE n.user_id = $1 ORDER BY n.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [yo.id]
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS count FROM notifications WHERE user_id = $1', [yo.id]);
    return {
      items: filas.rows.map((f) => ({
        id: Number(f.id),
        type: f.type,
        content: f.content,
        is_read: !!f.is_read,
        created_at: f.created_at,
        post_id: f.post_id ? Number(f.post_id) : null,
        comment_id: f.comment_id ? Number(f.comment_id) : null,
        from_user_id: f.from_user_id ? Number(f.from_user_id) : null,
        from_username: f.from_username,
        from_display_name: f.from_display_name || f.from_username,
        from_avatar_url: f.from_avatar_url,
      })),
      total: Number(total?.count || 0),
      page,
      limit,
    };
  });

  router.post('/api/notifications/read-all', async (c) => {
    const yo = await c.exigir();
    await c.pool.query('UPDATE notifications SET is_read = TRUE WHERE user_id = $1 AND is_read = FALSE', [yo.id]);
    return { ok: true };
  });

  router.post('/api/notifications/:id/read', async (c) => {
    const yo = await c.exigir();
    await c.pool.query('UPDATE notifications SET is_read = TRUE WHERE id = $1 AND user_id = $2', [
      Number(c.params.id), yo.id,
    ]);
    return { ok: true };
  });

  // ---------- Quién reaccionó (prueba social real) ----------
  // Devuelve las últimas personas que dieron «me gusta» y el total, para
  // poder escribir «A María y 23 más les gusta» con datos de verdad.
  router.get('/api/posts/:id/likes', async (c) => {
    const yo = await c.exigir();
    const postId = Number(c.params.id);
    if (!postId) throw new ApiErr('Falta la publicación', 400);
    const total = await uno(
      c.pool,
      'SELECT COUNT(*)::int AS count FROM likes WHERE post_id = $1',
      [postId]
    );
    const filas = await c.pool.query(
      `SELECT u.id, u.username, u.display_name, u.avatar_url, u.is_verified
         FROM likes l JOIN users u ON u.id = l.user_id
        WHERE l.post_id = $1
        ORDER BY l.created_at DESC LIMIT 4`,
      [postId]
    );
    void yo;
    return {
      total: Number(total?.count || 0),
      items: filas.rows.map((u) => ({
        id: Number(u.id),
        username: u.username,
        display_name: u.display_name || u.username,
        avatar_url: u.avatar_url,
        is_verified: !!u.is_verified,
      })),
    };
  });

  // ---------- Mis números ----------
  // Un resumen honesto de la cuenta: lo que hay en la base, sin inventar.
  router.get('/api/me/stats', async (c) => {
    const yo = await c.exigir();
    const f = await uno(
      c.pool,
      `SELECT
         (SELECT COUNT(*)::int FROM posts WHERE user_id = $1 AND status = 'active') AS posts,
         (SELECT COUNT(*)::int FROM comments WHERE user_id = $1 AND status = 'active') AS comments,
         (SELECT COUNT(*)::int FROM saves WHERE user_id = $1) AS guardados,
         (SELECT COUNT(*)::int FROM likes l JOIN posts p ON p.id = l.post_id WHERE p.user_id = $1 AND p.status = 'active') AS me_gusta_recibidos,
         (SELECT COUNT(*)::int FROM posts p JOIN likes l ON l.post_id = p.id WHERE p.user_id = $1) AS reacciones,
         (SELECT COALESCE(SUM(p.comments_count), 0)::int FROM posts p WHERE p.user_id = $1 AND p.status = 'active') AS comentarios_recibidos,
         (SELECT COUNT(*)::int FROM conversations WHERE user_a = $1 OR user_b = $1) AS conversaciones,
         (SELECT COUNT(*)::int FROM messages WHERE sender_id = $1 AND status <> 'deleted') AS mensajes,
         (SELECT COUNT(*)::int FROM stories WHERE user_id = $1 AND expires_at > NOW()) AS historias,
         (SELECT COUNT(*)::int FROM group_members WHERE user_id = $1) AS grupos,
         (SELECT COUNT(*)::int FROM follows WHERE following_id = $1) AS seguidores,
         (SELECT COUNT(*)::int FROM follows WHERE follower_id = $1) AS siguiendo`,
      [yo.id]
    );
    return {
      posts: Number(f?.posts || 0),
      comentarios: Number(f?.comments || 0),
      guardados: Number(f?.guardados || 0),
      me_gusta_recibidos: Number(f?.me_gusta_recibidos || 0),
      reacciones: Number(f?.reacciones || 0),
      comentarios_recibidos: Number(f?.comentarios_recibidos || 0),
      conversaciones: Number(f?.conversaciones || 0),
      mensajes: Number(f?.mensajes || 0),
      historias: Number(f?.historias || 0),
      grupos: Number(f?.grupos || 0),
      seguidores: Number(f?.seguidores || 0),
      siguiendo: Number(f?.siguiendo || 0),
    };
  });

  // ---------- Personas que bloqueé ----------
  // Hasta ahora se podía bloquear pero no ver ni deshacer la lista.
  router.get('/api/me/blocked', async (c) => {
    const yo = await c.exigir();
    const filas = await c.pool.query(
      `SELECT u.id, u.username, u.display_name, u.avatar_url, u.is_verified, b.created_at::text AS blocked_at
         FROM blocks b JOIN users u ON u.id = b.blocked_id
        WHERE b.blocker_id = $1
        ORDER BY b.created_at DESC`,
      [yo.id]
    );
    return filas.rows.map((u) => ({
      id: Number(u.id),
      username: u.username,
      display_name: u.display_name || u.username,
      avatar_url: u.avatar_url,
      is_verified: !!u.is_verified,
      blocked_at: u.blocked_at,
    }));
  });

  // ---------- Reportes ----------
  router.post('/api/reports', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const tipo = ['post', 'comment', 'user', 'message'].includes(b.target_type) ? b.target_type : 'post';
    const objetivo = Number(b.target_id);
    const motivo = texto(b.reason, { min: 2, max: 80, campo: 'motivo' });
    const detalle = typeof b.detail === 'string' ? b.detail.slice(0, 500) : '';
    if (!objetivo) throw new ApiErr('Falta el elemento reportado', 400);
    await c.pool.query(
      'INSERT INTO reports (reporter_id, target_type, target_id, reason, detail) VALUES ($1, $2, $3, $4, $5)',
      [yo.id, tipo, objetivo, motivo, detalle]
    );
    await auditar(c.pool, Number(yo.id), 'reporte_creado', `${tipo} ${objetivo}`, c.ip);
    return { ok: true };
  });
}
