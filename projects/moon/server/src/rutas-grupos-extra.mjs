// Moon — Grupos, segunda parte (tanda 3)
// ============================================================
// Lo que le faltaba a un grupo para ser un grupo de verdad:
//   · eventos con «voy / quizás / no voy» y quién se apuntó
//   · archivos compartidos (nombre, tipo, peso y enlace)
//   · reglas del grupo y anuncio fijado arriba
//   · preguntas para entrar: la solicitud se aprueba o se rechaza
//   · sanciones (aviso, silencio, expulsión) y registro de lo que pasa
//
// Todo se apoya en las tablas de la migración 30. Quien creó el grupo y sus
// administradores mandan; el resto participa.

import { ApiErr, texto } from './util.mjs';
import { uno, filas } from './db.mjs';
import { auditar } from './db.mjs';
import { notificar, enviarA } from './ws.mjs';

export const ROLES_MANDO = new Set(['owner', 'admin']);

/** Quién soy dentro del grupo y si tengo mando. */
async function miPapel(c, id) {
  const yo = await c.exigir();
  const grupo = await uno(
    c.pool,
    `SELECT g.*, COALESCE(gm.role, '') AS mi_papel
       FROM groups g
       LEFT JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $2
      WHERE g.id = $1`,
    [id, yo.id]
  );
  if (!grupo) throw new ApiErr('Grupo no encontrado', 404);
  const esMiembro = !!grupo.mi_papel;
  const mando = ROLES_MANDO.has(grupo.mi_papel);
  return { yo, grupo, esMiembro, mando };
}

/** Anota lo que pasa en el grupo (visible para quien lo administra). */
async function anotar(pool, groupId, userId, accion, detalle = '') {
  await pool
    .query('INSERT INTO group_log (group_id, user_id, accion, detalle) VALUES ($1, $2, $3, $4)', [
      groupId, userId, accion, String(detalle).slice(0, 300),
    ])
    .catch(() => {});
}

/** ¿Esta persona tiene un silencio vigente en el grupo? */
export async function silenciadoEn(pool, groupId, userId) {
  const s = await uno(
    pool,
    `SELECT id, hasta FROM group_sanctions
      WHERE group_id = $1 AND user_id = $2 AND tipo = 'silencio'
        AND (hasta IS NULL OR hasta > NOW())
      ORDER BY created_at DESC LIMIT 1`,
    [groupId, userId]
  );
  return s || null;
}

// ---------- Eventos ----------
function aEvento(e, yo) {
  const voy = Number(e.voy || 0);
  const quizas = Number(e.quizas || 0);
  const no = Number(e.no_van || 0);
  return {
    id: Number(e.id),
    group_id: Number(e.group_id),
    title: e.title,
    about: e.about || '',
    place: e.place || '',
    starts_at: e.starts_at,
    created_at: e.created_at,
    autor: e.autor_nombre || e.autor_usuario || '',
    autor_username: e.autor_usuario || '',
    voy,
    quizas,
    no_van: no,
    mi_estado: e.mi_estado || '',
    // Un evento pasado se marca solo: nadie tiene que borrarlo.
    pasado: new Date(e.starts_at).getTime() < Date.now(),
    mio: Number(e.user_id) === Number(yo),
  };
}

const SQL_EVENTOS = `
  SELECT e.*, e.starts_at::text AS starts_at, e.created_at::text AS created_at,
         u.username AS autor_usuario, u.display_name AS autor_nombre,
         (SELECT COUNT(*)::int FROM group_event_asistencias a WHERE a.event_id = e.id AND a.estado = 'voy') AS voy,
         (SELECT COUNT(*)::int FROM group_event_asistencias a WHERE a.event_id = e.id AND a.estado = 'quizas') AS quizas,
         (SELECT COUNT(*)::int FROM group_event_asistencias a WHERE a.event_id = e.id AND a.estado = 'no') AS no_van,
         (SELECT a.estado FROM group_event_asistencias a WHERE a.event_id = e.id AND a.user_id = $2) AS mi_estado
    FROM group_events e
    JOIN users u ON u.id = e.user_id`;

