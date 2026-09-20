// Moon — Avisos al teléfono (Web Push)
// ============================================================
// Permiten que llegue un aviso al teléfono aunque Moon esté cerrado o en
// segundo plano. Las llaves del servidor (VAPID) se generan solas la primera
// vez y se guardan en la tabla `app_settings`: no hay que configurar nada a
// mano ni pagar por un servicio de mensajería.

import webpush from 'web-push';
import { uno } from './db.mjs';

let listo = false;
let clavePublica = '';
let errorDeLlaves = '';

async function cargarLlaves(pool) {
  if (listo) return true;
  try {
    const guardadas = await uno(pool, `SELECT valor FROM app_settings WHERE clave = 'vapid'`);
    let llaves = null;
    if (guardadas?.valor) {
      try { llaves = JSON.parse(guardadas.valor); } catch { llaves = null; }
    }
    if (!llaves?.publicKey || !llaves?.privateKey) {
      llaves = webpush.generateVAPIDKeys();
      await pool.query(
        `INSERT INTO app_settings (clave, valor) VALUES ('vapid', $1)
         ON CONFLICT (clave) DO UPDATE SET valor = $1, updated_at = NOW()`,
        [JSON.stringify(llaves)]
      );
      console.log('[push] llaves de avisos creadas y guardadas');
    }
    const contacto = process.env.PUSH_CONTACT || 'mailto:admin@moon.app';
    webpush.setVapidDetails(contacto, llaves.publicKey, llaves.privateKey);
    clavePublica = llaves.publicKey;
    listo = true;
    return true;
  } catch (e) {
    errorDeLlaves = e.message;
    console.error('[push] no se pudieron preparar las llaves:', e.message);
    return false;
  }
}

export async function claveDeAvisos(pool) {
  if (!(await cargarLlaves(pool))) return '';
  return clavePublica;
}

/**
 * Manda un aviso a todos los dispositivos de una persona.
 * Si un dispositivo ya no existe (404/410), se borra de la lista.
 */
export async function enviarEmpuje(pool, userId, aviso) {
  if (!(await cargarLlaves(pool))) return { enviados: 0 };
  const subs = await pool.query(
    'SELECT id, endpoint, p256dh, auth FROM push_subscriptions WHERE user_id = $1',
    [Number(userId)]
  );
  let enviados = 0;
  let dispositivos = 0;
  for (const s of subs.rows) {
    const suscripcion = { endpoint: s.endpoint, keys: { p256dh: s.p256dh, auth: s.auth } };
    const cuerpo = JSON.stringify({
      title: aviso.titulo || 'Moon',
      body: aviso.texto || '',
      url: aviso.url || '#/feed',
      icon: '/iconos/icono-192.png',
      badge: '/iconos/icono-192.png',
      etiqueta: aviso.etiqueta || aviso.url || 'moon',
      // Los avisos de llamada se quedan en pantalla y vibran más fuerte,
      // como el timbre de una llamada de verdad.
      vibrar: aviso.vibrar || null,
      quedarse: aviso.quedarse === true,
    });
    try {
      await webpush.sendNotification(suscripcion, cuerpo, {
        TTL: aviso.quedarse ? 60 : 12 * 3600,
        urgency: aviso.urgente ? 'high' : 'normal',
      });
      enviados += 1;
      dispositivos += 1;
      pool.query('UPDATE push_subscriptions SET last_ok_at = NOW(), last_error = \'\' WHERE id = $1', [s.id]).catch(() => {});
    } catch (e) {
      const codigo = e?.statusCode || 0;
      if (codigo === 404 || codigo === 410) {
        // Ese navegador ya no existe: se borra de la lista y no cuenta.
        pool.query('DELETE FROM push_subscriptions WHERE id = $1', [s.id]).catch(() => {});
      } else {
        // Problema pasajero: el teléfono sigue apuntado.
        dispositivos += 1;
        pool.query('UPDATE push_subscriptions SET last_error = $2 WHERE id = $1', [s.id, String(e.message || e).slice(0, 200)]).catch(() => {});
      }
    }
  }
  return { enviados, dispositivos };
}

/**
 * Avisa a alguien si tiene el aviso encendido, sin romper nada si falla.
 * Devuelve cuántos avisos salieron y cuántos teléfonos hay apuntados.
 */
export async function empujarSiQuiere(pool, userId, clave, aviso) {
  try {
    if (clave) {
      const prefs = await uno(pool, 'SELECT * FROM notification_prefs WHERE user_id = $1', [userId]);
      if (prefs && prefs[clave] === false) return { enviados: 0, dispositivos: 0 };
    }
    return await enviarEmpuje(pool, userId, aviso);
  } catch (e) {
    console.error('[push] fallo al avisar:', e.message);
    return { enviados: 0, dispositivos: 0 };
  }
}
