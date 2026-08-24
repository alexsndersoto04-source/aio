import React, { createContext, useContext, useState, useEffect } from 'react';
import { api } from './api.js';

const AuthCtx = createContext(null);

export function useAuth() {
  return useContext(AuthCtx);
}

export function AuthProvider({ children }) {
  const [user, setUser] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const token = localStorage.getItem('moon_token');
    if (!token) { setLoading(false); return; }
    api('/api/auth/me')
      .then(u => setUser(u))
      .catch(() => { localStorage.removeItem('moon_token'); })
      .finally(() => setLoading(false));
  }, []);

  async function login(username, password) {
    const data = await api('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    });
    localStorage.setItem('moon_token', data.token);
    setUser(data.user);
  }

  async function register(username, email, password) {
    const data = await api('/api/auth/register', {
      method: 'POST',
      body: JSON.stringify({ username, email, password }),
    });
    localStorage.setItem('moon_token', data.token);
    setUser(data.user);
  }

  function logout() {
    localStorage.removeItem('moon_token');
    setUser(null);
  }

  return (
    <AuthCtx.Provider value={{ user, loading, login, register, logout }}>
      {children}
    </AuthCtx.Provider>
  );
}
