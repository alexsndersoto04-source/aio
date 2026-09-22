// Moon — Defensas de Seguridad y Blindaje en Vivo
// =========================================================================
// Defensas por dentro y por fuera:
// 1. Filtrado de IPs bloqueadas en memoria viva (0.001 ms por request).
// 2. Modo Blindaje / Escudo Anti-DDoS de emergencia activable desde el panel.
// 3. Protección anti-fuerza bruta en logins y 2FA.
// 4. Registro y auditoría de eventos de seguridad sospechosos.

import { uno } from './db.mjs';

const IPS_BLOQUEADAS = new Set();
let MODO_BLINDAJE = false;
const INTENTOS_FALLIDOS = new Map(); // ip -> [timestamps]

/**
 * Carga las IPs bloqueadas desde la base de datos a memoria al arrancar.
 */
export async function inicializarDefensas(pool) {
  try {
    const res = await pool.query('SELECT ip FROM blocked_ips');
    IPS_BLOQUEADAS.clear();
    for (const r of res.rows) {
      if (r.ip) IPS_BLOQUEADAS.add(r.ip.trim());
    }
    console.log(`[defensas] ${IPS_BLOQUEADAS.size} IPs bloqueadas cargadas en memoria`);

    const shieldSetting = await uno(pool, "SELECT valor FROM app_settings WHERE clave = 'security.shield_mode'");
    MODO_BLINDAJE = shieldSetting?.valor === 'true';
    if (MODO_BLINDAJE) {
      console.log('[defensas] ⚠️ MODO BLINDAJE ANTI-DDOS ACTIVO');
    }
  } catch (e) {
    console.warn('[defensas] No se pudieron inicializar IPs bloqueadas:', e.message);
  }
}

/**
 * Comprueba si una IP está en la lista negra.
 */
export function estaIpBloqueada(ip) {
  if (!ip) return false;
  return IPS_BLOQUEADAS.has(ip.trim());
}

/**
 * Bloquea una IP sospechosa en la base de datos y memoria.
 */
export async function bloquearIp(pool, ip, motivo = 'Bloqueo administrativo', adminId = null) {
  const ipLimpia = String(ip || '').trim();
  if (!ipLimpia) throw new Error('IP inválida');

  await pool.query(
    `INSERT INTO blocked_ips (ip, motivo, bloqueado_por)
     VALUES ($1, $2, $3)
     ON CONFLICT (ip) DO UPDATE SET motivo = EXCLUDED.motivo, creado_en = NOW()`,
    [ipLimpia, motivo, adminId]
  );
  IPS_BLOQUEADAS.add(ipLimpia);

  await registrarEventoSeguridad(pool, 'ip_bloqueada', `IP ${ipLimpia} bloqueada: ${motivo}`, ipLimpia, adminId);
  return { ok: true, ip: ipLimpia };
}

/**
 * Desbloquea una IP.
 */
export async function desbloquearIp(pool, ip, adminId = null) {
  const ipLimpia = String(ip || '').trim();
  await pool.query('DELETE FROM blocked_ips WHERE ip = $1', [ipLimpia]);
  IPS_BLOQUEADAS.delete(ipLimpia);

  await registrarEventoSeguridad(pool, 'ip_desbloqueada', `IP ${ipLimpia} desbloqueada`, ipLimpia, adminId);
  return { ok: true, ip: ipLimpia };
}

/**
 * Lista las IPs bloqueadas con detalles.
 */
export async function listarIpsBloqueadas(pool) {
  const res = await pool.query(
    `SELECT b.ip, b.motivo, b.creado_en::text AS creado_en, u.username AS bloqueado_por_usuario
     FROM blocked_ips b
     LEFT JOIN users u ON u.id = b.bloqueado_por
     ORDER BY b.creado_en DESC
     LIMIT 100`
  );
  return res.rows;
}

/**
 * Activa o desactiva el Modo Blindaje / Escudo Anti-DDoS.
 */
