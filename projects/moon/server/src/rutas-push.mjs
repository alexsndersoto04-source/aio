// Moon — Avisos al teléfono: rutas
// ============================================================
//   GET    /api/push/public-key   llave pública (la necesita el navegador)
//   POST   /api/push/subscribe    guarda este dispositivo
//   DELETE /api/push/subscribe    quita este dispositivo
//   POST   /api/push/test         manda un aviso de prueba a mis dispositivos
//   GET    /api/push/estado       cuántos dispositivos tengo conectados

import { ApiErr } from './util.mjs';
import { uno } from './db.mjs';
import { claveDeAvisos, enviarEmpuje } from './empuje.mjs';

export function registrarRutasPush(router) {
  router.get('/api/push/public-key', async (c) => {
    await c.exigir();
    const key = await claveDeAvisos(c.pool);
    if (!key) throw new ApiErr('Los avisos al teléfono no están disponibles ahora mismo', 503);
    return { key };
  });

  router.post('/api/push/subscribe', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const endpoint = String(b?.endpoint || '').slice(0, 600);
    const p256dh = String(b?.keys?.p256dh || '').slice(0, 400);
    const auth = String(b?.keys?.auth || '').slice(0, 400);
    if (!endpoint || !p256dh || !auth) throw new ApiErr('La suscripción llegó incompleta', 400);
    const device = String(b?.device || c.req.headers['user-agent'] || 'Web').slice(0, 200);

    await c.pool.query(
      `INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, device)
       VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT (endpoint) DO UPDATE SET user_id = $1, p256dh = $3, auth = $4, device = $5, last_error = ''`,
      [yo.id, endpoint, p256dh, auth, device]
    );
    const total = await uno(c.pool, 'SELECT COUNT(*)::int AS n FROM push_subscriptions WHERE user_id = $1', [yo.id]);
    return { ok: true, dispositivos: Number(total?.n || 0) };
  });

  router.delete('/api/push/subscribe', async (c) => {
    const yo = await c.exigir();
    const b = await c.cuerpo();
    const endpoint = String(b?.endpoint || '').slice(0, 600);
    if (endpoint) {
      await c.pool.query('DELETE FROM push_subscriptions WHERE endpoint = $1 AND user_id = $2', [endpoint, yo.id]);
    } else {
      await c.pool.query('DELETE FROM push_subscriptions WHERE user_id = $1', [yo.id]);
    }
    return { ok: true };
  });

  router.get('/api/push/estado', async (c) => {
    const yo = await c.exigir();
    const r = await uno(
      c.pool,
      `SELECT COUNT(*)::int AS n, MAX(GREATEST(COALESCE(last_ok_at, created_at), created_at))::text AS ultimo
         FROM push_subscriptions WHERE user_id = $1`,
      [yo.id]
    );
    return { dispositivos: Number(r?.n || 0), ultimo: r?.ultimo || null };
  });

  router.post('/api/push/test', async (c) => {
    const yo = await c.exigir();
    const r = await enviarEmpuje(c.pool, yo.id, {
      titulo: 'Moon',
      texto: 'Avisos al teléfono activados. Así te llegarán los mensajes.',
      url: '#/notifications',
      etiqueta: 'prueba',
    });
    if (r.enviados === 0) throw new ApiErr('No hay ningún dispositivo conectado todavía', 400);
    return { ok: true, enviados: r.enviados };
  });
}
