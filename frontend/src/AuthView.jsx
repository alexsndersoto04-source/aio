import React, { useState } from 'react';
import { useAuth } from './auth.jsx';

export default function AuthView() {
  const { login, register } = useAuth();
  const [mode, setMode] = useState('login');
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function handleSubmit(e) {
    e.preventDefault();
    setError('');
    setBusy(true);
    try {
      if (mode === 'login') {
        await login(username, password);
      } else {
        await register(username, email, password);
      }
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth-container">
      <div className="auth-card">
        <h1 className="auth-title">Moon</h1>
        <p className="auth-subtitle">{mode === 'login' ? 'Inicia sesion' : 'Crea tu cuenta'}</p>
        <form onSubmit={handleSubmit}>
          <input
            className="input"
            placeholder="Usuario"
            value={username}
            onChange={e => setUsername(e.target.value)}
            required
          />
          {mode === 'register' && (
            <input
              className="input"
              type="email"
              placeholder="Correo"
              value={email}
              onChange={e => setEmail(e.target.value)}
              required
            />
          )}
          <input
            className="input"
            type="password"
            placeholder="Contrasena"
            value={password}
            onChange={e => setPassword(e.target.value)}
            required
          />
          {error && <p className="error-text">{error}</p>}
          <button className="btn btn-primary" type="submit" disabled={busy}>
            {busy ? '...' : mode === 'login' ? 'Entrar' : 'Registrarse'}
          </button>
        </form>
        <p className="auth-toggle">
          {mode === 'login' ? (
            <>No tienes cuenta? <button className="link-btn" onClick={() => { setMode('register'); setError(''); }}>Registrate</button></>
          ) : (
            <>Ya tienes cuenta? <button className="link-btn" onClick={() => { setMode('login'); setError(''); }}>Inicia sesion</button></>
          )}
        </p>
      </div>
    </div>
  );
}
