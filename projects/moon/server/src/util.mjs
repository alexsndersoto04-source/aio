// Moon — Utilidades del servidor
// ============================================================
// Respuestas JSON, lectura del cuerpo, paginación y validaciones básicas.

export class ApiErr extends Error {
  constructor(mensaje, status = 400, code = '') {
    super(mensaje);
    this.status = status;
    this.code = code;
  }
}

export function json(res, status, data) {
  const texto = JSON.stringify(data === undefined ? null : data);
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Cache-Control': 'no-store',
    'X-Content-Type-Options': 'nosniff',
  });
  res.end(texto);
}

export function error(res, status, mensaje, code) {
  json(res, status, code ? { error: mensaje, code } : { error: mensaje });
}

export function noContent(res) {
  res.writeHead(204).end();
}

// Lee el cuerpo JSON (con límite de tamaño para evitar abusos).
export async function cuerpo(req, limite = 1024 * 1024) {
  const trozos = [];
  let total = 0;
  for await (const t of req) {
    total += t.length;
    if (total > limite) throw new ApiErr('Cuerpo demasiado grande', 413);
    trozos.push(t);
  }
  if (total === 0) return {};
  const texto = Buffer.concat(trozos).toString('utf8');
  try {
    const valor = JSON.parse(texto);
    return valor && typeof valor === 'object' ? valor : {};
  } catch {
    throw new ApiErr('JSON inválido', 400);
  }
}

export function paginacion(req, limitePorDefecto = 20, maximo = 50) {
  const url = new URL(req.url, 'http://x');
  const page = Math.max(1, Number(url.searchParams.get('page') || 1) || 1);
  const limite = Math.min(maximo, Math.max(1, Number(url.searchParams.get('limit') || limitePorDefecto) || limitePorDefecto));
  return { page, limit: limite, offset: (page - 1) * limite };
}

export function qs(req, nombre, porDefecto = '') {
  const url = new URL(req.url, 'http://x');
  return url.searchParams.get(nombre) ?? porDefecto;
}

export function ipDe(req) {
  const reenviada = req.headers['x-forwarded-for'];
  if (reenviada) return String(reenviada).split(',')[0].trim();
  return req.socket.remoteAddress || '';
}

export function texto(valor, { min = 0, max = 10000, campo = 'campo' } = {}) {
  if (typeof valor !== 'string') throw new ApiErr(`Falta ${campo}`, 400);
  const limpio = valor.trim();
  if (limpio.length < min) throw new ApiErr(`${campo} es demasiado corto`, 400);
  if (limpio.length > max) throw new ApiErr(`${campo} es demasiado largo`, 400);
  return limpio;
}

export function booleano(valor, porDefecto = false) {
  if (typeof valor === 'boolean') return valor;
  if (valor === 'true' || valor === 1 || valor === '1') return true;
  if (valor === 'false' || valor === 0 || valor === '0') return false;
  return porDefecto;
}

// Nombres de usuario y correo con reglas explícitas.
export function usuarioValido(u) {
  return /^[a-zA-Z0-9_]{3,24}$/.test(u);
}

export function correoValido(e) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(e);
}

export function hashtagsDe(contenido) {
  const vistos = new Set();
  for (const m of contenido.matchAll(/#([\p{L}\p{N}_]{2,40})/gu)) {
    vistos.add(m[1].toLowerCase());
    if (vistos.size >= 10) break;
  }
  return [...vistos];
}

export function mencionesDe(contenido) {
  const vistos = new Set();
  for (const m of contenido.matchAll(/@([a-zA-Z0-9_]{3,24})/g)) {
    vistos.add(m[1].toLowerCase());
    if (vistos.size >= 10) break;
  }
  return [...vistos];
}
