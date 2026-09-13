// Moon — Grupos
// ============================================================
// Grupos reales: se crean, se entra, se sale y se publica dentro. Cada grupo
// guarda su nombre, su descripción, su privacidad y la lista de miembros con
// su papel. Las publicaciones de un grupo son las mismas de siempre (me gusta,
// comentarios y guardados siguen funcionando), solo que con `group_id`.

import { ApiErr, texto, paginacion, qs, hashtagsDe, mencionesDe } from './util.mjs';
import { fila, uno, filas } from './db.mjs';
import { auditar } from './db.mjs';
import { notificar } from './ws.mjs';
import { demasiadoRapido } from './limites.mjs';
import { SQL_POST, conImagenes, aPublicacion } from './rutas-social.mjs';

/** Convierte un nombre en su dirección corta (solo letras, números y guiones). */
function aSlug(nombre) {
  const base = String(nombre || '')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40);
  return base || 'grupo';
}

function aGrupo(g, yo) {
  return {
    id: Number(g.id),
    name: g.name,
    slug: g.slug,
    about: g.about || '',
    cover_url: g.cover_url || '',
    privacy: g.privacy,
    miembros: Number(g.members_count || 0),
    publicaciones: Number(g.posts_count || 0),
    es_mio: Number(g.owner_id) === Number(yo),
    soy_miembro: !!g.soy_miembro,
    mi_papel: g.mi_papel || '',
    owner_username: g.owner_username || '',
    owner_display_name: g.owner_display_name || g.owner_username || '',
    created_at: g.created_at,
  };
}

