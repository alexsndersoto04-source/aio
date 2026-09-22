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
import { crearPool, crearPoolHibrido, migrar, auditar } from './db.mjs';
import { migrarA } from './migracion.mjs';
import { crearRouter, crearContexto, cors, manejadorErrores, json } from './nucleo.mjs';
import { registrarRutasAuth } from './rutas-auth.mjs';
import { registrarRutasSocial } from './rutas-social.mjs';
import { registrarRutasMensajes } from './rutas-mensajes.mjs';
import { registrarRutasAdmin } from './rutas-admin.mjs';
import { registrarRutasMedia } from './rutas-media.mjs';
import { registrarRutasHistorias } from './rutas-historias.mjs';
import { registrarRutasGrupos } from './rutas-grupos.mjs';
import { registrarRutasPush } from './rutas-push.mjs';
import { registrarRutasInteraccion } from './rutas-interaccion.mjs';
import { registrarRutasGruposExtra } from './rutas-grupos-extra.mjs';
import { registrarRutasPerfil } from './rutas-perfil.mjs';
import { registrarRutasTelegramAuth } from './rutas-telegram-auth.mjs';
import { registrarRutasBoveda } from './rutas-boveda.mjs';
import { microCache } from './cache-memoria.mjs';
import { inicializarDefensas, estaIpBloqueada, estadoModoBlindaje } from './defensas.mjs';
import { montarWs, conectados } from './ws.mjs';
import { importarDelDisco } from './medios.mjs';
import { programarCopiaDiaria } from './copias.mjs';
import { correoConfigurado, viaDeCorreo } from './correo.mjs';
import { demasiadoRapido } from './limites.mjs';
import { servirWeb, estadoWeb } from './estatico.mjs';
import { ApiErr } from './util.mjs';

const PUERTO = Number(process.env.PORT || 3000);

// Credenciales nuevas y frescas otorgadas por el usuario:
const URL_NEON_NUEVA = 'postgresql://neondb_owner:npg_XliM3eg0cSjd@ep-rough-wind-b5w04gn6-pooler.c-7.us-east-2.aws.neon.tech/neondb?sslmode=require';
const URL_SUPABASE_NUEVA = 'postgresql://postgres.frnzfirgsqxpggbrsoao:AlexSoto2316%40@aws-0-us-east-1.pooler.supabase.com:6543/postgres';

// Base Primaria (Neon):
const URL_BD_A = process.env.MOON_DB_OVERRIDE || URL_NEON_NUEVA;

// Base Secundaria de Respaldo / Failover (Supabase):
const URL_BD_B = process.env.DATABASE_URL_BACKUP || URL_SUPABASE_NUEVA;

const URL_BD = URL_BD_A;

const SECRETO = process.env.JWT_SECRET || 'moon_jwt_secret_ultra_seguro_2026_super_estable_resilient';
const BASE_PUBLICA = process.env.PUBLIC_BASE_URL || '';
const ORIGENES = (process.env.CORS_ORIGIN || '*').split(',').map((s) => s.trim()).filter(Boolean);

// La interfaz web vive en Cloudflare; todo lo que no sea /api se redirige allí
// (301) para que nadie se quede en la copia vieja que este servidor podría
// servir. Se apaga con MOON_WEB_REDIRECT=off o se cambia el destino.
const destinoWeb = process.env.MOON_WEB_REDIRECT ?? 'https://moon.alexsndersoto04.workers.dev';
const REDIR_WEB = destinoWeb === 'off' ? '' : destinoWeb.replace(/\/+$/, '');

if (!SECRETO || SECRETO.length < 32) {
  console.error('[api] JWT_SECRET debe existir y tener al menos 32 caracteres.');
  process.exit(1);
}

let pool = crearPoolHibrido(URL_BD_A, URL_BD_B);

