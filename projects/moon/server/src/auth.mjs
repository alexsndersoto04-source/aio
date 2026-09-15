// Moon — Autenticación
// ============================================================
// Contraseñas con Argon2id, tokens JWT HS256 (access de 15 min + refresh con
// rotación guardado como hash), códigos de verificación de un solo uso y
// sesiones por dispositivo. Mismo comportamiento que el servidor Titan.

import { createHmac, randomBytes, randomInt, createHash, timingSafeEqual } from 'node:crypto';
import { hash as argonHash, verify as argonVerify } from '@node-rs/argon2';
import { ApiErr } from './util.mjs';
import { fila, filas, uno } from './db.mjs';

export const ACCESO_MIN = 15;
export const REFRESCO_DIAS = 30;

// ---------- Contraseñas ----------

export function hashPassword(plano) {
  return argonHash(plano, { memoryCost: 19456, timeCost: 2, parallelism: 1 });
}

export async function verificarPassword(hashGuardado, plano) {
  if (!hashGuardado) return false;
  try {
    return await argonVerify(hashGuardado, plano);
  } catch {
    return false;
  }
}

// ---------- JWT (HS256, sin dependencias) ----------

const b64url = (buf) => Buffer.from(buf).toString('base64url');

function firmarCrudo(datos, secreto) {
  return createHmac('sha256', secreto).update(datos).digest('base64url');
}

export function firmarJwt(claims, secreto, segundos) {
  const cabecera = b64url(JSON.stringify({ alg: 'HS256', typ: 'JWT' }));
  const ahora = Math.floor(Date.now() / 1000);
  const cuerpo = b64url(
    JSON.stringify({ ...claims, iat: ahora, exp: ahora + segundos, iss: 'moon', sub: String(claims.uid) })
  );
  const datos = `${cabecera}.${cuerpo}`;
  return `${datos}.${firmarCrudo(datos, secreto)}`;
}

export function verificarJwt(token, secreto) {
  if (typeof token !== 'string') return null;
  const partes = token.split('.');
  if (partes.length !== 3) return null;
  const [cabecera, cuerpo, firma] = partes;
  const esperada = firmarCrudo(`${cabecera}.${cuerpo}`, secreto);
  const a = Buffer.from(firma);
  const b = Buffer.from(esperada);
  if (a.length !== b.length || !timingSafeEqual(a, b)) return null;
  let claims;
  try {
    claims = JSON.parse(Buffer.from(cuerpo, 'base64url').toString('utf8'));
  } catch {
    return null;
  }
  if (!claims || typeof claims.exp !== 'number' || claims.exp * 1000 < Date.now()) return null;
  return claims;
}

// ---------- Tokens de sesión (refresh) ----------

const sha256 = (v) => createHash('sha256').update(v).digest('hex');

export async function crearSesion(pool, userId, req, device = '') {
  const bruto = randomBytes(32).toString('hex');
  const ua = String(req.headers['user-agent'] || '').slice(0, 300);
  const ip = String(req.headers['x-forwarded-for'] || req.socket.remoteAddress || '').slice(0, 80);
  await pool.query(
    `INSERT INTO refresh_tokens (user_id, token_hash, device, user_agent, ip, expires_at)
     VALUES ($1, $2, $3, $4, $5, NOW() + ($6 || ' days')::interval)`,
    [userId, sha256(bruto), device.slice(0, 80), ua, ip, String(REFRESCO_DIAS)]
  );
  return bruto;
}

export async function refrescarSesion(pool, tokenBruto, req) {
  const actual = await fila(
    pool,
    `SELECT id, user_id FROM refresh_tokens
      WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > NOW()`,
    [sha256(tokenBruto)]
  );
  if (!actual) throw new ApiErr('Sesión expirada', 401, 'no_session');
  // Rotación: se revoca el token usado y se emite uno nuevo.
  await pool.query('UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1', [actual.id]);
  const nuevo = await crearSesion(pool, actual.user_id, req);
  return { userId: actual.user_id, refresh: nuevo };
}

export async function revocarSesion(pool, tokenBruto) {
  if (!tokenBruto) return;
  await pool.query('UPDATE refresh_tokens SET revoked_at = NOW() WHERE token_hash = $1', [sha256(tokenBruto)]);
}

