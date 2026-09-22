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
    pool.motor = 'neon-http';
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
  pool.motor = 'pg-tcp';
  return pool;
}

/**
 * Pool Híbrido con Failover Automático:
 * Combina dos proveedores de base de datos (ej. Neon como Primaria y Supabase como Secundaria).
 * Si la base primaria se satura, agota cuota o cae, el servidor conmuta
 * automáticamente a la secundaria sin interrupción del servicio.
 */
export class PoolHibrido {
  constructor(urlA, urlB) {
    this.urlA = urlA;
    this.urlB = urlB;
    this.poolA = urlA ? crearPool(urlA) : null;
    this.poolB = urlB ? crearPool(urlB) : null;
    this.activo = 'A';
    this.esHibrido = true;
  }

  get poolActual() {
    if (this.activo === 'A' && this.poolA) return this.poolA;
    if (this.poolB) return this.poolB;
    return this.poolA;
  }

  async query(sql, args = []) {
    try {
      return await this.poolActual.query(sql, args);
    } catch (err) {
      const msg = String(err.message || '').toLowerCase();
      const esFalloConexion = msg.includes('terminated') || msg.includes('econnreset') ||
        msg.includes('connection') || msg.includes('timeout') || msg.includes('quota') ||
        msg.includes('rate limit') || msg.includes('suspended') || msg.includes('limit reached');

      if (esFalloConexion && this.poolB && this.activo === 'A') {
        console.warn(`[bd-hibrida] ⚠️ Alerta en BD Primaria (${err.message}). Activando Failover automático a Supabase...`);
        this.activo = 'B';
        return await this.poolB.query(sql, args);
      } else if (esFalloConexion && this.poolA && this.activo === 'B') {
        console.warn(`[bd-hibrida] ⚠️ Alerta en BD Secundaria (${err.message}). Conmutando a BD Primaria Neon...`);
        this.activo = 'A';
        return await this.poolA.query(sql, args);
      }
      throw err;
    }
  }

  async connect() {
    try {
      return await this.poolActual.connect();
    } catch (err) {
      if (this.poolB && this.activo === 'A') {
        console.warn(`[bd-hibrida] Conmutando conexión interactiva a base de respaldo...`);
        this.activo = 'B';
        return await this.poolB.connect();
      }
      throw err;
    }
  }

  on(event, handler) {
    if (this.poolA && typeof this.poolA.on === 'function') this.poolA.on(event, handler);
    if (this.poolB && typeof this.poolB.on === 'function') this.poolB.on(event, handler);
  }
}

export function crearPoolHibrido(urlA, urlB) {
  if (!urlB) return crearPool(urlA);
  return new PoolHibrido(urlA, urlB);
}

async function aplicarMigracionesEnPool(pool, etiqueta = 'bd') {
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
      const sentencias = m.sql
        .split(';')
        .map((s) => s.trim())
        .filter(Boolean);
      for (const s of sentencias) await cliente.query(s);
      await cliente.query('INSERT INTO schema_migrations (version, name) VALUES ($1, $2)', [m.version, m.name]);
      await cliente.query('COMMIT');
      nuevas += 1;
      console.log(`[bd-${etiqueta}] migración v${m.version} ${m.name} aplicada`);
    } catch (e) {
      await cliente.query('ROLLBACK').catch(() => {});
      console.error(`[bd-${etiqueta}] fallo en la migración v${m.version} (${m.name}):`, e.message);
      throw e;
    } finally {
      cliente.release();
    }
  }
  if (nuevas === 0) console.log(`[bd-${etiqueta}] esquema al día`);
  return nuevas;
}

export async function migrar(pool) {
  if (pool instanceof PoolHibrido || pool.esHibrido) {
    let exito = false;
    if (pool.poolA) {
      try {
        await aplicarMigracionesEnPool(pool.poolA, 'primaria');
        exito = true;
      } catch (errA) {
        console.warn('[bd-hibrida] No se pudo migrar BD primaria, probando secundaria:', errA.message);
      }
    }
    if (pool.poolB) {
      try {
        await aplicarMigracionesEnPool(pool.poolB, 'secundaria');
        exito = true;
      } catch (errB) {
        console.warn('[bd-hibrida] No se pudo migrar BD secundaria:', errB.message);
      }
    }
    if (!exito) {
      throw new Error('No se pudo aplicar migraciones en ninguna de las bases de datos');
    }
    return;
  }
  return await aplicarMigracionesEnPool(pool, 'unica');
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
