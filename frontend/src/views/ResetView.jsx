// Moon — Recuperación de contraseña (solicitar + restablecer)

import React, { useState } from 'react';
import { authApi } from '../api.js';

export default function ResetView({ token }) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [msg, setMsg] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function request(e) {
    e.preventDefault();
    setBusy(true);
    setError('');
    setMsg('');
    try {
      const res = await authApi.recoveryRequest(email.trim());
      setMsg(res.message || 'Revisa tu correo');
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  async function reset(e) {
    e.preventDefault();
    setBusy(true);
    setError('');
    setMsg('');
    if (password !== confirm) {
      setError('Las contraseñas no coinciden');
      setBusy(false);
      return;
    }
    try {
      await authApi.recoveryVerify(token, password);
      setMsg('Contraseña actualizada. Ya puedes iniciar sesión.');
      setTimeout(() => { window.location.hash = '#/login'; }, 1600);
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth-shell">
      <div className="auth-card">
        <div className="brand"><span className="dot" />Moon</div>
        <h2>{token ? 'Nueva contraseña' : 'Recuperar acceso'}</h2>
        <p className="sub">
          {token
            ? 'Elige una contraseña nueva (mínimo 8 caracteres, letras y números).'
            : 'Te enviaremos un enlace seguro por correo.'}
        </p>
        {msg ? <div className="alert ok">{msg}</div> : null}
        {error ? <div className="alert err">{error}</div> : null}

        {token ? (
          <form onSubmit={reset}>
            <div className="field">
              <label>Nueva contraseña</label>
              <input className="input" type="password" value={password}
                onChange={(e) => setPassword(e.target.value)} required minLength={8} maxLength={128} />
            </div>
            <div className="field">
              <label>Repite la contraseña</label>
              <input className="input" type="password" value={confirm}
                onChange={(e) => setConfirm(e.target.value)} required minLength={8} maxLength={128} />
            </div>
            <button className="btn btn-block btn-lg" disabled={busy}>
              {busy ? 'Guardando…' : 'Guardar contraseña'}
            </button>
          </form>
        ) : (
          <form onSubmit={request}>
            <div className="field">
              <label>Correo electrónico</label>
              <input className="input" type="email" value={email}
                onChange={(e) => setEmail(e.target.value)} required autoFocus />
            </div>
            <button className="btn btn-block btn-lg" disabled={busy}>
              {busy ? 'Enviando…' : 'Enviar enlace'}
            </button>
          </form>
        )}

        <div className="switch">
          <a href="#/login">← Volver al inicio de sesión</a>
        </div>
      </div>
    </div>
  );
}
