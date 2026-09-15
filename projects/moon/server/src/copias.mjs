// Moon — Copias de seguridad
// ============================================================
// El plan gratuito de la base de datos no hace copias. Aquí se resuelve de
// dos maneras, sin pagar nada:
//
//   1. A mano: el panel de administración tiene un botón que descarga TODO
//      (usuarios, publicaciones, comentarios, mensajes, grupos…) en un
//      archivo JSON. Se puede guardar en el teléfono o en Drive.
//   2. Automática: una vez al día, si hay correo configurado, la copia se
//      envía por correo a las cuentas de administración. Así hay una copia
//      fuera del servidor sin que nadie tenga que acordarse.
//
// Las imágenes NO se incluyen en la copia (son muy pesadas); van aparte en
// su propio respaldo porque ya viven en la base de datos.

import { filas } from './db.mjs';
import { enviarCorreo, correoConfigurado } from './correo.mjs';

// Todas las tablas de Moon, en orden de lectura. Las de imágenes se resumen.
const TABLAS = [
  'users', 'follows', 'blocks', 'posts', 'post_images', 'likes', 'comments',
  'saves', 'hashtags', 'post_hashtags', 'conversations', 'messages',
  'notifications', 'notification_prefs', 'reports', 'blocked_words',
  'activity_log', 'app_stats', 'stories', 'story_views', 'groups',
  'group_members', 'polls', 'poll_votes',
];

const CAMPOS_PRIVADOS = ['password_hash', 'token_hash', 'code_hash'];

export async function volcarBase(pool) {
  const copia = {
    app: 'moon',
    fecha: new Date().toISOString(),
    version_esquema: null,
    tablas: {},
    resumen: {},
    imagenes: { total: 0, bytes: 0 },
  };

  try {
    const v = await filas(pool, 'SELECT MAX(version) AS v FROM schema_migrations');
    copia.version_esquema = Number(v[0]?.v || 0);
  } catch { /* sin tabla de migraciones */ }

  for (const tabla of TABLAS) {
    try {
      const datos = await filas(pool, `SELECT * FROM ${tabla}`);
      copia.tablas[tabla] = datos.map((fila) => {
        const limpia = { ...fila };
        for (const campo of CAMPOS_PRIVADOS) {
          if (campo in limpia) limpia[campo] = '[oculto en la copia]';
        }
        // Los campos de fecha se guardan como texto para que el archivo se
        // pueda leer en cualquier sitio.
        for (const [k, v] of Object.entries(limpia)) {
          if (v instanceof Date) limpia[k] = v.toISOString();
          else if (Buffer.isBuffer(v)) limpia[k] = `[imagen de ${v.length} bytes]`;
        }
        return limpia;
      });
      copia.resumen[tabla] = datos.length;
    } catch (e) {
      copia.tablas[tabla] = [];
      copia.resumen[tabla] = `no se pudo leer: ${e.message}`;
    }
  }

  try {
    const m = await filas(pool, 'SELECT COUNT(*)::int AS n, COALESCE(SUM(bytes), 0)::bigint AS b FROM media_blobs');
    copia.imagenes = { total: Number(m[0]?.n || 0), bytes: Number(m[0]?.b || 0) };
  } catch { /* sin imágenes */ }

  return copia;
}

/** Copia + envío por correo a la administración. Devuelve un resumen. */
export async function copiaPorCorreo(pool) {
  if (!correoConfigurado()) return { enviada: false, motivo: 'sin correo configurado' };

  const admins = await filas(
    pool,
    `SELECT email FROM users WHERE role = 'admin' AND status = 'active' AND email <> '' ORDER BY id LIMIT 5`
  );
  if (admins.length === 0) return { enviada: false, motivo: 'sin administradores con correo' };

  const copia = await volcarBase(pool);
  const texto = [
    `Copia de seguridad de Moon — ${copia.fecha}`,
    '',
    ...Object.entries(copia.resumen).map(([t, n]) => `${t}: ${n}`),
    '',
    `Imágenes guardadas: ${copia.imagenes.total} (${Math.round(copia.imagenes.bytes / 1048576)} MB)`,
    '',
    'El archivo adjunto tiene toda la información en formato JSON.',
  ].join('\n');

  const contenido = Buffer.from(JSON.stringify(copia, null, 2), 'utf8');
  let enviada = 0;
  for (const a of admins) {
    const r = await enviarCorreo(a.email, `Moon · copia de seguridad ${copia.fecha.slice(0, 10)}`, texto, {
      adjunto: { nombre: `copia-moon-${copia.fecha.slice(0, 10)}.json`, contenido },
    });
    if (r.enviado) enviada += 1;
  }
  return { enviada: enviada > 0, enviadas: enviada, resumen: copia.resumen };
}

/** Programa la copia diaria (a la hora indicada, en hora del servidor). */
export function programarCopiaDiaria(pool, horaLocal = 4) {
  const UNA_HORA = 3600_000;
  setInterval(() => {
    const ahora = new Date();
    if (ahora.getHours() !== horaLocal || ahora.getMinutes() > 20) return;
    if (pool.__copiaEnviada === ahora.toISOString().slice(0, 10)) return;
    pool.__copiaEnviada = ahora.toISOString().slice(0, 10);
    copiaPorCorreo(pool).then((r) => {
      if (r.enviada) console.log(`[copias] copia diaria enviada por correo (${r.enviadas} destinatarios)`);
      else console.log('[copias] copia diaria no enviada:', r.motivo || 'sin destinatarios');
    }).catch((e) => console.error('[copias] fallo en la copia diaria:', e.message));
  }, UNA_HORA).unref?.();
}
