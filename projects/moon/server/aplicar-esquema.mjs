// Moon — Aplicar el esquema (migraciones) a una base de datos PostgreSQL
// ============================================================
// Uso: node aplicar-esquema.mjs <DATABASE_URL>
//
// Aplica exactamente las mismas migraciones que el servidor, pero sin
// arrancar la API. Sirve para preparar una base nueva (Neon, Render…)
// antes de mover los datos o el servicio.
//
// IMPORTANTE: no imprime la cadena de conexión (es un secreto).

import { crearPool, migrar } from './src/db.mjs';

const url = process.argv[2];
if (!url) {
  console.error('Uso: node aplicar-esquema.mjs <DATABASE_URL>');
  process.exit(1);
}

const pool = crearPool(url);

try {
  const v = await pool.query('SELECT version()');
  console.log('[bd] conectado a:', v.rows[0].version.slice(0, 30));

  const nuevas = await migrar(pool);
  console.log(`[bd] ${nuevas > 0 ? `aplicadas ${nuevas} migraciones` : 'esquema ya estaba al día'}`);

  const t = await pool.query(`SELECT COUNT(*)::int AS n FROM information_schema.tables WHERE table_schema = 'public'`);
  console.log(`[bd] tablas totales en la base: ${t.rows[0].n}`);
  console.log('[ok] listo para recibir datos');
} catch (e) {
  console.error('[error]', e.message);
  process.exitCode = 1;
} finally {
  await pool.end();
}