// Explica en palabras llanas qué hacer cuando la conexión con la base de datos
// falla. Devuelve la lista de pistas, o vacía si es un fallo pasajero (que sí
// merece reintentos).
function pistasDeConexion(e, cadena) {
  const codigo = (e && e.code) || '';
  const texto = String((e && e.message) || '');
  if (String(cadena).includes('YOUR-PASSWORD')) {
    return [
      'La dirección DATABASE_URL todavía lleva el hueco [YOUR-PASSWORD].',
      'Hay que poner ahí la contraseña real de la base de datos.',
      'En Supabase: botón Connect → pestaña Session pooler → copiar la cadena',
      'y sustituir [YOUR-PASSWORD] por la contraseña que elegiste.',
    ];
  }
  if (codigo === '28P01' || /password authentication failed/i.test(texto)) {
    return [
      'La contraseña de la base de datos no coincide.',
      'Solución: en Supabase, Settings → Database → Reset database password,',
      'copia la contraseña nueva y pégala en DATABASE_URL, dentro de la cadena,',
      'en el lugar de [YOUR-PASSWORD]. Después guarda: se vuelve a publicar solo.',
    ];
  }
  if (codigo === '3D000') {
    return [
      'La base de datos indicada no existe.',
      'Al final de DATABASE_URL (después de la última barra) debe decir «postgres».',
    ];
  }
  if (['ENOTFOUND', 'EAI_AGAIN'].includes(codigo)) {
    return [
      'No se encontró el servidor de la base de datos.',
      'Revisa la dirección (el trozo entre la @ y el puerto) de DATABASE_URL.',
    ];
  }
  if (['ECONNREFUSED', 'ETIMEDOUT', 'EHOSTUNREACH', 'ENETUNREACH'].includes(codigo)) {
    return [
      'No se pudo llegar a la base de datos (red o puerto).',
      'Copia de nuevo la cadena desde Supabase, pestaña Session pooler.',
    ];
  }
  return [];
}

// La base de datos puede tardar en despertar (los servicios gratuitos se
// apagan cuando nadie los usa). En vez de rendirse al primer intento, se
// espera un poco y se vuelve a probar. Los errores de contraseña, en cambio,
// no se reintentan: se explican y se termina, para no llenar el registro.
async function prepararBase() {
  const intentos = Number(process.env.MOON_BD_INTENTOS || 6);
  for (let i = 1; i <= intentos; i += 1) {
    try {
      await migrar(pool);
      console.log('[bd-hibrida] ✅ Bases de datos migradas y listas');
      return;
    } catch (e) {
      console.error(`[bd] intento ${i} de ${intentos} falló: ${e.message}`);
      if (i === intentos) {
        console.warn('[bd] continuando arranque para permitir failover...');
        return;
      }
      await new Promise((r) => setTimeout(r, 2000 * i));
    }
  }
}

/** Dirección de la base de datos sin la contraseña, para poder revisarla. */
function destinoVisible(cadena) {
  try {
    const u = new URL(cadena);
    return `${u.hostname}:${u.port || 5432}${u.pathname} (usuario ${u.username})`;
  } catch {
    return 'dirección ilegible';
  }
}

await prepararBase();

// «Base en uso»: si una mudanza previa guardó la dirección de la base nueva
// (moon.db_override), desde este momento es la que manda. Se lee de la base
// de arranque y, de estar, se cambia todo a la base nueva.
let URL_USO = URL_BD;
try {
  const nota = (await pool.query("SELECT valor FROM app_settings WHERE clave = 'moon.db_override'")).rows[0];
  if (nota && nota.valor && nota.valor !== URL_BD) {
    const nueva = crearPool(nota.valor);
    await migrar(nueva); // garantiza que el esquema está al día en la base nueva
    console.log(`[bd] base en uso cambiada a: ${destinoVisible(nota.valor)}`);
    await pool.end().catch(() => {});
    pool = nueva;
    URL_USO = nota.valor;
  }
} catch (e) {
  console.error('[bd] no se pudo leer la nota «base en uso» (se sigue con la actual):', e.message);
}

// Migración automática (una sola vez): si MOON_MIGRATE_DEST está definido,
// se copia todo a la base nueva al arrancar. Solo migra a una base VACÍA con
// su esquema ya aplicado (ver migracion.mjs). Así la mudanza a Neon no
// requiere tocar nada más: se pone la variable, se despliega, y se copia solo.
const DESTINO_MIGRACION = process.env.MOON_MIGRATE_DEST || '';
if (DESTINO_MIGRACION) {
  migrarA(pool, DESTINO_MIGRACION)
    .then((informe) => {
      console.log(
        `[migracion] automática lista: ${informe.total_filas} filas en ${Object.keys(informe.tablas).length} tablas, fotos ${informe.fotos.destino} bytes (verificado). ` +
        'Ya se puede cambiar DATABASE_URL a la base nueva y quitar MOON_MIGRATE_DEST.'
      );
      auditar(pool, null, 'migracion_automatica', `${informe.total_filas} filas a la base nueva`).catch(() => {});
    })
    .catch((e) => {
      // No tumba el servicio: si la base nueva no está lista (sin esquema o ya
      // tiene datos), se queda tal cual y se corrige volviendo a desplegar.
      console.error('[migracion] automática no se ejecutó:', e.message);
    });
}

