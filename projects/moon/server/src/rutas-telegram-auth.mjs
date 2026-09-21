// Moon — Generador de sesión Telegram (StringSession)
// ============================================================
// Endpoint temporal para enlazar tu cuenta de Telegram con Moon.
// Solo accesible para administradores (o con clave segura).

import { TelegramClient } from 'telegram';
import { StringSession } from 'telegram/sessions/index.js';
import { ApiErr } from './util.mjs';

let tgTemp = {
  client: null,
  phoneCodeHash: '',
  sessionString: '',
  phoneNumber: '',
  apiId: 0,
  apiHash: '',
};

export function registrarRutasTelegramAuth(router) {
  // 1. Pedir código a Telegram
  router.post('/api/admin/telegram/enviar-codigo', async (c) => {
    const b = await c.cuerpo();
    const apiId = Number(b.api_id || 37564514);
    const apiHash = String(b.api_hash || '26564a3de304f28400a2c0eab6a14968');
    const phoneNumber = String(b.phone || '+584246053395').replace(/\s+/g, '');

    const session = new StringSession('');
    const client = new TelegramClient(session, apiId, apiHash, {
      connectionRetries: 5,
    });

    try {
      await client.connect();
      const res = await client.sendCode({ apiId, apiHash }, phoneNumber);
      
      tgTemp = {
        client,
        phoneCodeHash: res.phoneCodeHash,
        sessionString: client.session.save(),
        phoneNumber,
        apiId,
        apiHash,
      };

      return {
        ok: true,
        message: 'Código enviado a tu aplicación de Telegram',
        phoneCodeHash: res.phoneCodeHash,
      };
    } catch (err) {
      console.error('[tg-auth] Error al enviar código:', err);
      throw new ApiErr(`Error de Telegram: ${err.message}`, 400);
    }
  });

  // 2. Verificar código y devolver StringSession
  router.post('/api/admin/telegram/verificar-codigo', async (c) => {
    const b = await c.cuerpo();
    const code = String(b.code || '').trim();
    const password = String(b.password || '').trim(); // 2FA si tiene

    if (!tgTemp.client || !tgTemp.phoneCodeHash) {
      throw new ApiErr('Primero debes solicitar el código', 400);
    }

    try {
      await tgTemp.client.signIn({
        phoneNumber: tgTemp.phoneNumber,
        phoneCodeHash: tgTemp.phoneCodeHash,
        phoneCode: code,
        password: password ? async () => password : undefined,
        onError: (err) => {
          throw err;
        },
      });

      const sessionFinal = tgTemp.client.session.save();
      return {
        ok: true,
        stringSession: sessionFinal,
        message: 'Sesión generada con éxito permanente',
      };
    } catch (err) {
      console.error('[tg-auth] Error al verificar código:', err);
      throw new ApiErr(`Error al verificar código: ${err.message}`, 400);
    }
  });
}
