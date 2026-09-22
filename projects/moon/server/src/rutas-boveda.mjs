// Moon — Rutas API de la Bóveda de Datos (Tiered Storage)
// =========================================================================
// Endpoints administrativos para monitoreo de capacidad de Neon,
// generación de snapshots comprimidos y archivado de registros fríos en Telegram.

import {
  obtenerEstadoBoveda,
  listarLotesBoveda,
  crearSnapshotBoveda,
  archivarNotificacionesViejas,
  archivarLogsActividad,
  descargarLoteDeBoveda,
} from './boveda-datos.mjs';
import { ApiErr } from './util.mjs';

export function registrarRutasBoveda(router) {
  // Estado de almacenamiento general: Neon vs Telegram
  router.get('/api/boveda/estado', async (c) => {
    await c.admin();
    return await obtenerEstadoBoveda(c.pool);
  });

  // Lista de paquetes y lotes indexados en la Bóveda
  router.get('/api/boveda/lotes', async (c) => {
    await c.admin();
    return await listarLotesBoveda(c.pool, 50);
  });

  // Genera un Snapshot estructurado completo y lo guarda en Telegram
  router.post('/api/boveda/snapshot', async (c) => {
    await c.admin();
    try {
      const res = await crearSnapshotBoveda(c.pool);
      return { ok: true, mensaje: 'Snapshot asegurado en la Bóveda de Telegram', ...res };
    } catch (e) {
      throw new ApiErr(500, `Error generando snapshot en bóveda: ${e.message}`);
    }
  });

  // Archivar datos fríos bajo demanda
  router.post('/api/boveda/archivar', async (c) => {
    await c.admin();
    const b = await c.body();
    const tipo = b.tipo || 'todo';
    const dias = Number(b.dias || 30);

    const resultados = {};

    if (tipo === 'notificaciones' || tipo === 'todo') {
      try {
        resultados.notificaciones = await archivarNotificacionesViejas(c.pool, dias);
      } catch (e) {
        resultados.notificaciones = { error: e.message };
      }
    }

    if (tipo === 'actividad' || tipo === 'todo') {
      try {
        resultados.actividad = await archivarLogsActividad(c.pool, Math.min(dias, 15));
      } catch (e) {
        resultados.actividad = { error: e.message };
      }
    }

    return {
      ok: true,
      mensaje: 'Proceso de archivado en Bóveda completado',
      resultados,
    };
  });

  // Inspección de un lote archivado
  router.get('/api/boveda/lote/:id', async (c) => {
    await c.admin();
    const id = Number(c.params.id);
    const r = await c.pool.query('SELECT * FROM boveda_indice WHERE id = $1', [id]);
    const lote = r.rows[0];
    if (!lote) throw new ApiErr(404, 'Lote no encontrado en el índice');

    try {
      const contenido = await descargarLoteDeBoveda(lote.tg_msg_id);
      return {
        indice: lote,
        contenido,
      };
    } catch (e) {
      throw new ApiErr(500, `No se pudo descargar el lote desde Telegram: ${e.message}`);
    }
  });
}