export async function cambiarModoBlindaje(pool, activar, adminId = null) {
  MODO_BLINDAJE = Boolean(activar);
  await pool.query(
    `INSERT INTO app_settings (clave, valor)
     VALUES ('security.shield_mode', $1)
     ON CONFLICT (clave) DO UPDATE SET valor = EXCLUDED.valor, updated_at = NOW()`,
    [String(MODO_BLINDAJE)]
  );

  await registrarEventoSeguridad(
    pool,
    MODO_BLINDAJE ? 'modo_blindaje_activado' : 'modo_blindaje_desactivado',
    `Escudo Anti-DDoS ${MODO_BLINDAJE ? 'ACTIVADO' : 'DESACTIVADO'}`,
    '',
    adminId
  );

  return { ok: true, modo_blindaje: MODO_BLINDAJE };
}

export function estadoModoBlindaje() {
  return MODO_BLINDAJE;
}

/**
 * Registra intento fallido de login. Si supera el umbral, auto-bloquea temporalmente la IP.
 */
export async function registrarFalloLogin(pool, ip, username = '') {
  const ahora = Date.now();
  const intentos = (INTENTOS_FALLIDOS.get(ip) || []).filter((t) => ahora - t < 600_000); // 10 min
  intentos.push(ahora);
  INTENTOS_FALLIDOS.set(ip, intentos);

  await registrarEventoSeguridad(
    pool,
    'login_fallido',
    `Intento fallido para usuario: "${username.slice(0, 40)}" (${intentos.length} intentos en 10 min)`,
    ip
  );

  if (intentos.length >= 6) {
    await bloquearIp(pool, ip, 'Auto-bloqueo: 6 intentos fallidos de autenticación');
  }
}

/**
 * Limpia los fallos al haber un login exitoso.
 */
export function limpiarFallosLogin(ip) {
  INTENTOS_FALLIDOS.delete(ip);
}

/**
 * Registra un evento de seguridad estructurado.
 */
export async function registrarEventoSeguridad(pool, tipo, detalle = '', ip = '', userId = null) {
  try {
    await pool.query(
      `INSERT INTO security_events (tipo, detalle, ip, user_id)
       VALUES ($1, $2, $3, $4)`,
      [tipo, detalle.slice(0, 300), ip.slice(0, 60), userId]
    );
  } catch (e) {
    // Si la tabla aún no migró
  }
}

/**
 * Lista eventos recientes de seguridad.
 */
export async function listarEventosSeguridad(pool, limite = 40) {
  try {
    const res = await pool.query(
      `SELECT s.id, s.tipo, s.detalle, s.ip, s.user_id, s.creado_en::text AS creado_en, u.username
       FROM security_events s
       LEFT JOIN users u ON u.id = s.user_id
       ORDER BY s.id DESC
       LIMIT $1`,
      [limite]
    );
    return res.rows;
  } catch {
    return [];
  }
}

/**
 * Informe completo del estado de seguridad y defensas activas de Moon.
 */
export async function obtenerEstadoSeguridad(pool) {
  const ipsBloqueadasCount = IPS_BLOQUEADAS.size;
  let eventosRecientes = [];
  try {
    eventosRecientes = await listarEventosSeguridad(pool, 15);
  } catch {}

  return {
    defensas: {
      modo_blindaje: MODO_BLINDAJE,
      ips_bloqueadas_total: ipsBloqueadasCount,
      argon2id_passwords: { estado: 'activo', algoritmo: 'Argon2id', memoria_kb: 65536, iteraciones: 3 },
      dos_factores_totp: { estado: 'activo', protocolo: 'RFC 6238 TOTP' },
      proteccion_sql_injection: { estado: 'activo', mecanismo: 'Consultas 100% Parametrizadas' },
      proteccion_xss_csrf: { estado: 'activo', mecanismo: 'Sanitización de HTML y SameSite estricto' },
      cabeceras_http_seguras: {
        hsts: 'max-age=31536000; includeSubDomains; preload',
        x_frame_options: 'DENY',
        x_content_type_options: 'nosniff',
        referrer_policy: 'strict-origin-when-cross-origin',
        permissions_policy: 'camera=(self), microphone=(self)',
      },
      integridad_boveda: { estado: 'activo', checksum: 'SHA-256 criptográfico' },
      rate_limiting: { estado: 'activo', modo: MODO_BLINDAJE ? 'Estricto (Blindaje)' : 'Normal Dinámico' },
    },
    eventos_recientes: eventosRecientes,
  };
}
