// Moon — Almacén de archivos en Telegram (Fotos y Videos)
// ============================================================
// Guarda fotos y videos en canales privados de Telegram usando MTProto (GramJS).
// Neon Postgres solo almacena el ID del archivo y la referencia.

import { TelegramClient } from 'telegram';
import { StringSession } from 'telegram/sessions/index.js';

const API_ID = Number(process.env.TELEGRAM_API_ID || 37564514);
const API_HASH = process.env.TELEGRAM_API_HASH || '26564a3de304f28400a2c0eab6a14968';
const SESSION_STR = process.env.TELEGRAM_SESSION || '';
const CANAL_FOTOS = process.env.TELEGRAM_CANAL_FOTOS || 'https://t.me/+oA1Fs3nUzQs1MGNh';
const CANAL_VIDEOS = process.env.TELEGRAM_CANAL_VIDEOS || 'https://t.me/+etOgVw2JrjIzYmYx';

let clienteTg = null;

export async function obtenerClienteTelegram() {
  if (!SESSION_STR) return null;
  if (clienteTg) return clienteTg;

  try {
    const sesion = new StringSession(SESSION_STR);
    clienteTg = new TelegramClient(sesion, API_ID, API_HASH, {
      connectionRetries: 3,
    });
    await clienteTg.connect();
    return clienteTg;
  } catch (err) {
    console.error('[tg-almacen] No se pudo conectar a Telegram:', err.message);
    return null;
  }
}

/**
 * Sube una imagen o video al canal privado correspondiente en Telegram.
 */
export async function subirATelegram(buffer, { nombre = 'archivo', tipo = 'image' } = {}) {
  const tg = await obtenerClienteTelegram();
  if (!tg) return null;

  try {
    const destino = tipo.startsWith('video') ? CANAL_VIDEOS : CANAL_FOTOS;
    const { CustomFile } = await import('telegram/client/uploads.js');
    const toUpload = new CustomFile(nombre, buffer.length, '', buffer);
    const mensaje = await tg.sendFile(destino, {
      file: toUpload,
      caption: `Moon Media: ${nombre}`,
    });

    return {
      ok: true,
      tg_id: mensaje.id,
      canal: destino,
    };
  } catch (err) {
    console.error('[tg-almacen] Error subiendo archivo:', err.message);
    return null;
  }
}
