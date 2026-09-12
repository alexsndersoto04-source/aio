// Moon — Administración y moderación
// ============================================================
// Resumen, serie diaria, gestión de personas (suspender, activar, verificar),
// reportes, palabras bloqueadas y registro de actividad.

import { ApiErr, texto, paginacion, qs, booleano } from './util.mjs';
import { uno } from './db.mjs';
import { auditar } from './db.mjs';
import { enviarA } from './ws.mjs';

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
}
