// Moon — Cliente API real
// ============================================================
// - Access token (JWT 15 min) en memoria + localStorage persistente
// - Refresh automático con rotación y cola (una sola petición a la vez)
// - Errores tipificados (ApiError con status/code), 401 -> reintento único
// - CERO simulaciones: todas las llamadas van al backend real.

export const API_URL = (import.meta.env.VITE_API_URL || '').replace(/\/$/, '') || '';

// URL base para WebSocket (deriva del API_URL)
export function wsUrl() {
  const base = API_URL || window.location.origin;
  const wsProto = base.startsWith('https') ? 'wss' : 'ws';
  return `${wsProto}://${base.replace(/^https?:\/\//, '')}/ws`;
}

export class ApiError extends Error {
  constructor(message, status, code) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
  }
}

const ACCESS_KEY = 'moon_access_token';
const REFRESH_KEY = 'moon_refresh_token';
const USER_KEY = 'moon_user';

let accessToken = localStorage.getItem(ACCESS_KEY);
let refreshPromise = null;

export function getAccessToken() { return accessToken; }

export function saveTokens(access, refresh) {
  accessToken = access || null;
  if (refresh) localStorage.setItem(REFRESH_KEY, refresh);
  if (access) localStorage.setItem(ACCESS_KEY, access);
  else localStorage.removeItem(ACCESS_KEY);
}

export function getRefreshToken() { return localStorage.getItem(REFRESH_KEY); }

export function clearTokens() {
  accessToken = null;
  localStorage.removeItem(ACCESS_KEY);
  localStorage.removeItem(REFRESH_KEY);
  localStorage.removeItem(USER_KEY);
}

export function saveUser(user) { localStorage.setItem(USER_KEY, JSON.stringify(user)); }
export function getUser() {
  try { return JSON.parse(localStorage.getItem(USER_KEY) || 'null'); } catch { return null; }
}

// Refresca el token (rotación). Una sola petición concurrente.
async function refreshAccess() {
  if (!refreshPromise) {
    refreshPromise = (async () => {
      const refresh = getRefreshToken();
      if (!refresh) throw new ApiError('Sesión expirada', 401, 'no_session');
      const res = await fetch(`${API_URL}/api/auth/refresh`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'x-refresh-token': refresh },
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        clearTokens();
        throw new ApiError(data.error || 'Sesión expirada', res.status, data.code);
      }
      saveTokens(data.access_token, data.refresh_token);
      saveUser(data.user);
      return data.access_token;
    })().finally(() => { refreshPromise = null; });
  }
  return refreshPromise;
}

async function raw(method, path, { body, headers = {}, retried = false } = {}) {
  const url = `${API_URL}${path}`;
  const h = { ...headers };
  if (body !== undefined && !(body instanceof FormData)) h['Content-Type'] = 'application/json';
  if (accessToken) h['Authorization'] = `Bearer ${accessToken}`;
  const init = { method, headers: h };
  if (body !== undefined) init.body = body instanceof FormData ? body : JSON.stringify(body);

  let res;
  try {
    res = await fetch(url, init);
  } catch {
    throw new ApiError('No se pudo conectar con el servidor', 0, 'network');
  }

  // 401 con token presente -> intenta refresh una sola vez
  if (res.status === 401 && accessToken && !retried) {
    try {
      await refreshAccess();
      return raw(method, path, { body, headers, retried: true });
    } catch (e) {
      throw e instanceof ApiError ? e : new ApiError('Sesión expirada', 401, 'no_session');
    }
  }

  if (res.status === 204) return null;
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    const err = new ApiError(data.error || `Error ${res.status}`, res.status, data.code);
    throw err;
  }
  return data;
}

export const api = {
  get: (p, o) => raw('GET', p, o),
  post: (p, body, o) => raw('POST', p, { body, ...o }),
  patch: (p, body, o) => raw('PATCH', p, { body, ...o }),
  put: (p, body, o) => raw('PUT', p, { body, ...o }),
  del: (p, o) => raw('DELETE', p, o),
};

// ---- Endpoints de autenticación (sin token) ----
export const authApi = {
  register: (username, email, password) =>
    api.post('/api/auth/register', { username, email, password }),
  login: (username, password) =>
    api.post('/api/auth/login', { username, password }),
  verify2fa: (temp_token, code) =>
    api.post('/api/auth/2fa/verify', { temp_token, code }),
  logout: () =>
    api.post('/api/auth/logout', {}, { headers: { 'x-refresh-token': getRefreshToken() || '' } }),
  recoveryRequest: (email) => api.post('/api/auth/recovery/request', { email }),
  recoveryVerify: (token, new_password) =>
    api.post('/api/auth/recovery/verify', { token, new_password }),
};

// ---- Media ----
export function uploadMedia(kind, file, onProgress) {
  const form = new FormData();
  form.append('name', kind);
  form.append('file', file);
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const url = `${API_URL}/api/upload`;
    xhr.open('POST', url);
    if (accessToken) xhr.setRequestHeader('Authorization', `Bearer ${accessToken}`);
    xhr.upload.onprogress = (e) => {
      if (onProgress && e.lengthComputable) onProgress(Math.round((e.loaded / e.total) * 100));
    };
    xhr.onload = () => {
      try {
        const data = JSON.parse(xhr.responseText || '{}');
        if (xhr.status >= 200 && xhr.status < 300) resolve(data);
        else reject(new ApiError(data.error || 'Subida fallida', xhr.status, data.code));
      } catch {
        reject(new ApiError('Subida fallida', xhr.status));
      }
    };
    xhr.onerror = () => reject(new ApiError('No se pudo conectar con el servidor', 0, 'network'));
    xhr.send(form);
  });
}