const router = crearRouter();
registrarRutasAuth(router);
registrarRutasSocial(router);
registrarRutasMensajes(router);
registrarRutasAdmin(router);
registrarRutasMedia(router);
registrarRutasHistorias(router);
registrarRutasGrupos(router);
registrarRutasPush(router);
registrarRutasInteraccion(router);
registrarRutasGruposExtra(router);
registrarRutasPerfil(router);
registrarRutasTelegramAuth(router);
registrarRutasBoveda(router);

// Salud (pública) y métricas (solo administración).
router.get('/api/health', async (c) => {
  let bd = true;
  try {
    await c.pool.query('SELECT 1');
  } catch {
    bd = false;
  }
  // Si las imágenes viven en la base de datos (lo normal), se informa: es lo
  // que garantiza que las fotos no se pierdan al reiniciar.
  let fotosEnBase = null;
  try {
    const r = await c.pool.query('SELECT COUNT(*)::int AS n FROM media_blobs');
    fotosEnBase = Number(r.rows[0]?.n || 0);
  } catch { fotosEnBase = null; }
  return {
    status: bd ? 'ok' : 'degraded',
    app: 'moon',
    time: new Date().toISOString(),
    db: bd,
    base: destinoVisible(URL_USO),
    motor: c.pool.motor || 'sin dato',
    canario: process.env.MOON_CANARIO || null,
    // Qué versión del código está corriendo (lo pone Render al desplegar):
    // sirve para saber si la API ya tiene los últimos cambios.
    commit: String(process.env.RENDER_GIT_COMMIT || process.env.GIT_COMMIT || '').slice(0, 7) || null,
    fotos_en_base: fotosEnBase,
    correo: correoConfigurado() ? viaDeCorreo() : 'sin configurar',
    telegram_almacen: Boolean(process.env.TELEGRAM_SESSION),
    boveda_tier: true,
  };
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
    micro_cache: microCache.estadisticas(),
    totales: r.rows[0],
  };
});

// Inicializar defensas y escudo en memoria
await inicializarDefensas(pool).catch(() => {});

// Las fotos que quedaran en el disco se pasan a la base de datos (una vez).
importarDelDisco(pool).catch(() => {});
// Copia de seguridad diaria por correo (si hay correo configurado).
programarCopiaDiaria(pool, Number(process.env.MOON_BACKUP_HORA || 4));

const aplicarCors = cors(ORIGENES);
const alError = manejadorErrores;

const servidor = createServer(async (req, res) => {
  aplicarCors(req, res);

  if (req.method === 'OPTIONS') {
    res.writeHead(204).end();
    return;
  }

  const ipCliente = String(req.headers['x-forwarded-for'] || req.socket.remoteAddress || '').split(',')[0].trim();
  if (estaIpBloqueada(ipCliente)) {
    json(res, 403, { error: 'Acceso denegado: tu dirección IP está bloqueada por seguridad' });
    return;
  }

  let camino;
  try {
    camino = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  } catch {
    json(res, 400, { error: 'Dirección inválida' });
    return;
  }

  // Límite general por IP (en Modo Blindaje es mucho más estricto para mitigar ataques).
  const maxPeticiones = estadoModoBlindaje() ? 80 : 300;
  if (camino.startsWith('/api/') && demasiadoRapido(`ip:${ipCliente}`, maxPeticiones, 60_000)) {
    json(res, 429, { error: 'Demasiadas peticiones, intenta en un minuto' });
    return;
  }

  const encontrado = router.buscar(req.method, camino);
  if (!encontrado) {
    // La web vive en Cloudflare: lo que no sea API se reenvía allí.
    if (REDIR_WEB) {
      res.writeHead(301, { Location: REDIR_WEB + req.url, 'Cache-Control': 'no-store' });
      res.end();
      return;
    }
    // No es una ruta de la API: puede ser la aplicación web compilada.
    if (servirWeb(req, res, camino)) return;
    json(res, 404, { error: `No existe ${req.method} ${camino}` });
    return;
  }

  // Para los enlaces de correo, prefiere el origen confiable de la web en uso
  // (Cloudflare) y cae en PUBLIC_BASE_URL si no está listado.
  const baseCorreos = ORIGENES.find((o) => o.includes('workers.dev')) || ORIGENES.find((o) => o !== '*') || BASE_PUBLICA;
  const c = crearContexto({ req, res, pool, secreto: SECRETO, basePublica: baseCorreos });
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
  console.log(`[api] Base de datos: ${destinoVisible(URL_USO)}`);
});

async function apagar(senal) {
  console.log(`\n[api] ${senal}: cerrando…`);
  servidor.close(() => {});
  await pool.end().catch(() => {});
  process.exit(0);
}
process.on('SIGTERM', () => apagar('SIGTERM'));
process.on('SIGINT', () => apagar('SIGINT'));
