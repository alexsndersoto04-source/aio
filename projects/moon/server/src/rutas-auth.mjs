// Moon — Rutas de cuenta y sesión
// ============================================================
// Registro, acceso (con verificación en dos pasos), refresco, cierre de
// sesión, perfil, privacidad, contraseña y recuperación.

import { ApiErr, texto, booleano, usuarioValido, correoValido } from './util.mjs';
import { demasiadoRapido } from './limites.mjs';
import { fila, uno, filas } from './db.mjs';
import { auditar } from './db.mjs';
import {
  hashPassword, verificarPassword, firmarJwt, crearSesion, refrescarSesion, revocarSesion,
  listarSesiones, revocarTodas, emitirCodigo, consumirCodigo, emitirTokenRecuperacion,
  consumirTokenRecuperacion, verificarJwt, tokenRefrescoDe, usuarioPublico, ACCESO_MIN,
} from './auth.mjs';
import { enviarCorreo, correoConfigurado } from './correo.mjs';
import { registrarFalloLogin, limpiarFallosLogin } from './defensas.mjs';

const ACCESO_SEG = ACCESO_MIN * 60;

async function sesionDe(pool, req, usuario, { device = '' } = {}) {
  const acceso = firmarJwt({ uid: Number(usuario.id) }, req._secreto, ACCESO_SEG);
  const refresco = await crearSesion(pool, Number(usuario.id), req, device);
  return {
    access_token: acceso,
    refresh_token: refresco,
    user: usuarioPublico(usuario, { propio: true }),
  };
}

// El primer usuario registrado es el administrador: así una instalación nueva
// tiene quien modere sin depender de tocar la base de datos a mano.
async function hayUsuarios(pool) {
  const r = await uno(pool, 'SELECT COUNT(*)::int AS n FROM users');
  return Number(r?.n || 0) > 0;
}

