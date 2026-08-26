// Moon — Contexto de autenticación
// ============================================================
// Estado global de sesión con persistencia, login (incluye 2FA),
// registro, cierre de sesión y conexión del WebSocket real.

import React, { createContext, useContext, useEffect, useState, useCallback } from 'react';
import {
  api, authApi, saveTokens, clearTokens, saveUser, getUser,
  getAccessToken, getRefreshToken, refreshAccess,
} from './api.js';
import { realtime } from './realtime.js';

const AuthContext = createContext(null);

export function AuthProvider({ children }) {
  const [user, setUser] = useState(() => getUser());
  const [loading, setLoading] = useState(true);

  const applySession = useCallback((resp) => {
    saveTokens(resp.access_token, resp.refresh_token);
    if (resp.user) {
      saveUser(resp.user);
      setUser(resp.user);
    }
    realtime.start();
  }, []);

  const login = useCallback(async (username, password) => {
    const resp = await authApi.login(username, password);
    if (resp.twofa_required) {
      // Necesita código por email: devuelve el paso intermedio.
      return { twofa: true, temp_token: resp.temp_token };
    }
    applySession(resp);
    return { twofa: false };
  }, [applySession]);

  const verify2fa = useCallback(async (temp_token, code) => {
    const resp = await authApi.verify2fa(temp_token, code);
    applySession(resp);
  }, [applySession]);

  const register = useCallback(async (username, email, password) => {
    const resp = await authApi.register(username, email, password);
    applySession(resp);
  }, [applySession]);

  const logout = useCallback(async () => {
    try { await authApi.logout(); } catch { /* revocar ya es best-effort */ }
    clearTokens();
    setUser(null);
    realtime.stop();
  }, []);

  const refreshMe = useCallback(async () => {
    try {
      const me = await api.get('/api/auth/me');
      saveUser(me);
      setUser(me);
    } catch { /* sesión inválida la maneja api.js */ }
  }, []);

  // Al arrancar: si hay refresh token, intenta restaurar sesión
  // (aunque el access token haya expirado o falte).
  useEffect(() => {
    let alive = true;
    (async () => {
      if (!getRefreshToken()) { setLoading(false); return; }
      try {
        if (!getAccessToken()) {
          // Access token ausente/expirado: rotar con el refresh primero.
          await refreshAccess();
        }
        const me = await api.get('/api/auth/me');
        if (!alive) return;
        saveUser(me);
        setUser(me);
        realtime.start();
      } catch {
        if (!alive) return;
        clearTokens();
        setUser(null);
      } finally {
        if (alive) setLoading(false);
      }
    })();
    return () => { alive = false; };
  }, []);

  const value = {
    user, setUser, loading, login, register, logout, verify2fa, refreshMe,
    isAdmin: !!(user && user.role === 'admin'),
  };

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  return useContext(AuthContext);
}
