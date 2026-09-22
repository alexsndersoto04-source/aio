// Moon — Almacén de archivos en Telegram (Fotos, Videos y Bóveda de Datos)
// =========================================================================
// Guarda fotos, videos y paquetes comprimidos de base de datos en canales
// privados de Telegram usando MTProto (GramJS).
// Canales: "Moon Fotos", "Moon Videos", "Moon — Bóveda de Dato".

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
 * Resuelve y asegura la entidad del canal de Bóveda.
 */
async function asegurarCanalBoveda(tg) {
  if (entidadBoveda) return entidadBoveda;

  // 1. Probar en diálogos buscando por título
  try {
    const dialogs = await tg.getDialogs({ limit: 100 });
    for (const d of dialogs) {
      const n = (d.title || d.name || '').trim().toLowerCase();
      if (n.includes('boveda') || n.includes('bóveda') || (n.includes('moon') && n.includes('dato')) || n.includes('boveda de dato')) {
        entidadBoveda = d.entity || d.inputEntity || d;
        console.log(`[tg-almacen] Canal Bóveda resuelto en diálogos: "${d.title}" (ID: ${d.id})`);
        return entidadBoveda;
      }
    }
  } catch (err) {
    console.warn('[tg-almacen] Advertencia escaneando diálogos:', err.message);
  }

  // 2. Probar CheckChatInvite con el hash de invitación
  if (BOVEDA_INVITE_HASH) {
    try {
      const invite = await tg.invoke(new Api.messages.CheckChatInvite({ hash: BOVEDA_INVITE_HASH }));
      if (invite && invite.chat) {
        entidadBoveda = invite.chat;
        console.log(`[tg-almacen] Canal Bóveda resuelto por CheckChatInvite: "${invite.chat.title}"`);
        return entidadBoveda;
      }
    } catch (checkErr) {
      // Ignorar si no es accesible por check directo
    }

    // 3. Probar ImportChatInvite para unirse
    try {
      const importRes = await tg.invoke(new Api.messages.ImportChatInvite({ hash: BOVEDA_INVITE_HASH }));
      if (importRes) {
        entidadBoveda = importRes.chats?.[0] || importRes.chat || importRes;
        console.log('[tg-almacen] Unido exitosamente al canal de Bóveda!');
        return entidadBoveda;
      }
    } catch (invErr) {
      if (invErr.errorMessage === 'USER_ALREADY_PARTICIPANT' || /already/i.test(invErr.message)) {
        try {
          const dialogs = await tg.getDialogs({ limit: 100 });
          for (const d of dialogs) {
            const n = (d.title || '').trim().toLowerCase();
            if (n.includes('boveda') || n.includes('bóveda') || n.includes('dato')) {
              entidadBoveda = d.entity || d.inputEntity || d;
              return entidadBoveda;
            }
          }
        } catch {}
      } else {
        console.warn('[tg-almacen] ImportChatInvite error:', invErr.message);
      }
    }
  }

  return entidadBoveda;
}

/**
 * Resuelve el canal privado buscándolo en los diálogos de la cuenta.
 */
async function resolverCanal(tg, tipo) {
  const t = String(tipo || '').toLowerCase();

  if (t.includes('boveda') || t.includes('dato') || t.includes('vault')) {
    return await asegurarCanalBoveda(tg);
  }

  if (t.startsWith('video') && entidadVideos) return entidadVideos;
  if (!t.startsWith('video') && entidadFotos) return entidadFotos;

  try {
    const dialogs = await tg.getDialogs({ limit: 80 });
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
    console.error('[tg-almacen] Error escaneando diálogos:', e.message);
  }

  if (t.startsWith('video')) return entidadVideos;
  return entidadFotos;
}

/**
 * Sube una imagen, video o paquete a Telegram.
 */
export async function subirATelegram(buffer, { nombre = 'archivo.bin', tipo = 'image/jpeg', caption = '' } = {}) {
  const tg = await obtenerClienteTelegram();
  if (!tg) {
    throw new Error('Sin sesión activa de Telegram configurada en el servidor');
  }

  const canal = await resolverCanal(tg, tipo);
  if (!canal) {
    throw new Error(`No se encontró el canal de Telegram para tipo "${tipo}". Asegúrate de que el bot o cuenta sea miembro o administrador del canal.`);
  }

  const esDoc = !tipo.startsWith('image/') && !tipo.startsWith('video/');

  let toUpload = buffer;
  try {
    const { CustomFile } = await import('telegram/client/uploads.js');
    toUpload = new CustomFile(nombre, buffer.length, '', buffer);
  } catch {
    toUpload = buffer;
  }

  const pie = caption || `Moon ${tipo}: ${nombre} (${new Date().toISOString()})`;
  const opcionesEnvio = {
    file: toUpload,
    caption: pie,
  };
  if (esDoc) {
    opcionesEnvio.forceDocument = true;
  }

  const mensaje = await tg.sendFile(canal, opcionesEnvio);

  console.log(`[tg-almacen] Archivo enviado a Telegram! ID: ${mensaje.id} (Tipo: ${tipo})`);
  return {
    ok: true,
    tg_id: mensaje.id,
  };
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
      canal_nombre: canal?.title || canal?.name || 'Moon — Bóveda de Dato',
    };
  } catch (e) {
    return { conectado: true, canal: false, motivo: e.message };
  }
}
