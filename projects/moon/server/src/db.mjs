// Moon — Base de datos (PostgreSQL real)
// ============================================================
// Pool de conexiones y migraciones. Las migraciones NO se escriben aquí:
// se extraen de `projects/moon/src/db.titan` (la fuente de verdad del
// repositorio) con `node extraer-esquema.mjs`, para que el servidor de
// desarrollo en Node y el servidor Titan en producción apliquen exactamente
// el mismo esquema.

import pg from 'pg';
import { Pool as NeonPool, neonConfig } from '@neondatabase/serverless';
import ws from 'ws';

// El driver serverless usa HTTP (fetch) para las consultas, así no se abren
// conexiones TCP persistentes y se evita el límite de conexiones de Render.
// Para las transacciones interactivas (migraciones) usa WebSocket.
neonConfig.webSocketConstructor = ws;
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const rutaEsquema = resolve(aqui, '../esquema.json');

export function crearPool(url) {
  let anfitrion = '';
  try { anfitrion = new URL(url).hostname; } catch { anfitrion = ''; }
  const esNeon = anfitrion.endsWith('.neon.tech') || anfitrion.includes('neon.tech');

  if (esNeon) {
    // Base de Neon: driver serverless, consultas por HTTP (sin pool TCP que
    // cuente contra el límite de conexiones del hosting).
    const pool = new NeonPool({ connectionString: url });
    pool.on('error', (e) => console.error('[bd] error de conexión:', e.message));
    return pool;
  }

  // Cualquier otra base (Supabase, Postgres propio…): pg como siempre.
  const ajustes = {
    connectionString: url,
    max: 10,
    idleTimeoutMillis: 30000,
    connectionTimeoutMillis: 15000,
  };
  const esLocal =
    !anfitrion || ['localhost', '127.0.0.1', '::1'].includes(anfitrion) || anfitrion.endsWith('.local');
  if (!esLocal) ajustes.ssl = { rejectUnauthorized: false };
  const pool = new pg.Pool(ajustes);
  pool.on('error', (e) => console.error('[bd] error en conexión inactiva:', e.message));
  return pool;
}

export async function migrar(pool) {
  const migraciones = JSON.parse(readFileSync(rutaEsquema, 'utf8'));
  await pool.query(`CREATE TABLE IF NOT EXISTS schema_migrations (
    version BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  )`);

  const aplicadas = new Set(
    (await pool.query('SELECT version FROM schema_migrations')).rows.map((r) => String(r.version))
  );

  let nuevas = 0;
  for (const m of migraciones) {
    if (aplicadas.has(String(m.version))) continue;
    const cliente = await pool.connect();
    try {
      await cliente.query('BEGIN');
      // Cada migración trae varias sentencias separadas por «;».
      const sentencias = m.sql
        .split(';')
        .map((s) => s.trim())
        .filter(Boolean);
      for (const s of sentencias) await cliente.query(s);
      await cliente.query('INSERT INTO schema_migrations (version, name) VALUES ($1, $2)', [m.version, m.name]);
      await cliente.query('COMMIT');
      nuevas += 1;
      console.log(`[bd] migración v${m.version} ${m.name} aplicada`);
    } catch (e) {
      await cliente.query('ROLLBACK').catch(() => {});
      console.error(`[bd] fallo en la migración v${m.version} (${m.name}):`, e.message);
      throw e;
    } finally {
      cliente.release();
    }
  }
  if (nuevas === 0) console.log('[bd] esquema al día');
  return nuevas;
}

// Azúcar para consultas.
export const q = (pool, sql, args = []) => pool.query(sql, args);
export const filas = async (pool, sql, args = []) => (await pool.query(sql, args)).rows;
export const fila = async (pool, sql, args = []) => (await pool.query(sql, args)).rows[0] || null;
export const uno = async (pool, sql, args = []) => (await pool.query(sql, args)).rows[0] || null;
export const contar = async (pool, sql, args = []) => Number((await pool.query(sql, args)).rows[0]?.count || 0);

// Registro de actividad (auditoría ligera) y estadísticas diarias.
export async function auditar(pool, userId, accion, detalle = '', ip = '') {
  await pool
    .query('INSERT INTO activity_log (user_id, action, detail, ip) VALUES ($1, $2, $3, $4)', [
      userId, accion, detalle.slice(0, 300), ip,
    ])
    .catch(() => {});
}

export async function sumarEstadistica(pool, campo) {
  const permitidos = ['new_users', 'new_posts', 'new_messages', 'new_likes', 'new_comments', 'new_follows'];
  if (!permitidos.includes(campo)) return;
  await pool
    .query(
      `INSERT INTO app_stats (stat_date, ${campo}) VALUES (CURRENT_DATE, 1)
       ON CONFLICT (stat_date) DO UPDATE SET ${campo} = app_stats.${campo} + 1`
    )
    .catch(() => {});
}
