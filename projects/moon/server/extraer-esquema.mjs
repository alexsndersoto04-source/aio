// Extrae el esquema de la base de datos desde `projects/moon/src/db.titan`
// y lo escribe en `projects/moon/server/esquema.json`.
//
// El repositorio tiene UNA sola fuente de verdad para las tablas (el código
// de Titan, que es lo que se despliega). Este script la lee y genera el
// archivo que usa el servidor de desarrollo en Node, para que ambos apliquen
// exactamente las mismas migraciones.
//
// Uso, desde cualquier sitio:
//   node projects/moon/server/extraer-esquema.mjs

import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const origen = resolve(aqui, '../src/db.titan');
const destino = resolve(aqui, 'esquema.json');

const fuente = readFileSync(origen, 'utf8');

// Cada migración es una llamada:  mx_build(3, "nombre", "SQL")
const patron = /mx_build\(\s*(\d+)\s*,\s*"([^"]*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)/g;

const migraciones = [];
let m;
while ((m = patron.exec(fuente)) !== null) {
  const [, version, nombre, sqlCrudo] = m;
  // Deshacer los escapes de la cadena de Titan.
  const sql = sqlCrudo.replace(/\\"/g, '"').replace(/\\n/g, '\n').replace(/\\\\/g, '\\');
  migraciones.push({ version: Number(version), name: nombre, sql });
}

if (migraciones.length === 0) {
  console.error('No se encontró ninguna migración en', origen);
  process.exit(1);
}

migraciones.sort((a, b) => a.version - b.version);

// El fichero lo consume el servidor de desarrollo; se guarda con el texto
// legible para poder revisarlo.
writeFileSync(destino, JSON.stringify(migraciones, null, 2) + '\n', 'utf8');

console.log(`Migraciones extraídas de db.titan: ${migraciones.length}`);
for (const mi of migraciones) {
  const tablas = [...mi.sql.matchAll(/CREATE TABLE (?:IF NOT EXISTS )?([a-z_]+)/g)].map((x) => x[1]);
  console.log(`  v${String(mi.version).padStart(2)} ${mi.name}${tablas.length ? ` (tablas: ${tablas.join(', ')})` : ''}`);
}
console.log(`Escrito: ${destino}`);
