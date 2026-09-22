// Moon — Bóveda de Datos Fría (Telegram Tiered Storage)
// =========================================================================
// Arquitectura de almacenamiento en dos capas:
//   1. Capa Caliente (Neon PostgreSQL): datos activos y transacciones frecuentes (< 500 MB).
//   2. Capa Fría (Telegram Bóveda de Datos): archivos históricos, snapshots y registros antiguos
//      comprimidos en gzip al ~90% de ahorro con almacenamiento infinito a coste cero.

import { gzipSync, gunzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { subirATelegram, descargarDeTelegram, estadoCanalBoveda } from './almacen-telegram.mjs';
import { microCache } from './cache-memoria.mjs';

const LIMITE_NEON_BYTES = 500 * 1024 * 1024; // 500 MB cuota free Neon
const CACHE_LOTES = new Map(); // LRU memoria temporal (máx 15 lotes)

function sha256(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

/**
 * Consulta el estado de almacenamiento actual en Neon y el historial de la Bóveda.
 */
export async function obtenerEstadoBoveda(pool) {
  let bytesUsados = 0;
  let desglose = [];
  let errorBd = null;

  try {
    const resDb = await pool.query('SELECT pg_database_size(current_database()) AS bytes');
    bytesUsados = Number(resDb.rows[0]?.bytes || 0);

    const resTablas = await pool.query(`
      SELECT relname AS tabla,
             pg_total_relation_size(relid) AS bytes,
             n_live_tup AS filas_aprox
      FROM pg_stat_user_tables
      ORDER BY bytes DESC
      LIMIT 12
    `);
    desglose = resTablas.rows.map((r) => ({
      tabla: r.tabla,
      bytes: Number(r.bytes || 0),
      filas: Number(r.filas_aprox || 0),
    }));
  } catch (e) {
    errorBd = e.message;
  }

  // Totales archivados en la bóveda
  let resumenBoveda = {
    total_lotes: 0,
    total_registros: 0,
    bytes_originales: 0,
    bytes_comprimidos: 0,
    ahorro_bytes: 0,
    ahorro_porcentaje: 0,
  };

  try {
    const resBov = await pool.query(`
      SELECT COUNT(*)::int AS total_lotes,
             COALESCE(SUM(total_registros), 0)::bigint AS total_registros,
             COALESCE(SUM(bytes_originales), 0)::bigint AS bytes_originales,
             COALESCE(SUM(bytes_comprimidos), 0)::bigint AS bytes_comprimidos
      FROM boveda_indice
    `);
    const fila = resBov.rows[0];
    const orig = Number(fila.bytes_originales || 0);
    const comp = Number(fila.bytes_comprimidos || 0);
    const ahorro = Math.max(0, orig - comp);
    const pct = orig > 0 ? Math.round((ahorro / orig) * 100) : 0;

    resumenBoveda = {
      total_lotes: Number(fila.total_lotes || 0),
      total_registros: Number(fila.total_registros || 0),
      bytes_originales: orig,
      bytes_comprimidos: comp,
      ahorro_bytes: ahorro,
      ahorro_porcentaje: pct,
    };
  } catch (e) {
    // Si la tabla no ha sido creada aún
  }

  const estadoTg = await estadoCanalBoveda();

  const porcentajeNeon = Math.min(100, Math.round((bytesUsados / LIMITE_NEON_BYTES) * 100));

  return {
    capacidad_neon: {
      limite_mb: 500,
      usado_mb: +(bytesUsados / 1048576).toFixed(2),
      usado_bytes: bytesUsados,
      porcentaje_usado: porcentajeNeon,
      desglose_tablas: desglose,
      error: errorBd,
    },
    boveda_telegram: {
      ...estadoTg,
      ...resumenBoveda,
    },
    micro_cache: microCache.estadisticas(),
  };
}

/**
 * Empaqueta un array de registros, los comprime en gzip y los guarda en Telegram.
 */
export async function archivarLoteEnBoveda(pool, { modulo, subtipo = '', registros = [], metadatos = {} }) {
  if (!registros.length) return { omitido: true, motivo: 'Sin registros para archivar' };

  const datosJson = JSON.stringify({
    version: 1,
    modulo,
    subtipo,
    generado_en: new Date().toISOString(),
    total: registros.length,
    datos: registros,
    metadatos,
  });

  const bufferOriginal = Buffer.from(datosJson, 'utf8');
  const bytesOriginales = bufferOriginal.length;
  const checksum = sha256(bufferOriginal);

  // Compresión máxima gzip (reduce ~85-92% en JSON)
  const bufferComprimido = gzipSync(bufferOriginal, { level: 9 });
  const bytesComprimidos = bufferComprimido.length;

  const fechaCompacta = new Date().toISOString().replace(/[:.]/g, '-');
  const nombreArchivo = `boveda_${modulo}_${fechaCompacta}.json.gz`;
  const caption = `📦 Moon Bóveda: [${modulo}] ${registros.length} registros (${+(bytesOriginales / 1024).toFixed(1)} KB -> ${+(bytesComprimidos / 1024).toFixed(1)} KB | -${Math.round((1 - bytesComprimidos / bytesOriginales) * 100)}%)`;

  const subida = await subirATelegram(bufferComprimido, {
    nombre: nombreArchivo,
    tipo: 'boveda',
    caption,
  });

  if (!subida || !subida.tg_id) {
    throw new Error('No se pudo enviar el paquete comprimido a Telegram. Verifica la conexión y permisos del canal.');
  }

  // Extraer rangos de ID y fecha si están disponibles
  const ids = registros.map((r) => Number(r.id)).filter(Number.isFinite);
  const minId = ids.length ? Math.min(...ids) : null;
  const maxId = ids.length ? Math.max(...ids) : null;

  const fechas = registros.map((r) => r.created_at || r.creado_en).filter(Boolean);
  const minFecha = fechas.length ? new Date(fechas[0]) : null;
  const maxFecha = fechas.length ? new Date(fechas[fechas.length - 1]) : null;

  const ins = await pool.query(
    `INSERT INTO boveda_indice (
      modulo, subtipo, desde_id, hasta_id, desde_fecha, hasta_fecha,
      total_registros, tg_msg_id, bytes_originales, bytes_comprimidos,
      checksum_sha256, metadatos
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
    RETURNING id`,
    [
      modulo,
      subtipo,
      minId,
      maxId,
      minFecha,
      maxFecha,
      registros.length,
      subida.tg_id,
      bytesOriginales,
      bytesComprimidos,
      checksum,
      JSON.stringify(metadatos),
    ]
  );

  return {
    ok: true,
    indice_id: ins.rows[0]?.id,
    tg_msg_id: subida.tg_id,
    registros: registros.length,
    bytes_originales: bytesOriginales,
    bytes_comprimidos: bytesComprimidos,
    ahorro_porcentaje: Math.round((1 - bytesComprimidos / bytesOriginales) * 100),
  };
}

/**
 * Recupera y descomprime un lote archivado desde Telegram mediante su ID de mensaje.
 */
export async function descargarLoteDeBoveda(tgMsgId) {
  const claveCache = String(tgMsgId);
  if (CACHE_LOTES.has(claveCache)) {
    return CACHE_LOTES.get(claveCache);
  }

  const bytes = await descargarDeTelegram(tgMsgId, { tipo: 'boveda' });
  if (!bytes) {
    throw new Error(`No se pudo descargar el lote ${tgMsgId} desde Telegram.`);
  }

  let jsonStr;
  try {
    const descompreso = gunzipSync(bytes);
    jsonStr = descompreso.toString('utf8');
  } catch (e) {
    // Si no estaba comprimido en gzip, leer como texto directo
    jsonStr = bytes.toString('utf8');
  }

  const paquete = JSON.parse(jsonStr);

  // Mantener caché de hasta 15 lotes en memoria
  if (CACHE_LOTES.size >= 15) {
    const primerClave = CACHE_LOTES.keys().next().value;
    CACHE_LOTES.delete(primerClave);
  }
  CACHE_LOTES.set(claveCache, paquete);

  return paquete;
}

/**
 * Genera un Snapshot de Respaldo Completo de la aplicación y lo asegura en la Bóveda de Telegram.
 */
export async function crearSnapshotBoveda(pool) {
  // Extraemos datos esenciales estructurados de las tablas principales
  const [usuarios, posts, comentarios, grupos, configuracion] = await Promise.all([
    pool.query('SELECT id, username, email, display_name, role, status, created_at FROM users ORDER BY id ASC'),
    pool.query("SELECT id, user_id, content, status, likes_count, comments_count, created_at FROM posts WHERE status = 'active' ORDER BY id ASC"),
    pool.query("SELECT id, post_id, user_id, content, status, created_at FROM comments WHERE status = 'active' ORDER BY id ASC"),
    pool.query('SELECT id, name, slug, privacy, created_at FROM groups ORDER BY id ASC'),
    pool.query('SELECT clave, valor FROM app_settings'),
  ]);

  const paquete = {
    tipo_snapshot: 'completo_estructurado',
    fecha_generacion: new Date().toISOString(),
    tablas: {
      users: usuarios.rows,
      posts: posts.rows,
      comments: comentarios.rows,
      groups: grupos.rows,
      app_settings: configuracion.rows,
    },
    resumen: {
      usuarios: usuarios.rowCount,
      publicaciones: posts.rowCount,
      comentarios: comentarios.rowCount,
      grupos: grupos.rowCount,
    },
  };

  const resultado = await archivarLoteEnBoveda(pool, {
    modulo: 'snapshot_completo',
    subtipo: 'respaldo_estructurado',
    registros: [paquete],
    metadatos: paquete.resumen,
  });

  return resultado;
}

/**
 * Archiva y purga notificaciones leídas de más de `dias` días hacia la Bóveda de Telegram.
 */
export async function archivarNotificacionesViejas(pool, dias = 30) {
  const consulta = await pool.query(
    `SELECT id, user_id, from_user_id, type, post_id, comment_id, content, is_read, created_at
     FROM notifications
     WHERE is_read = TRUE
       AND created_at < NOW() - ($1 || ' days')::interval
     ORDER BY id ASC
     LIMIT 1000`,
    [dias]
  );

  if (!consulta.rows.length) {
    return { purgadas: 0, motivo: 'No hay notificaciones viejas para archivar' };
  }

  const resultado = await archivarLoteEnBoveda(pool, {
    modulo: 'notificaciones',
    subtipo: 'leidas_antiguas',
    registros: consulta.rows,
    metadatos: { dias_antiguedad: dias },
  });

  // Una vez asegurado en Telegram, limpiamos de Neon
  const ids = consulta.rows.map((r) => r.id);
  await pool.query('DELETE FROM notifications WHERE id = ANY($1::bigint[])', [ids]);

  return {
    ...resultado,
    purgadas: ids.length,
  };
}

/**
 * Archiva y purga el log de auditoría antiguo hacia la Bóveda de Telegram.
 */
export async function archivarLogsActividad(pool, dias = 20) {
  const consulta = await pool.query(
    `SELECT id, user_id, action, detail, ip, created_at
     FROM activity_log
     WHERE created_at < NOW() - ($1 || ' days')::interval
     ORDER BY id ASC
     LIMIT 2000`,
    [dias]
  );

  if (!consulta.rows.length) {
    return { purgadas: 0, motivo: 'No hay logs antiguos para archivar' };
  }

  const resultado = await archivarLoteEnBoveda(pool, {
    modulo: 'auditoria',
    subtipo: 'activity_log_historico',
    registros: consulta.rows,
    metadatos: { dias_antiguedad: dias },
  });

  const ids = consulta.rows.map((r) => r.id);
  await pool.query('DELETE FROM activity_log WHERE id = ANY($1::bigint[])', [ids]);

  return {
    ...resultado,
    purgadas: ids.length,
  };
}

/**
 * Lista los últimos paquetes guardados en la Bóveda.
 */
export async function listarLotesBoveda(pool, limite = 30) {
  const r = await pool.query(
    `SELECT id, modulo, subtipo, desde_id, hasta_id, desde_fecha, hasta_fecha,
            total_registros, tg_msg_id, bytes_originales, bytes_comprimidos,
            checksum_sha256, metadatos, creado_en
     FROM boveda_indice
     ORDER BY id DESC
     LIMIT $1`,
    [limite]
  );
  return r.rows;
}
