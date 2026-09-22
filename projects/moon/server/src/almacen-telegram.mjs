// Moon — Almacén de archivos en Telegram (Fotos, Videos y Bóveda de Datos)
// =========================================================================
// Guarda fotos, videos y paquetes comprimidos de base de datos en canales
// privados de Telegram usando MTProto (GramJS).
// Busca automáticamente los canales por nombre ("Moon Fotos", "Moon Videos", "Moon — Bóveda de Dato").

import { TelegramClient, Api } from 'telegram';
import { StringSession } from 'telegram/sessions/index.js';

const API_ID = Number(process.env.TELEGRAM_API_ID || 37564514);
const API_HASH = process.env.TELEGRAM_API_HASH || '26564a3de304f28400a2c0eab6a14968';
const SESSION_STR = process.env.TELEGRAM_SESSION || '';
const BOVEDA_INVITE_HASH = process.env.TELEGRAM_BOVEDA_HASH || 'jM4QNDy178JmNjdh';

let clienteTg = null;
let entidadFotos = null;
let entidadVideos = null;
let entidadBoveda = null;

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
 * Resuelve el canal privado buscándolo en los diálogos de la cuenta.
 */
async function resolverCanal(tg, tipo) {
  const t = String(tipo || '').toLowerCase();

  if (t.includes('boveda') || t.includes('dato') || t.includes('vault')) {
    if (entidadBoveda) return entidadBoveda;
  } else if (t.startsWith('video')) {
    if (entidadVideos) return entidadVideos;
  } else {
    if (entidadFotos) return entidadFotos;
  }

  try {
    const dialogs = await tg.getDialogs({});
    for (const d of dialogs) {
      const nombre = (d.title || '').trim().toLowerCase();
      if ((nombre.includes('boveda') || nombre.includes('bóveda') || nombre.includes('dato')) && !entidadBoveda) {
        entidadBoveda = d.entity || d.inputEntity;
        console.log(`[tg-almacen] Canal Bóveda detectado: "${d.title}" (ID: ${d.id})`);
      }
      if (nombre.includes('foto') && !entidadFotos) {
        entidadFotos = d.entity || d.inputEntity;
      }
      if (nombre.includes('video') && !entidadVideos) {
        entidadVideos = d.entity || d.inputEntity;
      }
    }
  } catch (e) {
    console.error('[tg-almacen] Error escaneando diálogos:', e.message);
  }

  // Si se busca la bóveda y aún no está en los diálogos, intentar unirse con el hash
  if ((t.includes('boveda') || t.includes('dato') || t.includes('vault')) && !entidadBoveda && BOVEDA_INVITE_HASH) {
    try {
      console.log('[tg-almacen] Intentando unirse al canal de Bóveda por enlace de invitación...');
      const res = await tg.invoke(new Api.messages.ImportChatInvite({ hash: BOVEDA_INVITE_HASH }));
      if (res) {
        entidadBoveda = res.chats?.[0] || res.chat || res;
        console.log('[tg-almacen] Unión exitosa al canal de Bóveda!');
      }
    } catch (invErr) {
      if (invErr.errorMessage === 'USER_ALREADY_PARTICIPANT' || /already/i.test(invErr.message)) {
        try {
          const dialogs = await tg.getDialogs({});
          for (const d of dialogs) {
            const n = (d.title || '').trim().toLowerCase();
            if (n.includes('boveda') || n.includes('bóveda') || n.includes('dato')) {
              entidadBoveda = d.entity || d.inputEntity;
              break;
            }
          }
        } catch {}
      } else {
        console.warn('[tg-almacen] No se pudo unir por invitación:', invErr.message);
      }
    }
  }

  if (t.includes('boveda') || t.includes('dato') || t.includes('vault')) return entidadBoveda;
  if (t.startsWith('video')) return entidadVideos;
  return entidadFotos;
}

/**
 * Sube una imagen, video o paquete a Telegram.
 */
export async function subirATelegram(buffer, { nombre = 'archivo.bin', tipo = 'image/jpeg', caption = '' } = {}) {
  const tg = await obtenerClienteTelegram();
  if (!tg) {
    console.warn('[tg-almacen] Sin sesión activa de Telegram, archivo omitido');
    return null;
  }

  try {
    const canal = await resolverCanal(tg, tipo);
    if (!canal) {
      console.warn(`[tg-almacen] No se encontró canal para tipo "${tipo}"`);
      return null;
    }

    let toUpload = buffer;
    try {
      const { CustomFile } = await import('telegram/client/uploads.js');
      toUpload = new CustomFile(nombre, buffer.length, '', buffer);
    } catch {
      toUpload = buffer;
    }

    const pie = caption || `Moon ${tipo}: ${nombre} (${new Date().toISOString()})`;
    const mensaje = await tg.sendFile(canal, {
      file: toUpload,
      caption: pie,
    });

    console.log(`[tg-almacen] Archivo enviado a Telegram! ID: ${mensaje.id} (Tipo: ${tipo})`);
    return {
      ok: true,
      tg_id: mensaje.id,
    };
  } catch (err) {
    console.error('[tg-almacen] Error subiendo archivo a Telegram:', err.message);
    return null;
  }
}

/**
 * Descarga los bytes de un archivo guardado en Telegram mediante el id del mensaje.
 */
export async function descargarDeTelegram(tgId, { tipo = 'video' } = {}) {
  const tg = await obtenerClienteTelegram();
  if (!tg) return null;

  try {
    const canal = await resolverCanal(tg, tipo);
    if (!canal) return null;

    const mensajes = await tg.getMessages(canal, { ids: [Number(tgId)] });
    const msg = mensajes && mensajes[0];
    if (!msg || !msg.media) return null;

    const buffer = await tg.downloadMedia(msg, {});
    return buffer ? Buffer.from(buffer) : null;
  } catch (err) {
    console.error('[tg-almacen] Error descargando archivo de Telegram:', err.message);
    return null;
  }
}

/**
 * Rescata un video de Telegram buscando su nombre en el canal si no tenemos el ID guardado.
 */
export async function recuperarVideoPorNombre(nombre) {
  const tg = await obtenerClienteTelegram();
  if (!tg) return null;

  try {
    const canal = await resolverCanal(tg, 'video');
    if (!canal) return null;

    const mensajes = await tg.getMessages(canal, { limit: 40 });
    for (const msg of mensajes) {
      if (msg.message && msg.message.includes(nombre) && msg.media) {
        console.log(`[tg-almacen] Rescatando video ${nombre} desde Telegram...`);
        const buffer = await tg.downloadMedia(msg, {});
        return buffer ? { bytes: Buffer.from(buffer), tg_id: msg.id } : null;
      }
    }
  } catch (err) {
    console.error('[tg-almacen] Error buscando video por nombre en Telegram:', err.message);
  }
  return null;
}

/**
 * Comprueba si el canal de Bóveda de Datos está disponible.
 */
export async function estadoCanalBoveda() {
  const tg = await obtenerClienteTelegram();
  if (!tg) return { conectado: false, canal: false, motivo: 'Sin sesión Telegram configurada' };
  try {
    const canal = await resolverCanal(tg, 'boveda');
    return {
      conectado: true,
      canal: Boolean(canal),
      canal_nombre: canal?.title || canal?.name || 'Moon — Bóveda de Datos',
    };
  } catch (e) {
    return { conectado: true, canal: false, motivo: e.message };
  }
}
