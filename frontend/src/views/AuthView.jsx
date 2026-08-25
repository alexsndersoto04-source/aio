import React, { useState } from 'react';
import { useAuth } from '../auth.jsx';
import { MoonLogo } from '../components/Icons.jsx';

export default function AuthView() {
  const { login, register } = useAuth();
  const [mode, setMode] = useState('login');
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  function switchMode(m) {
    setMode(m);
    setError('');
  }

  async function submit(e) {
    e.preventDefault();
    setError('');
    setBusy(true);
    try {
      if (mode === 'login') await login(username, password);
      else await register(username, email, password);
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth">
      <div className="auth-brand">
        <MoonLogo size={46} />
        <h1>Moon</h1>
        <p>
          Tu comunidad, a la altura.
          <br />
          Publica, conecta y sigue lo que importa.
        </p>
        <ul>
          <li>Publicaciones con me gusta y comentarios</li>
          <li>Mensajes directos entre usuarios</li>
          <li>Notificaciones de tu red</li>
          <li>Perfiles y tendencias</li>
        </ul>
      </div>
      <div className="auth-panel">
        <div className="auth-card">
          <div className="auth-tabs">
            <button
              className={`tab ${mode === 'login' ? 'tab-active' : ''}`}
              onClick={() => switchMode('login')}
              type="button"
            >
              Entrar
            </button>
            <button
              className={`tab ${mode === 'register' ? 'tab-active' : ''}`}
              onClick={() => switchMode('register')}
              type="button"
            >
              Crear cuenta
            </button>
          </div>
          <form onSubmit={submit}>
            <label className="field">
              <span>Usuario</span>
              <input
                className="input"
                value={username}
                onChange={e => setUsername(e.target.value)}
                required
                minLength={3}
                autoComplete="username"
              />
            </label>
            {mode === 'register' && (
              <label className="field">
                <span>Correo</span>
                <input
                  className="input"
                  type="email"
                  value={email}
                  onChange={e => setEmail(e.target.value)}
                  required
                  autoComplete="email"
                />
              </label>
            )}
            <label className="field">
              <span>Contraseña</span>
              <input
                className="input"
                type="password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                required
                minLength={8}
                autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              />
            </label>
            {mode === 'register' && <p className="muted xs field-hint">Mínimo 8 caracteres.</p>}
            {error && <p className="form-error">{error}</p>}
            <button className="btn btn-primary btn-block" type="submit" disabled={busy}>
              {busy ? 'Un momento…' : mode === 'login' ? 'Entrar' : 'Crear cuenta'}
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