// ---------- Archivos ----------
function aArchivo(f, yo) {
  return {
    id: Number(f.id),
    group_id: Number(f.group_id),
    nombre: f.nombre,
    url: f.url,
    tipo: f.tipo || '',
    peso: Number(f.peso || 0),
    created_at: f.created_at,
    autor: f.autor_nombre || f.autor_usuario || '',
    autor_username: f.autor_usuario || '',
    // Se puede borrar lo que subí yo; quien administra, cualquiera.
    puedo_borrar: Number(f.user_id) === Number(yo) || !!f.mando,
  };
}

const SQL_ARCHIVOS = `
  SELECT f.*, f.created_at::text AS created_at,
         u.username AS autor_usuario, u.display_name AS autor_nombre,
         (gm.role IN ('owner', 'admin')) AS mando
    FROM group_files f
    JOIN users u ON u.id = f.user_id
    LEFT JOIN group_members gm ON gm.group_id = f.group_id AND gm.user_id = $2`;

// ---------- Preguntas y solicitudes ----------
function aSolicitud(s) {
  return {
    id: Number(s.id),
    group_id: Number(s.group_id),
    user_id: Number(s.user_id),
    username: s.username,
    display_name: s.display_name || s.username,
    avatar_url: s.avatar_url || '',
    answers: Array.isArray(s.answers) ? s.answers : [],
    estado: s.estado,
    created_at: s.created_at,
  };
}

