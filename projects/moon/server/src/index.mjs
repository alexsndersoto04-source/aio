// Moon — Servidor (API + WebSocket)
// ============================================================
// Servidor HTTP real: enruta /api/*, atiende /ws, aplica CORS, limita
// peticiones y sirve las imágenes subidas.
//
// Variables de entorno:
//   DATABASE_URL    cadena de conexión de PostgreSQL (obligatoria)
//   PORT            puerto de escucha (por defecto 3000)
//   JWT_SECRET      secreto para firmar tokens (mínimo 32 caracteres)
//   CORS_ORIGIN     orígenes permitidos, separados por comas
//   PUBLIC_BASE_URL dirección pública del frontend (enlaces de correo)
//   MOON_UPLOADS    carpeta de imágenes (por defecto ../../uploads)

import { createServer } from 'node:http';
import { crearPool, migrar } from './db.mjs';
import { crearRouter, crearContexto, cors, manejadorErrores, json } from './nucleo.mjs';
import { registrarRutasAuth } from './rutas-auth.mjs';
import { registrarRutasSocial } from './rutas-social.mjs';
import { registrarRutasMensajes } from './rutas-mensajes.mjs';
import { registrarRutasAdmin } from './rutas-admin.mjs';
import { registrarRutasMedia } from './rutas-media.mjs';
import { registrarRutasHistorias } from './rutas-historias.mjs';
import { montarWs, conectados } from './ws.mjs';
import { demasiadoRapido } from './limites.mjs';
import { servirWeb, estadoWeb } from './estatico.mjs';
import { ApiErr } from './util.mjs';

const PUERTO = Number(process.env.PORT || 3000);
const URL_BD = process.env.DATABASE_URL || 'postgres://moon@127.0.0.1:5432/moon';
const SECRETO = process.env.JWT_SECRET || '';
const BASE_PUBLICA = process.env.PUBLIC_BASE_URL || '';
const ORIGENES = (process.env.CORS_ORIGIN || '*').split(',').map((s) => s.trim()).filter(Boolean);

if (!SECRETO || SECRETO.length < 32) {
  console.error('[api] JWT_SECRET debe existir y tener al menos 32 caracteres.');
  process.exit(1);
}

const pool = crearPool(URL_BD);
await migrar(pool);

const router = crearRouter();
registrarRutasAuth(router);
registrarRutasSocial(router);
registrarRutasMensajes(router);
registrarRutasAdmin(router);
registrarRutasMedia(router);
registrarRutasHistorias(router);

// Salud (pública) y métricas (solo administración).
router.get('/api/health', async (c) => {
  let bd = true;
  try {
    await c.pool.query('SELECT 1');
  } catch {
    bd = false;
  }
  return { status: bd ? 'ok' : 'degraded', app: 'moon', time: new Date().toISOString(), db: bd };
});

router.get('/api/metrics', async (c) => {
  await c.admin();
  const r = await c.pool.query(`SELECT
      (SELECT COUNT(*)::int FROM users) AS users,
      (SELECT COUNT(*)::int FROM posts WHERE status = 'active') AS posts,
      (SELECT COUNT(*)::int FROM messages) AS messages,
      (SELECT COUNT(*)::int FROM notifications) AS notifications`);
  return {
    service: 'moon-api',
    time: new Date().toISOString(),
    websockets: conectados(),
    memoria_mb: Math.round(process.memoryUsage().rss / 1048576),
    totales: r.rows[0],
  };
});

const aplicarCors = cors(ORIGENES);
const alError = manejadorErrores;

const servidor = createServer(async (req, res) => {
  aplicarCors(req, res);

  if (req.method === 'OPTIONS') {
    res.writeHead(204).end();
    return;
  }

  let camino;
  try {
    camino = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  } catch {
    json(res, 400, { error: 'Dirección inválida' });
    return;
  }

  // Límite general por IP para frenar abusos (las rutas sensibles tienen el suyo).
  if (camino.startsWith('/api/') && demasiadoRapido(`ip:${req.socket.remoteAddress}`, 300, 60_000)) {
    json(res, 429, { error: 'Demasiadas peticiones, intenta en un minuto' });
    return;
  }

  const encontrado = router.buscar(req.method, camino);
  if (!encontrado) {
    // No es una ruta de la API: puede ser la aplicación web compilada.
    if (servirWeb(req, res, camino)) return;
    json(res, 404, { error: `No existe ${req.method} ${camino}` });
    return;
  }

  const c = crearContexto({ req, res, pool, secreto: SECRETO, basePublica: BASE_PUBLICA });
  c.params = encontrado.params;
  req._secreto = SECRETO;

  try {
    const datos = await encontrado.ruta.handler(c);
    // Las rutas que envían la respuesta por su cuenta (imágenes, descargas)
    // ya tienen las cabeceras fuera: aquí no hay nada más que hacer.
    if (res.writableEnded || res.headersSent) return;
    if (datos === undefined || datos === null) {
      res.writeHead(204).end();
      return;
    }
    json(res, 200, datos);
  } catch (e) {
    if (res.headersSent) {
      console.error('[api] fallo con la respuesta ya iniciada:', e);
      res.end();
      return;
    }
    if (e instanceof ApiErr) {
      json(res, e.status, e.code ? { error: e.message, code: e.code } : { error: e.message });
      return;
    }
    alError(res)(e);
  }
});

montarWs(servidor, pool, SECRETO);

servidor.listen(PUERTO, '0.0.0.0', () => {
  console.log(`[api] Moon escuchando en http://0.0.0.0:${PUERTO}`);
  console.log(`[api] WebSocket en /ws · ${router.rutas.length} rutas registradas`);
  console.log(`[api] Interfaz web: ${estadoWeb()}`);
});

async function apagar(senal) {
  console.log(`\n[api] ${senal}: cerrando…`);
  servidor.close(() => {});
  await pool.end().catch(() => {});
  process.exit(0);
}
process.on('SIGTERM', () => apagar('SIGTERM'));
process.on('SIGINT', () => apagar('SIGINT'));
