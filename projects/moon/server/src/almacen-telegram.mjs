// Moon — Almacén de archivos en Telegram (Fotos y Videos)
// ============================================================
// Guarda fotos y videos en canales privados de Telegram usando MTProto (GramJS).
// Busca automáticamente los canales por nombre ("Moon Fotos" y "Moon Videos").

import { TelegramClient } from 'telegram';
import { StringSession } from 'telegram/sessions/index.js';

const API_ID = Number(process.env.TELEGRAM_API_ID || 37564514);
const API_HASH = process.env.TELEGRAM_API_HASH || '26564a3de304f28400a2c0eab6a14968';
const SESSION_STR = process.env.TELEGRAM_SESSION || '';

let clienteTg = null;
let entidadFotos = null;
let entidadVideos = null;

export async function obtenerClienteTelegram() {
  if (!SESSION_STR) return null;
  if (clienteTg) return clienteTg;

  try {
    const sesion = new StringSession(SESSION_STR);
    clienteTg = new TelegramClient(sesion, API_ID, API_HASH, {
      connectionRetries: 3,
    });
    await clienteTg.connect();
    console.log('[tg-almacen] Cliente Telegram conectado exitosamente');
    return clienteTg;
  } catch (err) {
    console.error('[tg-almacen] No se pudo conectar a Telegram:', err.message);
    return null;
  }
}

/**
 * Resuelve el canal privado buscándolo en los diálogos de tu cuenta.
 */
async function resolverCanal(tg, tipo) {
  if (tipo.startsWith('video') && entidadVideos) return entidadVideos;
  if (!tipo.startsWith('video') && entidadFotos) return entidadFotos;

  try {
    const dialogs = await tg.getDialogs({});
    for (const d of dialogs) {
      const nombre = (d.title || '').trim().toLowerCase();
      if (nombre.includes('foto') && !entidadFotos) {
        entidadFotos = d.entity || d.inputEntity;
      }
      if (nombre.includes('video') && !entidadVideos) {
        entidadVideos = d.entity || d.inputEntity;
      }
    }
  } catch (e) {
    console.error('[tg-almacen] Error resolviendo canales:', e.message);
  }

  return tipo.startsWith('video') ? entidadVideos : entidadFotos;
}

/**
 * Sube una imagen o video al canal privado correspondiente en Telegram.
 */
export async function subirATelegram(buffer, { nombre = 'archivo.jpg', tipo = 'image/jpeg' } = {}) {
  const tg = await obtenerClienteTelegram();
  if (!tg) {
    console.warn('[tg-almacen] Sin sesion activa de Telegram, archivo omitido');
    return null;
  }

  try {
    const canal = await resolverCanal(tg, tipo);
    if (!canal) {
      console.warn('[tg-almacen] No se encontró el canal en los chats de tu cuenta');
      return null;
    }

    const { CustomFile } = await import('telegram/client/uploads.js');
    const toUpload = new CustomFile(nombre, buffer.length, '', buffer);

    const mensaje = await tg.sendFile(canal, {
      file: toUpload,
      caption: `Moon Media: ${nombre} (${new Date().toISOString()})`,
    });

    console.log(`[tg-almacen] Archivo enviado a Telegram con éxito! ID: ${mensaje.id}`);
    return {
      ok: true,
      tg_id: mensaje.id,
    };
  } catch (err) {
    console.error('[tg-almacen] Error subiendo archivo a Telegram:', err.message);
    return null;
  }
}
