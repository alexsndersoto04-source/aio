// Moon — Generador de sesión Telegram (StringSession)
// ============================================================
// Endpoint temporal para enlazar tu cuenta de Telegram con Moon.

import { TelegramClient, Api } from 'telegram';
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
      // Método nativo MTProto GramJS
      await tgTemp.client.invoke(
        new Api.auth.SignIn({
          phoneNumber: tgTemp.phoneNumber,
          phoneCodeHash: tgTemp.phoneCodeHash,
          phoneCode: code,
        })
      );

      const sessionFinal = tgTemp.client.session.save();
      return {
        ok: true,
        stringSession: sessionFinal,
        message: 'Sesión generada con éxito permanente',
      };
    } catch (err) {
      // Si pide contraseña 2FA de la cuenta de Telegram
      if (err.message && err.message.includes('SESSION_PASSWORD_NEEDED') && password) {
        try {
          // Si tiene 2FA configurada en Telegram
          const { computeCheck } = await import('telegram/Password.js');
          const pwd = await tgTemp.client.invoke(new Api.account.GetPassword());
          const myPassword = await computeCheck(pwd, password);
          await tgTemp.client.invoke(new Api.auth.CheckPassword({ password: myPassword }));
          const sessionFinal = tgTemp.client.session.save();
          return {
            ok: true,
            stringSession: sessionFinal,
            message: 'Sesión generada con éxito permanente (con 2FA)',
          };
        } catch (e2) {
          throw new ApiErr(`Contraseña 2FA incorrecta: ${e2.message}`, 400);
        }
      }
      console.error('[tg-auth] Error al verificar código:', err);
      throw new ApiErr(`Error al verificar código: ${err.message}`, 400);
    }
  });
}
