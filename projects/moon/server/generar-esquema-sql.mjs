// Moon — Generar el esquema completo en un solo archivo SQL
// ============================================================
// Uso: node generar-esquema-sql.mjs [archivo_de_salida]
//
// Produce `esquema-moon.sql` con TODO el esquema de Moon en un solo
// archivo, listo para pegar en un editor de consultas (por ejemplo, la
// consola SQL de Neon) o ejecutar con `psql -f`. Idempotente: al
// terminar marca las migraciones como aplicadas, de modo que el
// servidor no las vuelve a correr.
//
// Regenerar cuando cambie el esquema: node extraer-esquema.mjs && node generar-esquema-sql.mjs

import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const migraciones = JSON.parse(readFileSync(resolve(aqui, 'esquema.json'), 'utf8'));
const salida = process.argv[2] || resolve(aqui, 'esquema-moon.sql');

const lineas = [
  '-- Moon — esquema completo (generado; no editar a mano)',
  `-- ${migraciones.length} migraciones · ${new Date().toISOString()}`,
  '-- Ejecutar una sola vez en la base nueva (consola SQL de Neon o psql -f).',
  '',
  'BEGIN;',
  '',
  'CREATE TABLE IF NOT EXISTS schema_migrations (',
  '  version BIGINT PRIMARY KEY,',
  '  name TEXT NOT NULL,',
  '  applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()',
  ');',
  '',
];

for (const m of migraciones) {
  lineas.push(`-- v${m.version} ${m.name}`);
  for (const s of m.sql.split(';').map((x) => x.trim()).filter(Boolean)) {
    lineas.push(s + ';');
  }
  lineas.push(
    `INSERT INTO schema_migrations (version, name) VALUES (${m.version}, '${m.name.replace(/'/g, "''")}') ON CONFLICT (version) DO NOTHING;`
  );
  lineas.push('');
}

lineas.push('COMMIT;');
lineas.push('');
lineas.push(`-- Listo: ${migraciones.length} migraciones aplicadas.`);
writeFileSync(salida, lineas.join('\n'), 'utf8');
console.log(`[ok] esquema SQL escrito en ${salida} (${migraciones.length} migraciones, ${lineas.length} líneas)`);