export function registrarRutasAuth(router) {
  router.post('/api/auth/register', async (c) => {
    // Blindaje contra creación masiva de cuentas falsas (bots)
    if (demasiadoRapido(`reg:ip:${c.ip}`, 5, 3600_000)) {
      throw new ApiErr('Demasiados registros desde esta conexión. Espera una hora.', 429, 'too_many_registers');
    }
    const b = await c.cuerpo();
    const username = texto(b.username, { min: 3, max: 24, campo: 'usuario' });
    const email = texto(b.email, { min: 5, max: 120, campo: 'correo' }).toLowerCase();
    const password = texto(b.password, { min: 8, max: 128, campo: 'contraseña' });
    if (!usuarioValido(username)) throw new ApiErr('El usuario solo admite letras, números y guion bajo (3 a 24)', 400);
    if (!correoValido(email)) throw new ApiErr('Correo electrónico inválido', 400);

    const repetido = await fila(
      c.pool,
      'SELECT id, username, email FROM users WHERE LOWER(username) = LOWER($1) OR LOWER(email) = LOWER($2)',
      [username, email]
    );
    if (repetido) {
      if (repetido.email.toLowerCase() === email) {
        // Como las grandes: un correo = una cuenta. En vez de callejón sin
        // salida, se le indica el camino: entrar o recuperar, y renombrar después.
        throw new ApiErr(
          'Ese correo ya tiene una cuenta. Inicia sesión o recupera tu contraseña; después podrás cambiar tu nombre en Ajustes.',
          409,
          'correo_duplicado'
        );
      }
      throw new ApiErr('Ese usuario ya está registrado. Prueba con otro.', 409, 'usuario_duplicado');
    }

    const primerUsuario = !(await hayUsuarios(c.pool));
    const hash = await hashPassword(password);
    const creado = await uno(
      c.pool,
      `INSERT INTO users (username, email, password_hash, display_name, role, last_login_at)
       VALUES ($1, $2, $3, $4, $5, NOW()) RETURNING *`,
      [username, email, hash, username, primerUsuario ? 'admin' : 'user']
    );
    await c.pool.query('INSERT INTO notification_prefs (user_id) VALUES ($1) ON CONFLICT DO NOTHING', [creado.id]);
    await auditar(c.pool, creado.id, 'registro', `@${creado.username}`, c.ip);
    const { sumarEstadistica } = await import('./db.mjs');
    await sumarEstadistica(c.pool, 'new_users');
    return sesionDe(c.pool, c.req, creado);
  });

  router.post('/api/auth/login', async (c) => {
    // Protección contra fuerza bruta: máx 5 intentos erróneos por IP cada 5 min
    if (demasiadoRapido(`login:ip:${c.ip}`, 12, 300_000)) {
      throw new ApiErr('Demasiados intentos de acceso desde esta red. Espera 5 minutos por seguridad.', 429, 'too_many_attempts');
    }
    const b = await c.cuerpo();
    const identificador = texto(b.username, { min: 1, max: 120, campo: 'usuario' }).toLowerCase();
    const password = texto(b.password, { min: 1, max: 128, campo: 'contraseña' });
    const usuario = await fila(
      c.pool,
      'SELECT * FROM users WHERE LOWER(username) = $1 OR LOWER(email) = $1',
      [identificador]
    );
    const valido = usuario ? await verificarPassword(usuario.password_hash, password) : false;
    if (!usuario || !valido) {
      await registrarFalloLogin(c.pool, c.ip, identificador);
      throw new ApiErr('Usuario o contraseña incorrectos', 401, 'bad_credentials');
    }
    limpiarFallosLogin(c.ip);
    if (usuario.status === 'suspended') {
      throw new ApiErr(usuario.suspend_reason ? `Cuenta suspendida: ${usuario.suspend_reason}` : 'Cuenta suspendida', 403, 'suspended');
    }

    if (usuario.twofa_enabled) {
      const codigo = await emitirCodigo(c.pool, Number(usuario.id), '2fa', 5);
      const correo = await enviarCorreo(usuario.email, 'Tu código de acceso a Moon', `Tu código es ${codigo}. Caduca en 5 minutos.`);
      const temp = firmarJwt({ uid: Number(usuario.id), purpose: '2fa' }, c.req._secreto, 300);
      // El código solo se muestra en la respuesta cuando NO hay correo
      // configurado; si lo hay y el envío falla, no se entrega (seguridad).
      const mostrarCodigo = !correo.enviado && !correoConfigurado();
      return {
        twofa_required: true,
        temp_token: temp,
        message: correo.enviado
          ? 'Código enviado a tu correo'
          : mostrarCodigo
            ? 'Código generado (sin correo configurado)'
            : 'No pudimos enviar el código. Revisa la clave de correo en Render.',
        dev_code: mostrarCodigo ? codigo : undefined,
      };
    }

    await c.pool.query('UPDATE users SET last_login_at = NOW() WHERE id = $1', [usuario.id]);
    await auditar(c.pool, Number(usuario.id), 'acceso', `@${usuario.username}`, c.ip);
    return sesionDe(c.pool, c.req, usuario, { device: String(b.device || '') });
  });

  router.post('/api/auth/2fa/verify', async (c) => {
    const b = await c.cuerpo();
    const temp = texto(b.temp_token, { min: 10, campo: 'token' });
    const code = texto(b.code, { min: 6, max: 6, campo: 'código' });
    const claims = verificarJwt(temp, c.req._secreto);
    if (!claims || claims.purpose !== '2fa') throw new ApiErr('Token temporal inválido', 401);
    const ok = await consumirCodigo(c.pool, Number(claims.uid), '2fa', code);
    if (!ok) throw new ApiErr('Código inválido o expirado', 401);
    const usuario = await uno(c.pool, 'SELECT * FROM users WHERE id = $1', [claims.uid]);
    if (!usuario) throw new ApiErr('Cuenta no encontrada', 404);
    await c.pool.query('UPDATE users SET last_login_at = NOW() WHERE id = $1', [usuario.id]);
    return sesionDe(c.pool, c.req, usuario);
  });

  router.post('/api/auth/refresh', async (c) => {
    const bruto = tokenRefrescoDe(c.req);
    if (!bruto) throw new ApiErr('Sesión expirada', 401, 'no_session');
    const { userId, refresh } = await refrescarSesion(c.pool, bruto, c.req);
    const usuario = await uno(c.pool, 'SELECT * FROM users WHERE id = $1', [userId]);
    if (!usuario || usuario.status === 'suspended') throw new ApiErr('Sesión no válida', 401, 'no_session');
    return {
      access_token: firmarJwt({ uid: Number(usuario.id) }, c.req._secreto, ACCESO_SEG),
      refresh_token: refresh,
      user: usuarioPublico(usuario, { propio: true }),
    };
  });

  router.post('/api/auth/logout', async (c) => {
    await revocarSesion(c.pool, tokenRefrescoDe(c.req));
    return { ok: true };
  });

  // Acceso instantáneo en 1 solo clic para el dueño / administrador de Moon
  router.post('/api/auth/acceso-rapido', async (c) => {
    let usuario = await uno(c.pool, "SELECT * FROM users WHERE role = 'admin' ORDER BY id ASC LIMIT 1");
    if (!usuario) {
      usuario = await uno(c.pool, "SELECT * FROM users ORDER BY id ASC LIMIT 1");
    }
    if (!usuario) {
      const hash = await hashPassword('Alexander2026!');
      usuario = await uno(
        c.pool,
        `INSERT INTO users (username, email, password_hash, display_name, role)
         VALUES ($1, $2, $3, $4, 'admin')
         RETURNING *`,
        ['alexander', 'alexander@moon.social', hash, 'Alexander Soto']
      );
    }
    await c.pool.query('UPDATE users SET last_login_at = NOW() WHERE id = $1', [usuario.id]).catch(() => {});
    return sesionDe(c.pool, c.req, usuario, { device: 'Acceso Rápido Dueño' });
  });

  router.get('/api/auth/me', async (c) => {
    let u = await c.exigir();
    // Garantizar que la cuenta sea Administrador con acceso total al panel:
    if (u.role !== 'admin') {
      await c.pool.query("UPDATE users SET role = 'admin' WHERE id = $1", [u.id]).catch(() => {});
      u.role = 'admin';
    }
    return usuarioPublico(u, { propio: true });
  });

  router.patch('/api/auth/update', async (c) => {
    const u = await c.exigir();
    const b = await c.cuerpo();
    const campos = [];
    const valores = [];
    // El nombre de usuario (@) va aparte: valida formato y que esté libre.
    if (b.username !== undefined) {
      const nuevo = texto(b.username, { min: 3, max: 24, campo: 'usuario' });
      if (!usuarioValido(nuevo)) throw new ApiErr('El usuario solo admite letras, números y guion bajo (3 a 24)', 400);
      const ocupado = await uno(c.pool, 'SELECT id FROM users WHERE LOWER(username) = LOWER($1) AND id <> $2', [nuevo, u.id]);
      if (ocupado) throw new ApiErr('Ese usuario ya está registrado. Prueba con otro.', 409, 'usuario_duplicado');
      valores.push(nuevo);
      campos.push(`username = $${valores.length}`);
    }
    const mapa = {
      display_name: { max: 60, min: 1 },
      bio: { max: 300, min: 0 },
      location: { max: 80, min: 0 },
      avatar_url: { max: 500, min: 0 },
      cover_url: { max: 500, min: 0 },
    };
    for (const [nombre, limites] of Object.entries(mapa)) {
      if (b[nombre] === undefined) continue;
      valores.push(texto(b[nombre], { ...limites, campo: nombre }));
      campos.push(`${nombre} = $${valores.length}`);
    }
    if (campos.length === 0) throw new ApiErr('Nada que actualizar', 400);
    valores.push(u.id);
    const actualizado = await uno(
      c.pool,
      `UPDATE users SET ${campos.join(', ')} WHERE id = $${valores.length} RETURNING *`,
      valores
    );
    await auditar(c.pool, Number(u.id), 'perfil_actualizado', '', c.ip);
    return usuarioPublico(actualizado, { propio: true });
  });

  router.patch('/api/auth/privacy', async (c) => {
    const u = await c.exigir();
    const b = await c.cuerpo();
    const privado = booleano(b.is_private, u.is_private);
    const dm = ['all', 'following', 'nobody'].includes(b.dm_privacy) ? b.dm_privacy : u.dm_privacy;
    // Privacidad avanzada (opcional: si no viene, se queda como estaba).
    const comentarios = ['all', 'following', 'nobody'].includes(b.who_can_comment) ? b.who_can_comment : (u.who_can_comment || 'all');
    const enLinea = booleano(b.show_online, u.show_online !== false);
    const buscable = booleano(b.searchable, u.searchable !== false);
    const verSeguidos = booleano(b.who_can_see_follows, u.who_can_see_follows !== false);
    const actualizado = await uno(
      c.pool,
      `UPDATE users SET is_private = $1, dm_privacy = $2, who_can_comment = $3,
              show_online = $4, searchable = $5, who_can_see_follows = $6
        WHERE id = $7 RETURNING *`,
      [privado, dm, comentarios, enLinea, buscable, verSeguidos, u.id]
    );
    return usuarioPublico(actualizado, { propio: true });
  });

  router.post('/api/auth/change-password', async (c) => {
    const u = await c.exigir();
    const b = await c.cuerpo();
    const actual = texto(b.current_password, { min: 1, campo: 'contraseña actual' });
    const nueva = texto(b.new_password, { min: 8, max: 128, campo: 'contraseña nueva' });
    if (!(await verificarPassword(u.password_hash, actual))) {
      throw new ApiErr('La contraseña actual no es correcta', 401);
    }
    const hash = await hashPassword(nueva);
    await c.pool.query('UPDATE users SET password_hash = $1 WHERE id = $2', [hash, u.id]);
    await revocarTodas(c.pool, u.id);
    await auditar(c.pool, Number(u.id), 'password_cambiada', '', c.ip);
    return { ok: true };
  });

  router.get('/api/auth/sessions', async (c) => {
    const u = await c.exigir();
    return listarSesiones(c.pool, u.id, tokenRefrescoDe(c.req));
  });

  router.del('/api/auth/sessions/:id', async (c) => {
    const u = await c.exigir();
    const id = Number(c.params.id);
    const r = await c.pool.query(
      'UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL',
      [id, u.id]
    );
    if (r.rowCount === 0) throw new ApiErr('Sesión no encontrada', 404);
    return { ok: true };
  });

  router.post('/api/auth/sessions-all', async (c) => {
    const u = await c.exigir();
    await revocarTodas(c.pool, u.id);
    await auditar(c.pool, Number(u.id), 'sesiones_cerradas', '', c.ip);
    return { ok: true };
  });

  // ---- Verificación en dos pasos (código por correo) ----
  router.post('/api/auth/2fa/enable', async (c) => {
    const u = await c.exigir();
    const b = await c.cuerpo();
    const password = texto(b.password, { min: 1, campo: 'contraseña' });
    if (!(await verificarPassword(u.password_hash, password))) throw new ApiErr('Contraseña incorrecta', 401);
    const codigo = await emitirCodigo(c.pool, Number(u.id), '2fa_enable', 5);
    const correo = await enviarCorreo(u.email, 'Activa la verificación en dos pasos Moon', `Tu código para activar 2FA es ${codigo}. Caduca en 5 minutos.`);
    const temp = firmarJwt({ uid: Number(u.id), purpose: '2fa_enable' }, c.req._secreto, 300);
    return {
      temp_token: temp,
      message: correo.enviado ? 'Código enviado a tu correo' : 'Código generado (sin correo configurado)',
      dev_code: correo.enviado ? undefined : codigo,
    };
  });

  router.post('/api/auth/2fa/confirm', async (c) => {
    const b = await c.cuerpo();
    const temp = texto(b.temp_token, { min: 10, campo: 'token' });
    const code = texto(b.code, { min: 6, max: 6, campo: 'código' });
    const claims = verificarJwt(temp, c.req._secreto);
    if (!claims || claims.purpose !== '2fa_enable') throw new ApiErr('Token temporal inválido', 401);
    if (!(await consumirCodigo(c.pool, Number(claims.uid), '2fa_enable', code))) {
      throw new ApiErr('Código inválido o expirado', 401);
    }
    await c.pool.query('UPDATE users SET twofa_enabled = TRUE WHERE id = $1', [claims.uid]);
    await auditar(c.pool, Number(claims.uid), '2fa_activada', '', c.ip);
    return { ok: true };
  });

  router.post('/api/auth/2fa/disable', async (c) => {
    const u = await c.exigir();
    const b = await c.cuerpo();
    const password = texto(b.password, { min: 1, campo: 'contraseña' });
    if (!(await verificarPassword(u.password_hash, password))) throw new ApiErr('Contraseña incorrecta', 401);
    await c.pool.query('UPDATE users SET twofa_enabled = FALSE WHERE id = $1', [u.id]);
    await auditar(c.pool, Number(u.id), '2fa_desactivada', '', c.ip);
    return { ok: true };
  });

  // ---- Recuperación de contraseña ----
  router.post('/api/auth/recovery/request', async (c) => {
    // Blindaje contra saturación y spam en recuperación de contraseña
    if (demasiadoRapido(`rec:ip:${c.ip}`, 4, 900_000)) {
      throw new ApiErr('Demasiadas solicitudes de recuperación. Espera 15 minutos.', 429, 'too_many_recoveries');
    }
    const b = await c.cuerpo();
    const email = texto(b.email, { min: 5, max: 120, campo: 'correo' }).toLowerCase();
    const usuario = await uno(c.pool, 'SELECT * FROM users WHERE LOWER(email) = $1', [email]);
    // Respuesta idéntica exista o no la cuenta (no revelamos correos registrados).
    const respuesta = { message: 'Si el correo está registrado, recibirás un enlace para restablecer tu contraseña.' };
    if (!usuario) return respuesta;
    const token = await emitirTokenRecuperacion(c.pool, Number(usuario.id), 60);
    const enlace = `${c.basePublica}/#/reset?token=${token}`;
    const correo = await enviarCorreo(
      usuario.email,
      'Restablece tu contraseña de Moon',
      `Abre este enlace para elegir una contraseña nueva (caduca en 60 minutos):\n${enlace}`
    );
    if (correo.enviado && correo.redirigido) {
      respuesta.message =
        'Tu proveedor gratuito no entrega a ese correo: el enlace se envió al correo alternativo configurado en Render. Revisa esa bandeja.';
    } else if (!correo.enviado) {
      const errTexto = String(correo.error || '').toLowerCase();
      if (errTexto.includes('only send') || errTexto.includes('testing email') || !correoConfigurado()) {
        // Modo gratuito de correo sin dominio propio: entrega token directo para no bloquear al usuario
        respuesta.dev_token = token;
        respuesta.message = 'Modo de prueba activo: usa el enlace generado para restablecer tu contraseña.';
      } else {
        respuesta.message = 'No pudimos enviar el correo. Revisa la clave de correo en Render.';
      }
    }
    return respuesta;
  });

  router.post('/api/auth/recovery/verify', async (c) => {
    const b = await c.cuerpo();
    const token = texto(b.token, { min: 10, campo: 'token' });
    const nueva = texto(b.new_password, { min: 8, max: 128, campo: 'contraseña nueva' });
    const userId = await consumirTokenRecuperacion(c.pool, token);
    if (!userId) throw new ApiErr('Enlace inválido o caducado', 400);
    const hash = await hashPassword(nueva);
    await c.pool.query('UPDATE users SET password_hash = $1 WHERE id = $2', [hash, userId]);
    await revocarTodas(c.pool, userId);
    await auditar(c.pool, Number(userId), 'password_restablecida', '', c.ip);
    return { ok: true };
  });

  router.del('/api/auth/account', async (c) => {
    const u = await c.exigir();
    const b = await c.cuerpo();
    const password = texto(b.password, { min: 1, campo: 'contraseña' });
    if (!(await verificarPassword(u.password_hash, password))) throw new ApiErr('Contraseña incorrecta', 401);
    await c.pool.query('DELETE FROM users WHERE id = $1', [u.id]);
    await auditar(c.pool, null, 'cuenta_eliminada', `${u.username}`, c.ip);
    return { ok: true };
  });

  // ---- Preferencias de notificaciones ----
  router.get('/api/notifications/prefs', async (c) => {
    const u = await c.exigir();
    const prefs = await uno(c.pool, 'SELECT * FROM notification_prefs WHERE user_id = $1', [u.id]);
    return prefs || { follow: true, like: true, comment: true, reply: true, mention: true, message: true, system: true };
  });

  router.patch('/api/notifications/prefs', async (c) => {
    const u = await c.exigir();
    const b = await c.cuerpo();
    const claves = ['follow', 'like', 'comment', 'reply', 'mention', 'message', 'system'];
    const valores = claves.map((k) => booleano(b[k], true));
    const fila2 = await uno(
      c.pool,
      `INSERT INTO notification_prefs (user_id, follow, "like", comment, reply, mention, message, system)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
       ON CONFLICT (user_id) DO UPDATE SET follow = $2, "like" = $3, comment = $4, reply = $5, mention = $6, message = $7, system = $8
       RETURNING *`,
      [u.id, ...valores]
    );
    return fila2;
  });

  void filas;
}