export const listarSesiones = (pool, userId, tokenActual) =>
  filas(
    pool,
    `SELECT id, device, user_agent, ip, created_at::text AS created_at,
            last_used_at::text AS last_used_at, token_hash
       FROM refresh_tokens
      WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW()
      ORDER BY created_at DESC`,
    [userId]
  ).then((rs) =>
    rs.map((r) => ({
      id: r.id,
      device: r.device || 'Dispositivo',
      user_agent: r.user_agent,
      ip: r.ip,
      created_at: r.created_at,
      last_used_at: r.last_used_at,
      current: tokenActual ? r.token_hash === sha256(tokenActual) : false,
    }))
  );

export const revocarTodas = (pool, userId) =>
  pool.query('UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL', [userId]);

// ---------- Códigos de un solo uso (2FA y recuperación) ----------

export async function emitirCodigo(pool, userId, tipo, minutos, { hash = true } = {}) {
  const codigo = String(randomInt(0, 1_000_000)).padStart(6, '0');
  const guardado = hash ? sha256(codigo) : codigo;
  await pool.query(
    `INSERT INTO recovery_tokens (user_id, token_hash, kind, expires_at)
     VALUES ($1, $2, $3, NOW() + ($4 || ' minutes')::interval)`,
    [userId, guardado, tipo, String(minutos)]
  );
  return codigo;
}

export async function consumirCodigo(pool, userId, tipo, codigo) {
  const filaToken = await fila(
    pool,
    `SELECT id, token_hash FROM recovery_tokens
      WHERE user_id = $1 AND kind = $2 AND used_at IS NULL AND expires_at > NOW()
      ORDER BY id DESC LIMIT 1`,
    [userId, tipo]
  );
  if (!filaToken) return false;
  const candidatos = [sha256(String(codigo)), String(codigo)];
  if (!candidatos.includes(filaToken.token_hash)) return false;
  await pool.query('UPDATE recovery_tokens SET used_at = NOW() WHERE id = $1', [filaToken.id]);
  return true;
}

export async function emitirTokenRecuperacion(pool, userId, minutos = 60) {
  const bruto = randomBytes(24).toString('hex');
  await pool.query(
    `INSERT INTO recovery_tokens (user_id, token_hash, kind, expires_at)
     VALUES ($1, $2, 'recovery', NOW() + ($3 || ' minutes')::interval)`,
    [userId, sha256(bruto), String(minutos)]
  );
  return bruto;
}

export async function consumirTokenRecuperacion(pool, tokenBruto) {
  const t = await fila(
    pool,
    `SELECT id, user_id FROM recovery_tokens
      WHERE token_hash = $1 AND kind = 'recovery' AND used_at IS NULL AND expires_at > NOW()`,
    [sha256(tokenBruto)]
  );
  if (!t) return null;
  await pool.query('UPDATE recovery_tokens SET used_at = NOW() WHERE id = $1', [t.id]);
  return t.user_id;
}

// ---------- Sesión actual en una petición ----------

export function tokenDe(req) {
  const cabecera = req.headers.authorization || '';
  return cabecera.startsWith('Bearer ') ? cabecera.slice(7).trim() : '';
}

export function tokenRefrescoDe(req) {
  return String(req.headers['x-refresh-token'] || '');
}

export function claimsDe(req, secreto) {
  return verificarJwt(tokenDe(req), secreto);
}

export async function usuarioActual(pool, req, secreto) {
  const claims = claimsDe(req, secreto);
  if (!claims || !claims.uid) return null;
  const u = await uno(pool, 'SELECT * FROM users WHERE id = $1', [claims.uid]);
  if (!u || u.status === 'suspended') return null;
  return u;
}

// ---------- Formas públicas ----------

export function usuarioPublico(u, { propio = false, tokenRefresco = '' } = {}) {
  const base = {
    id: Number(u.id),
    username: u.username,
    display_name: u.display_name || u.username,
    role: u.role,
    avatar_url: u.avatar_url,
    cover_url: u.cover_url,
    bio: u.bio,
    link: u.link,
    location: u.location,
    is_verified: u.is_verified,
    is_private: u.is_private,
    dm_privacy: u.dm_privacy,
    who_can_comment: u.who_can_comment || 'all',
    show_online: u.show_online !== false,
    searchable: u.searchable !== false,
    who_can_see_follows: u.who_can_see_follows !== false,
    twofa_enabled: u.twofa_enabled,
    followers_count: Number(u.followers_count),
    following_count: Number(u.following_count),
    posts_count: Number(u.posts_count),
    created_at: u.created_at ? String(u.created_at) : null,
  };
  if (propio) {
    base.email = u.email;
    base.status = u.status;
  }
  void tokenRefresco;
  return base;
}
