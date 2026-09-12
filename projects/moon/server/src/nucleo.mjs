// Moon — Núcleo HTTP
// ============================================================
// Enrutador mínimo (método + patrón con parámetros), contexto de petición,
// CORS y manejo uniforme de errores.

import { ApiErr, json, error, noContent, cuerpo as leerCuerpo } from './util.mjs';
import { usuarioActual } from './auth.mjs';

export function crearRouter() {
  const rutas = [];

  function add(metodo, patron, handler) {
    const partes = patron.split('/').filter(Boolean);
    rutas.push({ metodo, patron, partes, handler });
  }

  function buscar(metodo, camino) {
    const partes = camino.split('/').filter(Boolean);
    for (const r of rutas) {
      if (r.metodo !== metodo) continue;
      if (r.partes.length !== partes.length) continue;
      const params = {};
      let coincide = true;
      for (let i = 0; i < r.partes.length; i += 1) {
        const p = r.partes[i];
        if (p.startsWith(':')) params[p.slice(1)] = decodeURIComponent(partes[i]);
        else if (p !== partes[i]) { coincide = false; break; }
      }
      if (coincide) return { ruta: r, params };
    }
    return null;
  }

  return {
    add,
    get: (p, h) => add('GET', p, h),
    post: (p, h) => add('POST', p, h),
    patch: (p, h) => add('PATCH', p, h),
    put: (p, h) => add('PUT', p, h),
    del: (p, h) => add('DELETE', p, h),
    buscar,
    rutas,
  };
}

export function crearContexto({ req, res, pool, secreto, basePublica = '' }) {
  const url = new URL(req.url, 'http://x');
  const c = {
    req,
    res,
    pool,
    secreto,
    // Dirección pública del frontend (para enlaces en correos).
    basePublica: basePublica || `${req.headers['x-forwarded-proto'] || 'http'}://${req.headers.host || 'localhost'}`,
    url,
    params: {},
    get query() { return url.searchParams; },
    get ip() {
      return String(req.headers['x-forwarded-for'] || req.socket.remoteAddress || '').split(',')[0].trim();
    },
    _usuario: undefined,
    _cuerpo: undefined,
    async usuario() {
      if (c._usuario === undefined) c._usuario = await usuarioActual(pool, req, secreto);
      return c._usuario;
    },
    async exigir() {
      const u = await c.usuario();
      if (!u) throw new ApiErr('Sesión no válida', 401, 'unauthorized');
      return u;
    },
    async admin() {
      const u = await c.exigir();
      if (u.role !== 'admin') throw new ApiErr('Solo para administración', 403, 'forbidden');
      return u;
    },
    async cuerpo() {
      if (c._cuerpo === undefined) c._cuerpo = await leerCuerpo(req);
      return c._cuerpo;
    },
  };
  return c;
}

export function cors(origins) {
  return (req, res) => {
    const origen = req.headers.origin || '';
    const permitido =
      origins.includes('*') || origins.includes(origen) ? origen || '*' : origins[0] || '*';
    res.setHeader('Access-Control-Allow-Origin', permitido);
    res.setHeader('Access-Control-Allow-Credentials', 'true');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization, X-Refresh-Token');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PATCH, PUT, DELETE, OPTIONS');
    res.setHeader('Vary', 'Origin');
  };
}

export function manejadorErrores(res) {
  return (e) => {
    if (res.writableEnded) return;
    if (e instanceof ApiErr) {
      error(res, e.status, e.message, e.code);
      return;
    }
    console.error('[api] error inesperado:', e);
    error(res, 500, 'Error interno del servidor');
  };
}

export { json, noContent, ApiErr };
