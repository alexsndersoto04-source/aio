// Moon — Migración a otra base de datos (una sola vez)
// ============================================================
// Copia TODOS los datos de la base actual a una base nueva (p. ej. Neon),
// tal cual: usuarios, publicaciones, mensajes, grupos y las imágenes
// (que viven en la base como BYTEA). Al terminar, la base nueva queda
// lista para que el servidor la use en su lugar.
//
// Seguridad:
//   - Solo administración puede invocarlo.
//   - Solo migra a una base VACÍA (no pisa datos ajenos).
//   - La base destino debe tener el esquema aplicado (mismas tablas).
//   - No imprime ninguna cadena de conexión.

import pg from 'pg';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { filas, q, uno, migrar } from './db.mjs';

const aqui = dirname(fileURLToPath(import.meta.url));
const rutaEsquema = resolve(aqui, '../esquema.json');

// Orden de copia: primero el orden del esquema, luego se reordena según las
// dependencias reales (claves foráneas). Así una tabla que referencia a otra
// (p. ej. posts.group_id → groups, añadida por una migración tardía) se copia
// DESPUÉS de la que referencia.
function ordenTablas() {
  const migraciones = JSON.parse(readFileSync(rutaEsquema, 'utf8'));
  const orden = [];
  const vistas = new Set();
  for (const m of migraciones) {
    for (const sentencia of m.sql.split(';')) {
      const t = /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[\"']?(\w+)/i.exec(sentencia.trim());
      if (t && !vistas.has(t[1])) {
        orden.push(t[1]);
        vistas.add(t[1]);
      }
    }
  }
  return orden;
}

/** Reordena `tablas` para que ninguna se copie antes de las que referencia. */
async function ordenPorDependencias(pool, tablas) {
  const enLista = new Set(tablas);
  const dependeDe = {};
  for (const t of tablas) {
    const r = await filas(
      pool,
      `SELECT DISTINCT tgt.relname AS refiere
         FROM pg_constraint con
         JOIN pg_class src ON src.oid = con.conrelid
         JOIN pg_class tgt ON tgt.oid = con.confrelid
        WHERE con.contype = 'f' AND src.relname = $1 AND tgt.relname <> src.relname`,
      [t]
    );
    dependeDe[t] = r.filter((x) => enLista.has(x.refiere)).map((x) => x.refiere);
  }
  const restantes = new Set(tablas);
  const orden = [];
  while (restantes.size) {
    let avanzo = false;
    for (const t of [...restantes]) {
      if (dependeDe[t].every((d) => !restantes.has(d))) {
        orden.push(t);
        restantes.delete(t);
        avanzo = true;
      }
    }
    if (!avanzo) {
      // Ciclo de referencias (no debería ocurrir): se agregan las restantes.
      for (const t of [...restantes]) orden.push(t);
      break;
    }
  }
  return orden;
}

// Convierte el valor leído en un parámetro seguro para el INSERT:
// Buffer (bytea) y Date los maneja pg; los objetos (columnas JSON/JSONB)
// hay que serializarlos, porque pg no sabe qué hacer con un objeto.
function valorParametro(v) {
  if (v === null || v === undefined) return null;
  if (Buffer.isBuffer(v) || v instanceof Date) return v;
  if (typeof v === 'object') return JSON.stringify(v);
  return v;
}

function esLocal(url) {
  try {
    const h = new URL(url).hostname;
    return !h || ['localhost', '127.0.0.1', '::1'].includes(h) || h.endsWith('.local');
  } catch {
    return false;
  }
}

/**
 * Migra `pool` (la base del servidor) a `urlDestino`. Devuelve un informe.
 * Lanza un Error con mensaje en español si algo no procede.
 */
export async function migrarA(pool, urlDestino) {
  // --- Sanidad de la base destino -------------------------------
  const dest = new pg.Pool({
    connectionString: urlDestino,
    max: 2,
    connectionTimeoutMillis: 15000,
    ...(esLocal(urlDestino) ? {} : { ssl: { rejectUnauthorized: false } }),
  });
  dest.on('error', () => {});

  let destino;
  try {
    destino = await dest.connect();
  } catch (e) {
    await dest.end();
    throw new Error(`No se pudo conectar con la base nueva: ${e.message}`);
  }

  const informe = { tablas: {}, fotos: { origen: 0, destino: 0 }, secuencias: 0, esquema_creado: false };

  try {
    // Si la base nueva está completamente vacía (sin esquema), se crea el
    // esquema SOLO: se aplican las mismas migraciones que usa el servidor.
    // Así la mudanza a una base nueva (Neon) no requiere ningún paso manual.
    const tEsquema = await destino.query(
      `SELECT to_regclass('public.schema_migrations') AS existe`
    );
    if (!tEsquema.rows[0].existe) {
      const nuevas = await migrar(dest);
      console.log(`[migracion] esquema creado en la base nueva (${nuevas} migraciones)`);
      informe.esquema_creado = true;
    }

    // Solo migramos a una base vacía: así nunca se pisan datos.
    const ocupada = await destino.query(
      `SELECT (SELECT COUNT(*)::int FROM users) AS usuarios,
              (SELECT COUNT(*)::int FROM posts) AS publicaciones`
    ).catch(() => ({ rows: [{ usuarios: -1, publicaciones: -1 }] }));
    const { usuarios, publicaciones } = ocupada.rows[0];
    if (usuarios > 0 || publicaciones > 0) {
      throw new Error(`La base nueva ya tiene datos (${usuarios} usuarios, ${publicaciones} publicaciones). Por seguridad solo se migra a una base vacía.`);
    }

    // --- Copia tabla por tabla ---------------------------------
    const orden = ordenTablas();
    const reales = await filas(
      pool,
      `SELECT table_name FROM information_schema.tables
       WHERE table_schema = 'public' AND table_type = 'BASE TABLE'`
    );
    const realesSet = new Set(reales.map((r) => r.table_name));
    let copia = orden.filter((t) => realesSet.has(t));
    // Por si apareció una tabla nueva que el esquema no conoce.
    for (const r of reales) {
      if (!copia.includes(r.table_name) && r.table_name !== 'schema_migrations') copia.push(r.table_name);
    }
    // Reordena por dependencias reales (claves foráneas): posts no se copia
    // antes que groups, aunque el esquema haya creado posts primero.
    copia = await ordenPorDependencias(pool, copia);

    const LOTE = 200;
    for (const tabla of copia) {
      const cols = await filas(
        pool,
        `SELECT column_name FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = $1
         ORDER BY ordinal_position`,
        [tabla]
      );
      if (cols.length === 0) continue;
      const listaCols = cols.map((c) => `"${c.column_name}"`).join(', ');
      const total = Number((await uno(pool, `SELECT COUNT(*)::int AS n FROM "${tabla}"`))?.n || 0);
      let copiadas = 0;
      while (copiadas < total) {
        const filasLote = await filas(
          pool,
          `SELECT ${listaCols} FROM "${tabla}" LIMIT ${LOTE} OFFSET ${copiadas}`
        );
        if (filasLote.length === 0) break;
        const marcadores = [];
        const valores = [];
        for (const fila of filasLote) {
          const base = valores.length;
          marcadores.push(`(${cols.map((_, i) => '$' + (base + i + 1)).join(', ')})`);
          for (const c of cols) valores.push(valorParametro(fila[c.column_name]));
        }
        try {
          await destino.query(
            `INSERT INTO "${tabla}" (${listaCols}) VALUES ${marcadores.join(', ')}`,
            valores
          );
        } catch (e) {
          throw new Error(`En la tabla «${tabla}»: ${e.message}`);
        }
        copiadas += filasLote.length;
      }
      informe.tablas[tabla] = copiadas;

    }

    // --- Verificación de imágenes: todas las columnas bytea ----------
    const byteaCols = await filas(
      pool,
      `SELECT table_name, column_name FROM information_schema.columns
       WHERE table_schema = 'public' AND udt_name = 'bytea'`
    );
    let fotosOrigen = 0;
    let fotosDestino = 0;
    for (const tabla of [...new Set(byteaCols.map((c) => c.table_name))]) {
      const cols = byteaCols.filter((c) => c.table_name === tabla).map((c) => c.column_name);
      if (cols.length === 0) continue;
      const expr = cols.map((col) => `COALESCE(SUM(length("${col}")), 0)`).join(' + ');
      const [a, b] = await Promise.all([
        uno(pool, `SELECT (${expr})::bigint AS b FROM "${tabla}"`),
        destino.query(`SELECT (${expr})::bigint AS b FROM "${tabla}"`).then((r) => r.rows[0]),
      ]);
      const ao = Number(a?.b ?? 0);
      const bd = Number(b?.b ?? 0);
      fotosOrigen += ao;
      fotosDestino += bd;
      if (ao !== bd) {
        throw new Error(`Verificación de imágenes fallida en «${tabla}»: ${ao} bytes en origen, ${bd} en destino.`);
      }
    }
    informe.fotos.origen = fotosOrigen;
    informe.fotos.destino = fotosDestino;

    // --- Secuencias (para que las IDs nuevas no choquen) --------
    for (const tabla of copia) {
      const sec = await destino
        .query(`SELECT pg_get_serial_sequence('public."${tabla}"', 'id') AS s`)
        .catch(() => null);
      const nombre = sec?.rows[0]?.s;
      if (!nombre) continue;
      const maximo = (await destino.query(`SELECT COALESCE(MAX(id), 1) AS m FROM "${tabla}"`)).rows[0].m;
      await destino.query(`SELECT setval($1, $2, true)`, [nombre, Number(maximo)]);
      informe.secuencias += 1;
    }

    // --- Verificación final: conteos tabla por tabla ------------
    for (const tabla of copia) {
      const [a, b] = await Promise.all([
        uno(pool, `SELECT COUNT(*)::int AS n FROM "${tabla}"`),
        destino.query(`SELECT COUNT(*)::int AS n FROM "${tabla}"`).then((r) => r.rows[0]),
      ]);
      if (Number(a?.n ?? 0) !== Number(b?.n ?? 0)) {
        throw new Error(`Verificación fallida en la tabla «${tabla}»: ${a.n} en origen, ${b.n} en destino.`);
      }
    }

    informe.verificado = true;
    informe.total_filas = Object.values(informe.tablas).reduce((s, n) => s + n, 0);
    return informe;
  } finally {
    destino.release();
    await dest.end();
  }
}
