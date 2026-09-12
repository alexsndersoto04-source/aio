// Moon — Puesta en marcha del entorno real (un solo comando)
// ============================================================
// Levanta PostgreSQL de verdad (motor incluido, sin instalarlo en el
// sistema), aplica el esquema y arranca la API.
//
//   node dev.mjs            → arranca la base de datos y la API
//   node dev.mjs --solo-bd  → solo la base de datos
//   node dev.mjs --reset    → borra los datos y empieza de cero
//
// Variables de entorno opcionales:
//   MOON_PG_DIR    carpeta de datos (por defecto /tmp/moon-pgdata)
//   MOON_PG_PORT   puerto (por defecto 55432)
//   PORT           puerto de la API (por defecto 3000)

import { spawnSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync, readFileSync } from 'node:fs';
import { randomBytes } from 'node:crypto';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
const BIN = resolve(aqui, 'node_modules/@embedded-postgres/linux-x64/native/bin');
const DATOS = process.env.MOON_PG_DIR || '/tmp/moon-pgdata';
const PUERTO_BD = Number(process.env.MOON_PG_PORT || 55432);
const PUERTO_API = Number(process.env.PORT || 3000);
const USUARIO = 'moon';
const BASE = 'moon';
const ARCHIVO_SECRETO = resolve(aqui, '.jwt-secret');

const argumentos = process.argv.slice(2);
const soloBd = argumentos.includes('--solo-bd');
const reiniciar = argumentos.includes('--reset');

for (const binario of ['initdb', 'pg_ctl', 'postgres']) {
  if (!existsSync(resolve(BIN, binario))) {
    console.error(`Falta el motor PostgreSQL (${binario}). Ejecuta: npm install`);
    process.exit(1);
  }
}

function ejecutar(binario, args, opciones = {}) {
  const r = spawnSync(resolve(BIN, binario), args, {
    stdio: opciones.silencioso ? 'pipe' : 'inherit',
    encoding: 'utf8',
    env: { ...process.env, PGDATA: DATOS },
  });
  if (r.status !== 0 && !opciones.permitirFallo) {
    throw new Error(`${binario} falló (código ${r.status}): ${r.stderr || ''}`);
  }
  return r;
}

function baseEnMarcha() {
  const r = ejecutar('pg_ctl', ['-D', DATOS, 'status'], { silencioso: true, permitirFallo: true });
  return r.status === 0;
}

if (reiniciar && existsSync(DATOS)) {
  console.log('[dev] --reset: se borran los datos anteriores');
  if (baseEnMarcha()) ejecutar('pg_ctl', ['-D', DATOS, '-m', 'fast', 'stop'], { silencioso: true, permitirFallo: true });
  rmSync(DATOS, { recursive: true, force: true });
}

if (!existsSync(DATOS)) {
  console.log(`[dev] creando la base de datos en ${DATOS}`);
  mkdirSync(DATOS, { recursive: true });
  ejecutar('initdb', ['-D', DATOS, '-U', USUARIO, '--auth=trust', '-E', 'UTF8', '--no-sync']);
  // Solo desarrollo local: conexiones de confianza desde 127.0.0.1.
  writeFileSync(
    resolve(DATOS, 'pg_hba.conf'),
    [
      'local   all all trust',
      'host    all all 127.0.0.1/32 trust',
      'host    all all ::1/128      trust',
      '',
    ].join('\n')
  );
}

if (!baseEnMarcha()) {
  console.log(`[dev] arrancando PostgreSQL en el puerto ${PUERTO_BD}`);
  const log = resolve(DATOS, 'arranque.log');
  ejecutar('pg_ctl', [
    '-D', DATOS,
    '-l', log,
    '-o', `-p ${PUERTO_BD} -k /tmp -c listen_addresses=127.0.0.1`,
    '-w', '-t', '30',
    'start',
  ]);
} else {
  console.log('[dev] PostgreSQL ya estaba en marcha');
}

// La base de datos `moon` se crea una sola vez. El paquete del motor no
// incluye `psql`, así que se hace con el cliente de Node.
{
  const pg = (await import('pg')).default;
  const admin = new pg.Client({
    host: '127.0.0.1', port: PUERTO_BD, user: USUARIO, database: 'postgres',
  });
  await admin.connect();
  const r = await admin.query('SELECT 1 FROM pg_database WHERE datname = $1', [BASE]);
  if (r.rowCount === 0) {
    await admin.query(`CREATE DATABASE ${BASE}`);
    console.log(`[dev] base de datos «${BASE}» creada`);
  }
  await admin.end();
}

const cadena = `postgres://${USUARIO}@127.0.0.1:${PUERTO_BD}/${BASE}`;
console.log(`[dev] DATABASE_URL=${cadena}`);

if (soloBd) {
  console.log('[dev] base de datos lista (--solo-bd)');
  process.exit(0);
}

// Secreto JWT persistente entre arranques (si no viene del entorno).
let secreto = process.env.JWT_SECRET || '';
if (!secreto) {
  if (existsSync(ARCHIVO_SECRETO)) secreto = readFileSync(ARCHIVO_SECRETO, 'utf8').trim();
  if (!secreto || secreto.length < 32) {
    secreto = randomBytes(32).toString('hex');
    writeFileSync(ARCHIVO_SECRETO, secreto, { mode: 0o600 });
  }
}

const api = spawn(process.execPath, [resolve(aqui, 'src/index.mjs')], {
  stdio: 'inherit',
  env: {
    ...process.env,
    DATABASE_URL: cadena,
    PORT: String(PUERTO_API),
    JWT_SECRET: secreto,
    CORS_ORIGIN: process.env.CORS_ORIGIN || '*',
    MOON_UPLOADS: process.env.MOON_UPLOADS || resolve(aqui, '../uploads'),
  },
});

api.on('exit', (codigo) => {
  console.log(`[dev] la API terminó (código ${codigo})`);
  process.exit(codigo || 0);
});
