// Moon — Administración y moderación
// ============================================================
// Resumen, serie diaria, gestión de personas (suspender, activar, verificar),
// reportes, palabras bloqueadas y registro de actividad.

import { ApiErr, texto, paginacion, qs, booleano } from './util.mjs';
import { uno } from './db.mjs';
import { auditar } from './db.mjs';
import { volcarBase, copiaPorCorreo } from './copias.mjs';
import { migrarA } from './migracion.mjs';
import { enviarA } from './ws.mjs';
import { demasiadoRapido } from './limites.mjs';
import {
  obtenerEstadoSeguridad, cambiarModoBlindaje, bloquearIp, desbloquearIp,
  listarIpsBloqueadas, listarEventosSeguridad,
} from './defensas.mjs';
import { microCache } from './cache-memoria.mjs';

export function registrarRutasAdmin(router) {
  router.get('/api/admin/dashboard', async (c) => {
    await c.admin();
    const r = await uno(
      c.pool,
      `SELECT
        (SELECT COUNT(*)::int FROM users WHERE status = 'active') AS users_total,
        (SELECT COUNT(*)::int FROM users WHERE status = 'active' AND created_at::date = CURRENT_DATE) AS users_new_today,
        (SELECT COUNT(*)::int FROM users WHERE status = 'active' AND created_at > NOW() - INTERVAL '7 days') AS users_new_7d,
        (SELECT COUNT(*)::int FROM users WHERE status = 'suspended') AS suspended_users,
        (SELECT COUNT(*)::int FROM posts WHERE status = 'active') AS posts_total,
        (SELECT COUNT(*)::int FROM posts WHERE status = 'active' AND created_at::date = CURRENT_DATE) AS posts_today,
        (SELECT COUNT(*)::int FROM comments WHERE status = 'active') AS comments_total,
        (SELECT COUNT(*)::int FROM follows) AS follows_total,
        (SELECT COUNT(*)::int FROM messages WHERE status <> 'deleted') AS messages_total,
        (SELECT COUNT(*)::int FROM reports WHERE status = 'open') AS reports_open`
    );
    return r;
  });

  router.get('/api/admin/stats', async (c) => {
    await c.admin();
    const filas = await c.pool.query(
      `SELECT stat_date::text AS stat_date, new_users, new_posts, new_messages, new_likes, new_comments, new_follows
         FROM app_stats WHERE stat_date >= CURRENT_DATE - INTERVAL '30 days' ORDER BY stat_date ASC`
    );
    return filas.rows;
  });

  router.get('/api/admin/users', async (c) => {
    await c.admin();
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const consulta = texto(qs(c.req, 'q', ''), { min: 0, max: 80, campo: 'búsqueda' });
    const filtro = consulta ? `%${consulta}%` : null;
    const filas = await c.pool.query(
      `SELECT id, username, display_name, email, role, status, is_verified, suspend_reason,
              avatar_url, posts_count, followers_count, created_at::text AS created_at,
              last_login_at::text AS last_login_at
         FROM users
        WHERE ($1::text IS NULL OR username ILIKE $1 OR display_name ILIKE $1 OR email ILIKE $1)
        ORDER BY created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      [filtro]
    );
    const total = await uno(
      c.pool,
      `SELECT COUNT(*)::int AS count FROM users
        WHERE ($1::text IS NULL OR username ILIKE $1 OR display_name ILIKE $1 OR email ILIKE $1)`,
      [filtro]
    );
    return {
      items: filas.rows.map((u) => ({
        id: Number(u.id),
        username: u.username,
        display_name: u.display_name || u.username,
        email: u.email,
        role: u.role,
        status: u.status,
        is_suspended: u.status === 'suspended',
        suspend_reason: u.suspend_reason,
        is_verified: !!u.is_verified,
        avatar_url: u.avatar_url,
        posts_count: Number(u.posts_count),
        followers_count: Number(u.followers_count),
        created_at: u.created_at,
        last_login_at: u.last_login_at,
      })),
      total: Number(total?.count || 0),
      page,
      limit,
    };
  });

  router.post('/api/admin/users/:id/suspend', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const motivo = texto(b.reason || 'Incumplimiento de las normas', { min: 2, max: 200, campo: 'motivo' });
    const objetivo = Number(c.params.id);
    if (objetivo === Number(admin.id)) throw new ApiErr('No puedes suspender tu propia cuenta', 400);
    const r = await c.pool.query(
      `UPDATE users SET status = 'suspended', suspend_reason = $1 WHERE id = $2 RETURNING id`,
      [motivo, objetivo]
    );
    if (r.rowCount === 0) throw new ApiErr('Usuario no encontrado', 404);
    // Se cierran sus sesiones y se le avisa en vivo.
    await c.pool.query('UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL', [objetivo]);
    enviarA(objetivo, { type: 'account_suspended', reason: motivo });
    await auditar(c.pool, Number(admin.id), 'usuario_suspendido', `#${objetivo}: ${motivo}`, c.ip);
    return { ok: true };
  });

  router.post('/api/admin/users/:id/activate', async (c) => {
    const admin = await c.admin();
    const objetivo = Number(c.params.id);
    const r = await c.pool.query(
      `UPDATE users SET status = 'active', suspend_reason = '' WHERE id = $1 RETURNING id`,
      [objetivo]
    );
    if (r.rowCount === 0) throw new ApiErr('Usuario no encontrado', 404);
    enviarA(objetivo, { type: 'account_activated' });
    await auditar(c.pool, Number(admin.id), 'usuario_activado', `#${objetivo}`, c.ip);
    return { ok: true };
  });

  router.post('/api/admin/users/:id/verify', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const verificado = booleano(b.verified, true);
    const r = await c.pool.query('UPDATE users SET is_verified = $1 WHERE id = $2 RETURNING username', [
      verificado, Number(c.params.id),
    ]);
    if (r.rowCount === 0) throw new ApiErr('Usuario no encontrado', 404);
    await auditar(c.pool, Number(admin.id), verificado ? 'usuario_verificado' : 'verificacion_retirada', `#${c.params.id}`, c.ip);
    return { ok: true, is_verified: verificado };
  });

  router.post('/api/admin/users/:id/role', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const rol = ['user', 'admin'].includes(b.role) ? b.role : 'user';
    if (Number(c.params.id) === Number(admin.id)) throw new ApiErr('No puedes cambiar tu propio rol', 400);
    await c.pool.query('UPDATE users SET role = $1 WHERE id = $2', [rol, Number(c.params.id)]);
    await auditar(c.pool, Number(admin.id), 'rol_cambiado', `#${c.params.id} → ${rol}`, c.ip);
    return { ok: true, role: rol };
  });

  // ---------- Copias de seguridad ----------
  // El plan gratuito de la base de datos no hace copias: aquí se pueden
  // descargar a mano (y una vez al día se envían solas por correo).
  router.get('/api/admin/backup', async (c) => {
    await c.admin();
    if (demasiadoRapido(`copia:${c.ip || 'x'}`, 3, 600_000)) {
      throw new ApiErr('Se pidieron varias copias seguidas: espera unos minutos', 429, 'rate_limit');
    }
    const copia = await volcarBase(c.pool);
    const cuerpo = Buffer.from(JSON.stringify(copia, null, 2), 'utf8');
    c.res.writeHead(200, {
      'Content-Type': 'application/json; charset=utf-8',
      'Content-Disposition': `attachment; filename="copia-moon-${copia.fecha.slice(0, 10)}.json"`,
      'Content-Length': cuerpo.length,
      'Cache-Control': 'no-store',
    });
    c.res.end(cuerpo);
    await auditar(c.pool, null, 'copia_descargada', `${Math.round(cuerpo.length / 1024)} KB`, c.ip);
    return undefined;
  });

  router.post('/api/admin/backup/correo', async (c) => {
    await c.admin();
    const r = await copiaPorCorreo(c.pool);
    if (!r.enviada) {
      throw new ApiErr(
        r.motivo === 'sin correo configurado'
          ? 'Todavía no hay correo configurado: pega la clave de Resend en Render (ver DESPLEGAR.md).'
          : r.motivo || 'No se pudo enviar la copia: revisa la clave de correo en Render.',
        400
      );
    }
    return { ok: true, enviadas: r.enviadas, resumen: r.resumen };
  });

  // ---------- Reportes ----------
  router.get('/api/admin/reports', async (c) => {
    await c.admin();
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const estado = ['open', 'resolved', 'dismissed', 'all'].includes(qs(c.req, 'status', 'open'))
      ? qs(c.req, 'status', 'open')
      : 'open';
    const filtro = estado === 'all' ? '' : 'AND r.status = $1';
    const args = estado === 'all' ? [] : [estado];
    const filas = await c.pool.query(
      `SELECT r.id, r.target_type, r.target_id, r.reason, r.detail, r.status, r.resolution,
              r.created_at::text AS created_at, r.resolved_at::text AS resolved_at,
              u.username AS reporter_username
         FROM reports r JOIN users u ON u.id = r.reporter_id
        WHERE TRUE ${filtro}
        ORDER BY r.created_at DESC LIMIT ${limit} OFFSET ${offset}`,
      args
    );
    const total = await uno(
      c.pool,
      `SELECT COUNT(*)::int AS count FROM reports r WHERE TRUE ${filtro}`,
      args
    );
    return {
      items: filas.rows.map((f) => ({
        id: Number(f.id),
        target_type: f.target_type,
        target_id: Number(f.target_id),
        reason: f.reason,
        detail: f.detail,
        status: f.status,
        resolution: f.resolution,
        reporter_username: f.reporter_username,
        created_at: f.created_at,
        resolved_at: f.resolved_at,
      })),
      total: Number(total?.count || 0),
      page,
      limit,
    };
  });

  router.post('/api/admin/reports/:id/resolve', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const accion = ['remove', 'dismiss', 'suspend'].includes(b.action) ? b.action : 'dismiss';
    const nota = typeof b.note === 'string' ? b.note.slice(0, 300) : '';
    const reporte = await uno(c.pool, 'SELECT * FROM reports WHERE id = $1', [Number(c.params.id)]);
    if (!reporte) throw new ApiErr('Reporte no encontrado', 404);

    if (accion === 'remove') {
      if (reporte.target_type === 'post') {
        await c.pool.query(`UPDATE posts SET status = 'deleted' WHERE id = $1`, [reporte.target_id]);
      } else if (reporte.target_type === 'comment') {
        await c.pool.query(`UPDATE comments SET status = 'deleted' WHERE id = $1`, [reporte.target_id]);
      }
    }
    if (accion === 'suspend' && reporte.target_type === 'post') {
      const post = await uno(c.pool, 'SELECT user_id FROM posts WHERE id = $1', [reporte.target_id]);
      if (post && Number(post.user_id) !== Number(admin.id)) {
        await c.pool.query(`UPDATE users SET status = 'suspended', suspend_reason = $1 WHERE id = $2`, [nota || 'Contenido reportado', post.user_id]);
        await c.pool.query('UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL', [post.user_id]);
      }
    }
    await c.pool.query(
      `UPDATE reports SET status = 'resolved', resolution = $1, resolved_by = $2, resolved_at = NOW() WHERE id = $3`,
      [`${accion}${nota ? `: ${nota}` : ''}`, admin.id, reporte.id]
    );
    await auditar(c.pool, Number(admin.id), 'reporte_resuelto', `#${reporte.id} → ${accion}`, c.ip);
    return { ok: true };
  });

  // ---------- Palabras bloqueadas ----------
  router.get('/api/admin/words', async (c) => {
    await c.admin();
    const filas = await c.pool.query(
      'SELECT id, word, created_at::text AS created_at FROM blocked_words ORDER BY id DESC'
    );
    return filas.rows.map((f) => ({ id: Number(f.id), word: f.word, created_at: f.created_at }));
  });

  router.post('/api/admin/words', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const palabra = texto(b.word, { min: 2, max: 60, campo: 'palabra' }).toLowerCase();
    await c.pool.query('INSERT INTO blocked_words (word) VALUES ($1) ON CONFLICT (word) DO NOTHING', [palabra]);
    await auditar(c.pool, Number(admin.id), 'palabra_bloqueada', palabra, c.ip);
    return { ok: true };
  });

  router.del('/api/admin/words/:id', async (c) => {
    const admin = await c.admin();
    await c.pool.query('DELETE FROM blocked_words WHERE id = $1', [Number(c.params.id)]);
    await auditar(c.pool, Number(admin.id), 'palabra_liberada', `#${c.params.id}`, c.ip);
    return { ok: true };
  });

  // ---------- Actividad ----------
  router.get('/api/admin/activity', async (c) => {
    await c.admin();
    const { page, limit, offset } = paginacion(c.req, 30, 100);
    const filas = await c.pool.query(
      `SELECT a.id, a.action, a.detail, a.ip, a.created_at::text AS created_at, u.username
         FROM activity_log a LEFT JOIN users u ON u.id = a.user_id
        ORDER BY a.created_at DESC LIMIT ${limit} OFFSET ${offset}`
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS count FROM activity_log');
    return {
      items: filas.rows.map((f) => ({
        id: Number(f.id),
        action: f.action,
        detail: f.detail,
        ip: f.ip,
        username: f.username,
        created_at: f.created_at,
      })),
      total: Number(total?.count || 0),
      page,
      limit,
    };
  });

  // ---------- Migración a otra base de datos (una sola vez) ----------
  // El servidor (que SÍ puede hablar con ambas bases) copia todo desde su
  // base actual a la base nueva indicada en MOON_MIGRATE_DEST. Se usa una
  // vez, para mudarse a Neon sin perder nada.
  // El destino se pega en el panel (mudanza a Neon, una sola vez) o viene de
  // la variable MOON_MIGRATE_DEST. Se valida que parezca una dirección real.
  function direccionDestino(cuerpo) {
    const d = String(cuerpo?.destination || process.env.MOON_MIGRATE_DEST || '').trim();
    if (!d) {
      throw new ApiErr(
        'Indica la dirección de la base nueva (pega la de Neon) o define MOON_MIGRATE_DEST.',
        400,
        'sin_destino'
      );
    }
    if (!/^postgres(ql)?:\/\//i.test(d) || d.length < 25 || d.length > 600) {
      throw new ApiErr(
        'La dirección pegada no parece válida: debe empezar por postgres:// (cópiala completa, como se muestra en Neon).',
        400,
        'direccion_invalida'
      );
    }
    return d;
  }

  router.post('/api/admin/migrate', async (c) => {
    const admin = await c.admin();
    if (demasiadoRapido('migrar', 3, 600_000)) {
      throw new ApiErr('Demasiados intentos seguidos: espera unos minutos', 429, 'rate_limit');
    }
    const destino = direccionDestino(await c.cuerpo().catch(() => ({})));
    try {
      const informe = await migrarA(c.pool, destino);
      await auditar(c.pool, admin.id, 'base_migrada', `${informe.total_filas} filas a la base nueva`, c.ip);
      console.log(`[migracion] ${informe.total_filas} filas copiadas a la base nueva (verificado)`);
      return { ok: true, ...informe };
    } catch (e) {
      throw new ApiErr(e.message || 'No se pudo migrar', 400, 'migracion');
    }
  });

  // «Migrar y pasar a la base nueva»: copia todo (si falta), guarda la nueva
  // dirección en los ajustes y reinicia el servicio para que la use desde ya.
  router.post('/api/admin/migrate/activar', async (c) => {
    const admin = await c.admin();
    if (demasiadoRapido('migrar', 3, 600_000)) {
      throw new ApiErr('Demasiados intentos seguidos: espera unos minutos', 429, 'rate_limit');
    }
    const destino = direccionDestino(await c.cuerpo().catch(() => ({})));
    let informe;
    try {
      informe = await migrarA(c.pool, destino);
    } catch (e) {
      // Si ya se copió antes (la base nueva ya tiene datos), se activa igual.
      if (!/ya tiene datos/.test(e.message)) throw e;
      console.log('[migracion] la base nueva ya estaba copiada: se activa sin volver a copiar');
      informe = { total_filas: null, ya_copiada: true };
    }
    try {
      await c.pool.query(
        `INSERT INTO app_settings (clave, valor) VALUES ('moon.db_override', $1)
         ON CONFLICT (clave) DO UPDATE SET valor = EXCLUDED.valor, updated_at = NOW()`,
        [destino]
      );
      await auditar(c.pool, admin.id, 'base_activada', 'la app ahora usa la base nueva', c.ip);
      console.log(`[migracion] base nueva activada: reiniciando el servicio`);
      // La respuesta llega al navegador antes de cerrar.
      setTimeout(() => {
        console.log('[api] cerrando para usar la base nueva desde ya');
        process.exit(0);
      }, 800);
      return { ok: true, reiniciando: true, ...informe };
    } catch (e) {
      throw new ApiErr(e.message || 'No se pudo activar la base nueva', 400, 'migracion');
    }
  });

  // ============================================================
  // ---------- CONTROL TOTAL: SEGURIDAD Y BLINDAJE -------------
  // ============================================================

  // Estado general de seguridad y defensas
  router.get('/api/admin/security', async (c) => {
    await c.admin();
    const info = await obtenerEstadoSeguridad(c.pool);
    const ips = await listarIpsBloqueadas(c.pool);
    return {
      ...info,
      ips_bloqueadas: ips,
      cache_ram: microCache.estadisticas(),
    };
  });

  // Activar o desactivar Modo Blindaje Anti-DDoS
  router.post('/api/admin/security/shield', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const activar = booleano(b.activar, true);
    return await cambiarModoBlindaje(c.pool, activar, admin.id);
  });

  // Bloquear IP
  router.post('/api/admin/security/block-ip', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const ip = texto(b.ip, { min: 3, max: 60, campo: 'IP' });
    const motivo = texto(b.motivo || 'Bloqueo manual', { min: 1, max: 200, campo: 'motivo' });
    return await bloquearIp(c.pool, ip, motivo, admin.id);
  });

  // Desbloquear IP
  router.post('/api/admin/security/unblock-ip', async (c) => {
    const admin = await c.admin();
    const b = await c.cuerpo();
    const ip = texto(b.ip, { min: 3, max: 60, campo: 'IP' });
    return await desbloquearIp(c.pool, ip, admin.id);
  });

  // Purgar memoria micro-caché bajo demanda
  router.post('/api/admin/cache/clear', async (c) => {
    await c.admin();
    microCache.limpiar();
    return { ok: true, mensaje: 'Micro-caché en memoria RAM purgado exitosamente' };
  });

  // ============================================================
  // ---------- CONTROL TOTAL: GESTIÓN DE CONTENIDOS ------------
  // ============================================================

  // Listado de todas las publicaciones para moderación directa
  router.get('/api/admin/content/posts', async (c) => {
    await c.admin();
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const r = await c.pool.query(
      `SELECT p.id, p.content, p.status, p.likes_count, p.comments_count,
              p.created_at::text AS created_at, u.username, u.display_name, u.avatar_url
       FROM posts p
       JOIN users u ON u.id = p.user_id
       ORDER BY p.id DESC
       LIMIT $1 OFFSET $2`,
      [limit, offset]
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS count FROM posts');
    return { items: r.rows, total: Number(total?.count || 0), page, limit };
  });

  // Listado de todos los videos de Moon Watch para moderación directa
  router.get('/api/admin/content/videos', async (c) => {
    await c.admin();
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const r = await c.pool.query(
      `SELECT p.id, p.content, p.status, p.likes_count, p.comments_count,
              p.created_at::text AS created_at, u.username, u.display_name,
              pi.original_url AS video_url
       FROM posts p
       JOIN users u ON u.id = p.user_id
       JOIN post_images pi ON pi.post_id = p.id
       WHERE pi.original_url ILIKE '%.mp4%' OR pi.original_url ILIKE '%.webm%'
          OR pi.original_url ILIKE '%.mov%' OR pi.original_url ILIKE '%video%'
       ORDER BY p.id DESC
       LIMIT $1 OFFSET $2`,
      [limit, offset]
    );
    return { items: r.rows, page, limit };
  });

  // Listado de grupos creados para moderación
  router.get('/api/admin/content/groups', async (c) => {
    await c.admin();
    const { page, limit, offset } = paginacion(c.req, 20, 50);
    const r = await c.pool.query(
      `SELECT g.id, g.name, g.slug, g.privacy, g.members_count, g.posts_count,
              g.created_at::text AS created_at, u.username AS owner_username
       FROM groups g
       JOIN users u ON u.id = g.owner_id
       ORDER BY g.id DESC
       LIMIT $1 OFFSET $2`,
      [limit, offset]
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS count FROM groups');
    return { items: r.rows, total: Number(total?.count || 0), page, limit };
  });

  // Eliminar grupo desde administración
  router.del('/api/admin/content/groups/:id', async (c) => {
    const admin = await c.admin();
    const id = Number(c.params.id);
    await c.pool.query('DELETE FROM groups WHERE id = $1', [id]);
    await auditar(c.pool, Number(admin.id), 'grupo_eliminado_admin', `#${id}`, c.ip);
    return { ok: true };
  });

  // Eliminar publicación o video desde administración
  router.del('/api/admin/content/posts/:id', async (c) => {
    const admin = await c.admin();
    const id = Number(c.params.id);
    const post = await uno(c.pool, 'SELECT user_id FROM posts WHERE id = $1', [id]);
    if (post) {
      await c.pool.query('DELETE FROM posts WHERE id = $1', [id]);
      await c.pool.query('UPDATE users SET posts_count = GREATEST(0, posts_count - 1) WHERE id = $1', [post.user_id]);
      await auditar(c.pool, Number(admin.id), 'post_eliminado_admin', `#${id}`, c.ip);
    }
    return { ok: true };
  });

  router.del('/api/admin/content/videos/:id', async (c) => {
    const admin = await c.admin();
    const id = Number(c.params.id);
    const post = await uno(c.pool, 'SELECT user_id FROM posts WHERE id = $1', [id]);
    if (post) {
      await c.pool.query('DELETE FROM posts WHERE id = $1', [id]);
      await c.pool.query('UPDATE users SET posts_count = GREATEST(0, posts_count - 1) WHERE id = $1', [post.user_id]);
      await auditar(c.pool, Number(admin.id), 'video_eliminado_admin', `#${id}`, c.ip);
    }
    return { ok: true };
  });

  // ============================================================
  // ---------- CONTROL TOTAL: SESIONES Y KILLSWITCH ------------
  // ============================================================

  // Ver sesiones abiertas de un usuario (para detectar accesos no autorizados)
  router.get('/api/admin/users/:id/sessions', async (c) => {
    await c.admin();
    const id = Number(c.params.id);
    const sesiones = await c.pool.query(
      `SELECT id, device, user_agent, ip, created_at::text AS created_at,
              last_used_at::text AS last_used_at, expires_at::text AS expires_at,
              revoked_at::text AS revoked_at
       FROM refresh_tokens
       WHERE user_id = $1
       ORDER BY id DESC
       LIMIT 20`,
      [id]
    );
    return sesiones.rows;
  });

  // Revocar una sesión específica de un usuario
  router.del('/api/admin/users/:userId/sessions/:tokenId', async (c) => {
    const admin = await c.admin();
    const userId = Number(c.params.userId);
    const tokenId = Number(c.params.tokenId);
    await c.pool.query(
      'UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1 AND user_id = $2',
      [tokenId, userId]
    );
    await auditar(c.pool, Number(admin.id), 'sesion_revocada_admin', `Usuario #${userId}, token #${tokenId}`, c.ip);
    return { ok: true };
  });

  // Killswitch: cerrar todas las sesiones activas de un usuario
  router.post('/api/admin/users/:id/revoke-sessions', async (c) => {
    const admin = await c.admin();
    const id = Number(c.params.id);
    const r = await c.pool.query(
      'UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL',
      [id]
    );
    enviarA(id, { type: 'force_logout', reason: 'Sesiones revocadas por administración' });
    await auditar(c.pool, Number(admin.id), 'sesiones_revocadas_admin', `Usuario #${id} (${r.rowCount} sesiones)`, c.ip);
    return { ok: true, sesiones_revocadas: r.rowCount };
  });
}