export function registrarRutasGrupos(router) {
  // ---------- Listas ----------
  // Mis grupos y los que puedo descubrir, en una sola llamada.
  router.get('/api/groups', async (c) => {
    const yo = await c.exigir();
    const buscar = String(c.query.get('q') || '').trim();

    const comunes = `
      SELECT g.*, u.username AS owner_username, u.display_name AS owner_display_name,
             (gm.user_id IS NOT NULL) AS soy_miembro, COALESCE(gm.role, '') AS mi_papel
        FROM groups g
        JOIN users u ON u.id = g.owner_id
        LEFT JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $1`;

    const mios = await filas(
      c.pool,
      `${comunes}
       WHERE gm.user_id IS NOT NULL
       ORDER BY g.created_at DESC
       LIMIT 50`,
      [yo.id]
    );

    const parametros = [yo.id];
    let filtro = 'WHERE gm.user_id IS NULL';
    if (buscar) {
      parametros.push(`%${buscar}%`);
      filtro += ` AND (g.name ILIKE $${parametros.length} OR g.about ILIKE $${parametros.length})`;
    }

    const otros = await filas(
      c.pool,
      `${comunes}
       ${filtro}
       ORDER BY g.members_count DESC, g.created_at DESC
       LIMIT 30`,
      parametros
    );

    return {
      mios: mios.map((g) => aGrupo(g, yo.id)),
      descubrir: otros.map((g) => aGrupo(g, yo.id)),
    };
  });

  // ---------- Crear ----------
  router.post('/api/groups', async (c) => {
    const yo = await c.exigir();
    if (demasiadoRapido(`grupo:${yo.id}`, 5, 300_000)) {
      throw new ApiErr('Has creado varios grupos seguidos: espera un momento', 429, 'rate_limit');
    }
    const b = await c.cuerpo();
    const nombre = texto(b.name || '', { min: 3, max: 80, campo: 'nombre' }).trim();
    const about = typeof b.about === 'string' ? b.about.slice(0, 400).trim() : '';
    const privacidad = ['public', 'private'].includes(b.privacy) ? b.privacy : 'public';

    // La dirección corta debe ser única: si ya existe, se le añade un número.
    let slug = aSlug(nombre);
    let intento = 0;
    while (await uno(c.pool, 'SELECT 1 FROM groups WHERE slug = $1', [slug])) {
      intento += 1;
      slug = `${aSlug(nombre)}-${intento + 1}`;
      if (intento > 40) {
        slug = `${aSlug(nombre)}-${Date.now().toString(36)}`;
        break;
      }
    }

    const grupo = await uno(
      c.pool,
      `INSERT INTO groups (owner_id, name, slug, about, privacy)
       VALUES ($1, $2, $3, $4, $5) RETURNING *`,
      [yo.id, nombre, slug, about, privacidad]
    );
    await c.pool.query(
      `INSERT INTO group_members (group_id, user_id, role) VALUES ($1, $2, 'owner') ON CONFLICT DO NOTHING`,
      [grupo.id, yo.id]
    );
    await auditar(c.pool, Number(yo.id), 'grupo_creado', `#${grupo.id} ${nombre}`, c.ip);
    return aGrupo({ ...grupo, soy_miembro: true, mi_papel: 'owner' }, yo.id);
  });

  // ---------- Detalle ----------
  router.get('/api/groups/:id', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    if (!Number.isFinite(id)) throw new ApiErr('Grupo no encontrado', 404);

    const grupo = await uno(
      c.pool,
      `SELECT g.*, u.username AS owner_username, u.display_name AS owner_display_name,
              (gm.user_id IS NOT NULL) AS soy_miembro, COALESCE(gm.role, '') AS mi_papel
         FROM groups g
         JOIN users u ON u.id = g.owner_id
         LEFT JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $2
        WHERE g.id = $1`,
      [id, yo.id]
    );
    if (!grupo) throw new ApiErr('Grupo no encontrado', 404);

    const miembros = await filas(
      c.pool,
      `SELECT gm.role, gm.joined_at::text AS joined_at,
              u.id, u.username, u.display_name, u.avatar_url, u.is_verified
         FROM group_members gm JOIN users u ON u.id = gm.user_id
        WHERE gm.group_id = $1
        ORDER BY CASE gm.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, gm.joined_at ASC
        LIMIT 60`,
      [id]
    );

    return {
      ...aGrupo(grupo, yo.id),
      miembros_lista: miembros.map((m) => ({
        id: Number(m.id),
        username: m.username,
        display_name: m.display_name || m.username,
        avatar_url: m.avatar_url,
        is_verified: !!m.is_verified,
        papel: m.role,
        desde: m.joined_at,
      })),
    };
  });

  // ---------- Editar (solo quien lo creó) ----------
  router.patch('/api/groups/:id', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const grupo = await uno(c.pool, 'SELECT owner_id FROM groups WHERE id = $1', [id]);
    if (!grupo) throw new ApiErr('Grupo no encontrado', 404);
    if (Number(grupo.owner_id) !== Number(yo.id)) throw new ApiErr('Solo quien creó el grupo puede editarlo', 403);

    const b = await c.cuerpo();
    const about = typeof b.about === 'string' ? b.about.slice(0, 400).trim() : null;
    const nombre = typeof b.name === 'string' ? texto(b.name, { min: 3, max: 80, campo: 'nombre' }).trim() : null;
    const privacidad = ['public', 'private'].includes(b.privacy) ? b.privacy : null;

    const actualizado = await uno(
      c.pool,
      `UPDATE groups SET
          name = COALESCE($2, name),
          about = COALESCE($3, about),
          privacy = COALESCE($4, privacy)
        WHERE id = $1 RETURNING *`,
      [id, nombre, about, privacidad]
    );
    return aGrupo({ ...actualizado, soy_miembro: true, mi_papel: 'owner' }, yo.id);
  });

  // ---------- Entrar y salir ----------
  router.post('/api/groups/:id/join', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const grupo = await uno(c.pool, 'SELECT id, owner_id, name FROM groups WHERE id = $1', [id]);
    if (!grupo) throw new ApiErr('Grupo no encontrado', 404);

    const r = await c.pool.query(
      `INSERT INTO group_members (group_id, user_id, role) VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING`,
      [id, yo.id]
    );
    if (r.rowCount > 0) {
      await c.pool.query('UPDATE groups SET members_count = members_count + 1 WHERE id = $1', [id]);
      if (Number(grupo.owner_id) !== Number(yo.id)) {
        await notificar(c.pool, {
          userId: Number(grupo.owner_id),
          tipo: 'group_join',
          deUserId: Number(yo.id),
          contenido: `se unió a ${grupo.name}`,
        });
      }
      await auditar(c.pool, Number(yo.id), 'grupo_ingresado', `#${id}`, c.ip);
    }
    return { ok: true, soy_miembro: true };
  });

  router.del('/api/groups/:id/join', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const grupo = await uno(c.pool, 'SELECT owner_id FROM groups WHERE id = $1', [id]);
    if (!grupo) throw new ApiErr('Grupo no encontrado', 404);
    if (Number(grupo.owner_id) === Number(yo.id)) {
      throw new ApiErr('Quien creó el grupo no puede salir de él', 400);
    }
    const r = await c.pool.query('DELETE FROM group_members WHERE group_id = $1 AND user_id = $2', [id, yo.id]);
    if (r.rowCount > 0) {
      await c.pool.query('UPDATE groups SET members_count = GREATEST(members_count - 1, 0) WHERE id = $1', [id]);
    }
    return { ok: true, soy_miembro: false };
  });

  // ---------- Publicaciones del grupo ----------
  router.get('/api/groups/:id/posts', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const { page, limit, offset } = paginacion(c.query);

    const grupo = await uno(
      c.pool,
      `SELECT g.id, g.privacy,
              (gm.user_id IS NOT NULL) AS soy_miembro
         FROM groups g
         LEFT JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $2
        WHERE g.id = $1`,
      [id, yo.id]
    );
    if (!grupo) throw new ApiErr('Grupo no encontrado', 404);
    if (grupo.privacy === 'private' && !grupo.soy_miembro) {
      throw new ApiErr('Este grupo es privado: hay que entrar para ver lo que se publica', 403);
    }

    const items = await filas(
      c.pool,
      `${SQL_POST}
        WHERE p.group_id = $2 AND p.status <> 'deleted'
        ORDER BY p.created_at DESC
        LIMIT $3 OFFSET $4`,
      [yo.id, id, limit, offset]
    );
    await conImagenes(c.pool, items);
    const total = Number((await uno(c.pool, `SELECT COUNT(*)::int AS n FROM posts WHERE group_id = $1 AND status <> 'deleted'`, [id])).n);
    return { items: items.map((f) => aPublicacion(f, yo.id)), total, page, limit };
  });

  router.post('/api/groups/:id/posts', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    if (demasiadoRapido(`post:${yo.id}`, 10, 60_000)) {
      throw new ApiErr('Vas demasiado rápido: espera unos segundos', 429, 'rate_limit');
    }

    const grupo = await uno(
      c.pool,
      `SELECT g.id, g.privacy, g.name, (gm.user_id IS NOT NULL) AS soy_miembro
         FROM groups g
         LEFT JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $2
        WHERE g.id = $1`,
      [id, yo.id]
    );
    if (!grupo) throw new ApiErr('Grupo no encontrado', 404);
    if (!grupo.soy_miembro) throw new ApiErr('Entra al grupo para publicar en él', 403);

    const b = await c.cuerpo();
    const contenido = texto(b.content || '', { min: 0, max: 2000, campo: 'contenido' }).trim();
    const imagenes = Array.isArray(b.images) ? b.images.slice(0, 4) : [];
    if (!contenido && imagenes.length === 0) throw new ApiErr('Escribe algo o adjunta una imagen', 400);

    const bloqueadas = (await c.pool.query('SELECT word FROM blocked_words')).rows.map((r) => r.word.toLowerCase());
    const enMinusculas = ` ${contenido.toLowerCase()} `;
    const encontrada = bloqueadas.find((w) => w && enMinusculas.includes(` ${w} `));
    if (encontrada) throw new ApiErr(`El texto contiene una palabra no permitida: «${encontrada}»`, 400, 'blocked_word');

    const creado = await uno(
      c.pool,
      `INSERT INTO posts (user_id, content, group_id) VALUES ($1, $2, $3) RETURNING *`,
      [yo.id, contenido, id]
    );
    for (const [i, im] of imagenes.entries()) {
      const url = typeof im === 'string' ? im : im.url || im.original_url || '';
      if (!url) continue;
      await c.pool.query(
        'INSERT INTO post_images (post_id, position, original_url, thumb_url) VALUES ($1, $2, $3, $3)',
        [creado.id, i, url]
      );
    }
    await c.pool.query('UPDATE groups SET posts_count = posts_count + 1 WHERE id = $1', [id]);
    await c.pool.query('UPDATE users SET posts_count = posts_count + 1 WHERE id = $1', [yo.id]);

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
      const mencionado = await uno(c.pool, 'SELECT id FROM users WHERE username = $1', [nombre]);
      if (mencionado && Number(mencionado.id) !== Number(yo.id)) {
        await notificar(c.pool, {
          userId: Number(mencionado.id),
          tipo: 'mention',
          deUserId: Number(yo.id),
          postId: Number(creado.id),
          contenido: 'te mencionó en el grupo',
        });
      }
    }

    const completa = await fila(c.pool, `${SQL_POST} WHERE p.id = $2`, [yo.id, creado.id]);
    await conImagenes(c.pool, [completa]);

    // Aviso a los miembros del grupo (los que estén en línea lo reciben al momento).
    const miembros = await filas(
      c.pool,
      'SELECT user_id FROM group_members WHERE group_id = $1 AND user_id <> $2 LIMIT 200',
      [id, yo.id]
    );
    for (const m of miembros) {
      await notificar(c.pool, {
        userId: Number(m.user_id),
        tipo: 'group_post',
        deUserId: Number(yo.id),
        postId: Number(creado.id),
        contenido: `publicó en ${grupo.name}`,
      });
    }

    return aPublicacion(completa, yo.id);
  });

  // ---------- Mis grupos (para el menú lateral) ----------
  router.get('/api/me/groups', async (c) => {
    const yo = await c.exigir();
    const lista = await filas(
      c.pool,
      `SELECT g.id, g.name, g.slug, g.members_count, gm.role
         FROM group_members gm JOIN groups g ON g.id = gm.group_id
        WHERE gm.user_id = $1
        ORDER BY gm.joined_at DESC
        LIMIT 20`,
      [yo.id]
    );
    return lista.map((g) => ({
      id: Number(g.id),
      name: g.name,
      slug: g.slug,
      miembros: Number(g.members_count),
      papel: g.role,
    }));
  });
}

// Reexportado para quien necesite las etiquetas de la búsqueda.
export { qs };