export function registrarRutasGruposExtra(router) {
  // ============================================================
  // Eventos
  // ============================================================
  router.get('/api/groups/:id/events', async (c) => {
    const id = Number(c.params.id);
    const { yo, esMiembro, grupo } = await miPapel(c, id);
    if (!esMiembro && grupo.privacy === 'private') {
      throw new ApiErr('Este grupo es privado: entra para ver sus eventos', 403, 'privado');
    }
    const eventos = await filas(c.pool, `${SQL_EVENTOS} WHERE e.group_id = $1 ORDER BY e.starts_at ASC LIMIT 60`, [id, yo.id]);
    return { eventos: eventos.map((e) => aEvento(e, yo.id)) };
  });

  router.post('/api/groups/:id/events', async (c) => {
    const id = Number(c.params.id);
    const { yo, esMiembro, mando } = await miPapel(c, id);
    if (!esMiembro) throw new ApiErr('Entra al grupo para crear eventos', 403, 'no_miembro');
    if (!mando) throw new ApiErr('Solo quien administra el grupo puede crear eventos', 403, 'sin_mando');
    const b = await c.cuerpo();
    const titulo = texto(b.title || '', { min: 3, max: 120, campo: 'título' }).trim();
    const about = typeof b.about === 'string' ? b.about.slice(0, 600).trim() : '';
    const lugar = typeof b.place === 'string' ? b.place.slice(0, 160).trim() : '';
    const cuando = new Date(String(b.starts_at || ''));
    if (Number.isNaN(cuando.getTime())) throw new ApiErr('Ponle fecha y hora al evento', 400, 'sin_fecha');

    const creado = await uno(
      c.pool,
      `INSERT INTO group_events (group_id, user_id, title, about, place, starts_at)
       VALUES ($1, $2, $3, $4, $5, $6) RETURNING id`,
      [id, yo.id, titulo, about, lugar, cuando.toISOString()]
    );
    // Quien lo crea va de una: nadie crea un evento al que no piensa ir.
    await c.pool.query(
      `INSERT INTO group_event_asistencias (event_id, user_id, estado) VALUES ($1, $2, 'voy')
       ON CONFLICT DO NOTHING`,
      [creado.id, yo.id]
    );
    const e = await uno(c.pool, `${SQL_EVENTOS} WHERE e.id = $1`, [creado.id, yo.id]);
    await anotar(c.pool, id, yo.id, 'evento_creado', titulo);
    const miembros = await filas(c.pool, 'SELECT user_id FROM group_members WHERE group_id = $1 LIMIT 300', [id]);
    for (const m of miembros) {
      if (Number(m.user_id) === Number(yo.id)) continue;
      enviarA(Number(m.user_id), { type: 'group_event', group_id: id, title: titulo, starts_at: e.starts_at });
    }
    return aEvento(e, yo.id);
  });

  // «Voy», «quizás» o «no voy»: se cambia las veces que haga falta.
  router.post('/api/groups/:id/events/:evento/asistir', async (c) => {
    const id = Number(c.params.id);
    const eventoId = Number(c.params.evento);
    const { yo, esMiembro } = await miPapel(c, id);
    if (!esMiembro) throw new ApiErr('Entra al grupo para apuntarte', 403, 'no_miembro');
    const ev = await uno(c.pool, 'SELECT id, title FROM group_events WHERE id = $1 AND group_id = $2', [eventoId, id]);
    if (!ev) throw new ApiErr('Ese evento no existe en el grupo', 404, 'evento_no_existe');

    const b = await c.cuerpo().catch(() => ({}));
    const estado = ['voy', 'quizas', 'no'].includes(b?.estado) ? b.estado : 'voy';
    const antes = await uno(
      c.pool,
      'SELECT estado FROM group_event_asistencias WHERE event_id = $1 AND user_id = $2',
      [eventoId, yo.id]
    );
    if (antes && antes.estado === estado) {
      // Volver a pulsar el mismo botón quita la respuesta.
      await c.pool.query('DELETE FROM group_event_asistencias WHERE event_id = $1 AND user_id = $2', [eventoId, yo.id]);
    } else {
      await c.pool.query(
        `INSERT INTO group_event_asistencias (event_id, user_id, estado) VALUES ($1, $2, $3)
         ON CONFLICT (event_id, user_id) DO UPDATE SET estado = EXCLUDED.estado`,
        [eventoId, yo.id, estado]
      );
    }
    const e = await uno(c.pool, `${SQL_EVENTOS} WHERE e.id = $1`, [eventoId, yo.id]);
    await anotar(c.pool, id, yo.id, 'evento_respuesta', `${ev.title}: ${estado}`);
    return aEvento(e, yo.id);
  });

  // Quién se apuntó (con nombre y cara). Cualquier miembro lo ve.
  router.get('/api/groups/:id/events/:evento/asistentes', async (c) => {
    const id = Number(c.params.id);
    const eventoId = Number(c.params.evento);
    const { esMiembro, grupo } = await miPapel(c, id);
    if (!esMiembro && grupo.privacy === 'private') {
      throw new ApiErr('Este grupo es privado', 403, 'privado');
    }
    const gente = await filas(
      c.pool,
      `SELECT a.estado, u.id, u.username, u.display_name, u.avatar_url
         FROM group_event_asistencias a JOIN users u ON u.id = a.user_id
        WHERE a.event_id = $1
        ORDER BY CASE a.estado WHEN 'voy' THEN 0 WHEN 'quizas' THEN 1 ELSE 2 END, u.display_name`,
      [eventoId]
    );
    return {
      asistentes: gente.map((g) => ({
        id: Number(g.id),
        username: g.username,
        display_name: g.display_name || g.username,
        avatar_url: g.avatar_url || '',
        estado: g.estado,
      })),
    };
  });

  router.del('/api/groups/:id/events/:evento', async (c) => {
    const id = Number(c.params.id);
    const eventoId = Number(c.params.evento);
    const { yo, mando } = await miPapel(c, id);
    const ev = await uno(c.pool, 'SELECT id, user_id, title FROM group_events WHERE id = $1 AND group_id = $2', [eventoId, id]);
    if (!ev) throw new ApiErr('Ese evento no existe en el grupo', 404);
    if (!mando && Number(ev.user_id) !== Number(yo.id)) {
      throw new ApiErr('Solo quien lo creó o quien administra puede borrarlo', 403, 'sin_permiso');
    }
    await c.pool.query('DELETE FROM group_events WHERE id = $1', [eventoId]);
    await anotar(c.pool, id, yo.id, 'evento_borrado', ev.title);
    return { ok: true, borrado: eventoId };
  });

  // ============================================================
  // Archivos
  // ============================================================
  router.get('/api/groups/:id/files', async (c) => {
    const id = Number(c.params.id);
    const { yo, esMiembro, grupo } = await miPapel(c, id);
    if (!esMiembro && grupo.privacy === 'private') {
      throw new ApiErr('Este grupo es privado: entra para ver sus archivos', 403, 'privado');
    }
    const archivos = await filas(
      c.pool,
      `${SQL_ARCHIVOS} WHERE f.group_id = $1 ORDER BY f.created_at DESC LIMIT 80`,
      [id, yo.id]
    );
    return { archivos: archivos.map((f) => aArchivo(f, yo.id)) };
  });

  router.post('/api/groups/:id/files', async (c) => {
    const id = Number(c.params.id);
    const { yo, esMiembro } = await miPapel(c, id);
    if (!esMiembro) throw new ApiErr('Entra al grupo para compartir archivos', 403, 'no_miembro');
    const b = await c.cuerpo();
    const nombre = texto(b.nombre || '', { min: 1, max: 160, campo: 'nombre' }).trim();
    const url = String(b.url || '');
    if (!/^\/api\/media\/[A-Za-z0-9._-]+$/.test(url)) {
      throw new ApiErr('Sube el archivo primero y luego compártelo', 400, 'sin_archivo');
    }
    const tipo = typeof b.tipo === 'string' ? b.tipo.slice(0, 80) : '';
    const peso = Math.min(Math.max(Number(b.peso || 0), 0), 200 * 1024 * 1024);
    const creado = await uno(
      c.pool,
      `INSERT INTO group_files (group_id, user_id, nombre, url, tipo, peso)
       VALUES ($1, $2, $3, $4, $5, $6) RETURNING id`,
      [id, yo.id, nombre, url, tipo, peso]
    );
    const f = await uno(c.pool, `${SQL_ARCHIVOS} WHERE f.id = $1`, [creado.id, yo.id]);
    await anotar(c.pool, id, yo.id, 'archivo_compartido', nombre);
    return aArchivo(f, yo.id);
  });

  router.del('/api/groups/:id/files/:archivo', async (c) => {
    const id = Number(c.params.id);
    const archivoId = Number(c.params.archivo);
    const { yo, mando } = await miPapel(c, id);
    const f = await uno(c.pool, 'SELECT id, user_id, nombre FROM group_files WHERE id = $1 AND group_id = $2', [archivoId, id]);
    if (!f) throw new ApiErr('Ese archivo no está en el grupo', 404, 'archivo_no_existe');
    if (!mando && Number(f.user_id) !== Number(yo.id)) {
      throw new ApiErr('Solo quien lo compartió o quien administra puede quitarlo', 403, 'sin_permiso');
    }
    await c.pool.query('DELETE FROM group_files WHERE id = $1', [archivoId]);
    await anotar(c.pool, id, yo.id, 'archivo_borrado', f.nombre);
    return { ok: true, borrado: archivoId };
  });

  // ============================================================
  // Reglas y anuncio fijado
  // ============================================================
  router.get('/api/groups/:id/rules', async (c) => {
    const id = Number(c.params.id);
    const { esMiembro, grupo } = await miPapel(c, id);
    if (!esMiembro && grupo.privacy === 'private') {
      throw new ApiErr('Este grupo es privado: entra para ver sus reglas', 403, 'privado');
    }
    return {
      rules: grupo.rules || '',
      announcement: esMiembro || grupo.privacy === 'public' ? grupo.announcement || '' : '',
      announcement_at: grupo.announcement_at ? String(grupo.announcement_at) : null,
      join_questions: Array.isArray(grupo.join_questions) ? grupo.join_questions : [],
    };
  });

  router.patch('/api/groups/:id/rules', async (c) => {
    const id = Number(c.params.id);
    const { yo, mando } = await miPapel(c, id);
    if (!mando) throw new ApiErr('Solo quien administra el grupo puede cambiar las reglas', 403, 'sin_mando');
    const b = await c.cuerpo();
    const reglas = typeof b.rules === 'string' ? b.rules.slice(0, 2000).trim() : null;
    const anuncio = typeof b.announcement === 'string' ? b.announcement.slice(0, 600).trim() : null;
    const preguntas = Array.isArray(b.join_questions)
      ? b.join_questions.map((q) => String(q).slice(0, 160).trim()).filter(Boolean).slice(0, 5)
      : null;
    const actualizado = await uno(
      c.pool,
      `UPDATE groups SET
          rules = COALESCE($2, rules),
          announcement = COALESCE($3, announcement),
          announcement_at = CASE WHEN $3::text IS NULL THEN announcement_at
                                 WHEN $3 = '' THEN NULL ELSE NOW() END,
          join_questions = COALESCE($4::jsonb, join_questions)
        WHERE id = $1
        RETURNING rules, announcement, announcement_at::text AS announcement_at, join_questions`,
      [id, reglas, anuncio, preguntas ? JSON.stringify(preguntas) : null]
    );
    await anotar(c.pool, id, yo.id, anuncio !== null ? 'anuncio_cambiado' : 'reglas_cambiadas', anuncio || reglas || '');
    return {
      rules: actualizado.rules || '',
      announcement: actualizado.announcement || '',
      announcement_at: actualizado.announcement_at || null,
      join_questions: Array.isArray(actualizado.join_questions) ? actualizado.join_questions : [],
    };
  });

  // ============================================================
  // Preguntas para entrar: solicitudes
  // ============================================================
  // Lo que ve quien quiere entrar: si hay preguntas, se le piden.
  router.get('/api/groups/:id/preguntas', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const g = await uno(c.pool, 'SELECT id, name, privacy, join_questions FROM groups WHERE id = $1', [id]);
    if (!g) throw new ApiErr('Grupo no encontrado', 404);
    const mia = await uno(
      c.pool,
      `SELECT id, estado, created_at::text AS created_at FROM group_join_requests
        WHERE group_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT 1`,
      [id, yo.id]
    );
    return {
      pregunta: Array.isArray(g.join_questions) && g.join_questions.length > 0,
      preguntas: Array.isArray(g.join_questions) ? g.join_questions : [],
      mi_solicitud: mia ? { id: Number(mia.id), estado: mia.estado, created_at: mia.created_at } : null,
    };
  });

  // Pedir entrar contestando las preguntas.
  router.post('/api/groups/:id/solicitar', async (c) => {
    const yo = await c.exigir();
    const id = Number(c.params.id);
    const g = await uno(c.pool, 'SELECT id, name, owner_id, join_questions FROM groups WHERE id = $1', [id]);
    if (!g) throw new ApiErr('Grupo no encontrado', 404);

    const preguntas = Array.isArray(g.join_questions) ? g.join_questions : [];
    const b = await c.cuerpo();
    const respuestas = Array.isArray(b.answers) ? b.answers.map((a) => String(a).slice(0, 400).trim()) : [];
    if (preguntas.length > 0 && respuestas.length < preguntas.length) {
      throw new ApiErr('Contesta todas las preguntas para pedir entrar', 400, 'faltan_respuestas');
    }

    // Sin preguntas, entrar es directo: no tiene sentido hacer esperar a nadie.
    if (preguntas.length === 0) {
      const r = await c.pool.query(
        `INSERT INTO group_members (group_id, user_id, role) VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING`,
        [id, yo.id]
      );
      if (r.rowCount > 0) {
        await c.pool.query('UPDATE groups SET members_count = members_count + 1 WHERE id = $1', [id]);
        await anotar(c.pool, id, yo.id, 'miembro_entrado', yo.username);
        if (Number(g.owner_id) !== Number(yo.id)) {
          await notificar(c.pool, {
            userId: Number(g.owner_id), tipo: 'group_join', deUserId: Number(yo.id),
            contenido: `se unió a ${g.name}`,
          }).catch(() => {});
        }
      }
      return { ok: true, estado: 'dentro' };
    }

    const ya = await uno(
      c.pool,
      `SELECT id FROM group_join_requests WHERE group_id = $1 AND user_id = $2 AND estado = 'pendiente'`,
      [id, yo.id]
    );
    if (ya) return { ok: true, estado: 'pendiente', solicitud_id: Number(ya.id), repetida: true };

    const s = await uno(
      c.pool,
      `INSERT INTO group_join_requests (group_id, user_id, answers) VALUES ($1, $2, $3::jsonb) RETURNING id`,
      [id, yo.id, JSON.stringify(respuestas.slice(0, preguntas.length))]
    );
    await anotar(c.pool, id, yo.id, 'solicitud_enviada', respuestas[0] || '');
    if (Number(g.owner_id) !== Number(yo.id)) {
      await notificar(c.pool, {
        userId: Number(g.owner_id), tipo: 'group_join', deUserId: Number(yo.id),
        contenido: `quiere entrar a ${g.name}`,
      }).catch(() => {});
    }
    return { ok: true, estado: 'pendiente', solicitud_id: Number(s.id) };
  });

  // Quien administra ve las solicitudes pendientes con sus respuestas.
  router.get('/api/groups/:id/solicitudes', async (c) => {
    const id = Number(c.params.id);
    const { mando } = await miPapel(c, id);
    if (!mando) throw new ApiErr('Solo quien administra el grupo ve las solicitudes', 403, 'sin_mando');
    const lista = await filas(
      c.pool,
      `SELECT s.id, s.group_id, s.user_id, s.answers, s.estado, s.created_at::text AS created_at,
              u.username, u.display_name, u.avatar_url
         FROM group_join_requests s JOIN users u ON u.id = s.user_id
        WHERE s.group_id = $1 AND s.estado = 'pendiente'
        ORDER BY s.created_at ASC LIMIT 60`,
      [id]
    );
    return { solicitudes: lista.map(aSolicitud) };
  });

  router.post('/api/groups/:id/solicitudes/:solicitud', async (c) => {
    const id = Number(c.params.id);
    const solicitudId = Number(c.params.solicitud);
    const { yo, mando, grupo } = await miPapel(c, id);
    if (!mando) throw new ApiErr('Solo quien administra el grupo decide quién entra', 403, 'sin_mando');
    const b = await c.cuerpo().catch(() => ({}));
    const aprobar = b?.aprobar !== false;
    const s = await uno(
      c.pool,
      'SELECT id, user_id, estado FROM group_join_requests WHERE id = $1 AND group_id = $2',
      [solicitudId, id]
    );
    if (!s) throw new ApiErr('Esa solicitud no existe', 404, 'solicitud_no_existe');
    if (s.estado !== 'pendiente') throw new ApiErr('Esa solicitud ya se resolvió', 409, 'ya_resuelta');

    await c.pool.query(
      `UPDATE group_join_requests SET estado = $2, resolved_by = $3, resolved_at = NOW() WHERE id = $1`,
      [solicitudId, aprobar ? 'aprobada' : 'rechazada', yo.id]
    );

    if (aprobar) {
      const r = await c.pool.query(
        `INSERT INTO group_members (group_id, user_id, role) VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING`,
        [id, s.user_id]
      );
      if (r.rowCount > 0) {
        await c.pool.query('UPDATE groups SET members_count = members_count + 1 WHERE id = $1', [id]);
      }
      await anotar(c.pool, id, yo.id, 'solicitud_aprobada', `#${s.user_id}`);
    } else {
      await anotar(c.pool, id, yo.id, 'solicitud_rechazada', `#${s.user_id}`);
    }

    await notificar(c.pool, {
      userId: Number(s.user_id), tipo: 'group_join', deUserId: Number(yo.id),
      contenido: aprobar ? `Te aceptaron en ${grupo.name}` : `Por ahora no entraste a ${grupo.name}`,
    }).catch(() => {});
    enviarA(Number(s.user_id), {
      type: 'group_request',
      group_id: id,
      aprobada: aprobar,
      name: grupo.name,
    });
    return { ok: true, aprobada: aprobar };
  });

  // ============================================================
  // Sanciones y registro
  // ============================================================
  router.get('/api/groups/:id/sanciones', async (c) => {
    const id = Number(c.params.id);
    const { yo, mando } = await miPapel(c, id);
    // Cada quien ve lo suyo; quien administra, todo.
    const lista = await filas(
      c.pool,
      `SELECT s.id, s.user_id, s.tipo, s.motivo, s.hasta::text AS hasta, s.created_at::text AS created_at,
              u.username, u.display_name, u.avatar_url, p.username AS por_username
         FROM group_sanctions s
         JOIN users u ON u.id = s.user_id
         LEFT JOIN users p ON p.id = s.por_id
        WHERE s.group_id = $1 ${mando ? '' : 'AND s.user_id = $2'}
        ORDER BY s.created_at DESC LIMIT 60`,
      mando ? [id] : [id, yo.id]
    );
    return {
      sanciones: lista.map((s) => ({
        id: Number(s.id),
        user_id: Number(s.user_id),
        username: s.username,
        display_name: s.display_name || s.username,
        avatar_url: s.avatar_url || '',
        tipo: s.tipo,
        motivo: s.motivo || '',
        hasta: s.hasta,
        created_at: s.created_at,
        por: s.por_username || '',
        vigente: s.tipo === 'silencio' ? (!s.hasta || new Date(s.hasta).getTime() > Date.now()) : true,
      })),
      mando,
    };
  });

  router.post('/api/groups/:id/sanciones', async (c) => {
    const id = Number(c.params.id);
    const { yo, mando, grupo } = await miPapel(c, id);
    if (!mando) throw new ApiErr('Solo quien administra el grupo puede sancionar', 403, 'sin_mando');
    const b = await c.cuerpo();
    const quien = Number(b.user_id);
    const tipo = ['aviso', 'silencio', 'expulsion'].includes(b.tipo) ? b.tipo : 'aviso';
    const motivo = typeof b.motivo === 'string' ? b.motivo.slice(0, 300).trim() : '';
    if (!Number.isFinite(quien) || quien <= 0) throw new ApiErr('Dime a quién', 400, 'sin_persona');
    if (quien === Number(yo.id)) throw new ApiErr('No te sanciones a ti mismo', 409, 'soy_yo');
    if (quien === Number(grupo.owner_id)) throw new ApiErr('A quien creó el grupo no se le sanciona', 403, 'es_owner');

    const miembro = await uno(c.pool, 'SELECT user_id FROM group_members WHERE group_id = $1 AND user_id = $2', [id, quien]);
    if (!miembro) throw new ApiErr('Esa persona no está en el grupo', 404, 'no_esta');
    const minutos = Math.min(Math.max(Number(b.minutos || 0), 0), 60 * 24 * 30);
    const hasta = tipo === 'silencio' && minutos > 0 ? new Date(Date.now() + minutos * 60_000).toISOString() : null;

    const creada = await uno(
      c.pool,
      `INSERT INTO group_sanctions (group_id, user_id, por_id, tipo, motivo, hasta)
       VALUES ($1, $2, $3, $4, $5, $6) RETURNING id`,
      [id, quien, yo.id, tipo, motivo, hasta]
    );

    if (tipo === 'expulsion') {
      await c.pool.query('DELETE FROM group_members WHERE group_id = $1 AND user_id = $2', [id, quien]);
      await c.pool.query('UPDATE groups SET members_count = GREATEST(members_count - 1, 0) WHERE id = $1', [id]);
      enviarA(quien, { type: 'group_left', group_id: id });
    }
    await anotar(c.pool, id, yo.id, `sancion_${tipo}`, `#${quien} ${motivo}`);
    await notificar(c.pool, {
      userId: quien, tipo: 'system', deUserId: Number(yo.id),
      contenido: tipo === 'aviso' ? `Aviso en ${grupo.name}: ${motivo || 'revisa las reglas'}`
        : tipo === 'silencio' ? `Estás en silencio en ${grupo.name} por un rato`
          : `Te sacaron de ${grupo.name}`,
    }).catch(() => {});
    return { ok: true, id: Number(creada.id), tipo, hasta };
  });

  // Levantar una sanción (perdón).
  router.del('/api/groups/:id/sanciones/:sancion', async (c) => {
    const id = Number(c.params.id);
    const sancionId = Number(c.params.sancion);
    const { yo, mando } = await miPapel(c, id);
    if (!mando) throw new ApiErr('Solo quien administra el grupo puede levantar sanciones', 403, 'sin_mando');
    const s = await uno(c.pool, 'SELECT id, user_id, tipo FROM group_sanctions WHERE id = $1 AND group_id = $2', [sancionId, id]);
    if (!s) throw new ApiErr('Esa sanción no existe', 404);
    await c.pool.query('DELETE FROM group_sanctions WHERE id = $1', [sancionId]);
    await anotar(c.pool, id, yo.id, 'sancion_levantada', `#${s.user_id} ${s.tipo}`);
    return { ok: true, levantada: sancionId };
  });

  // El registro: qué ha pasado en el grupo, quién hizo qué y cuándo.
  router.get('/api/groups/:id/registro', async (c) => {
    const id = Number(c.params.id);
    const { mando } = await miPapel(c, id);
    if (!mando) throw new ApiErr('El registro es para quien administra el grupo', 403, 'sin_mando');
    const lista = await filas(
      c.pool,
      `SELECT l.id, l.accion, l.detalle, l.created_at::text AS created_at,
              u.username, u.display_name, u.avatar_url
         FROM group_log l LEFT JOIN users u ON u.id = l.user_id
        WHERE l.group_id = $1
        ORDER BY l.created_at DESC LIMIT 80`,
      [id]
    );
    return {
      registro: lista.map((l) => ({
        id: Number(l.id),
        accion: l.accion,
        detalle: l.detalle || '',
        created_at: l.created_at,
        username: l.username || '',
        display_name: l.display_name || l.username || 'Alguien',
        avatar_url: l.avatar_url || '',
      })),
    };
  });
}
