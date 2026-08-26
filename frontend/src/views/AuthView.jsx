// Moon — Login / Registro / 2FA

import React, { useState } from 'react';
import { useAuth } from '../auth.jsx';

function Field({ label, ...props }) {
  return (
    <div className="field">
      <label>{label}</label>
      <input className="input" {...props} />
    </div>
  );
}

export default function AuthView({ mode }) {
  const { login, register, verify2fa } = useAuth();
  const [form, setForm] = useState({ username: '', email: '', password: '', code: '' });
  const [twofa, setTwofa] = useState(null); // {temp_token}
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });

  async function submit(e) {
    e.preventDefault();
    setError('');
    setBusy(true);
    try {
      if (mode === 'login') {
        const res = await login(form.username.trim(), form.password);
        if (res.twofa) setTwofa(res);
      } else {
        await register(form.username.trim(), form.email.trim(), form.password);
      }
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  async function submit2fa(e) {
    e.preventDefault();
    setError('');
    setBusy(true);
    try {
      await verify2fa(twofa.temp_token, form.code.trim());
    } catch (err) {
      setError(err.message);
      setBusy(false);
    }
  }

  const isLogin = mode === 'login';

  return (
    <div className="auth-shell">
      <div className="auth-card">
        <div className="brand"><span className="dot" />Moon</div>
        <h2>{twofa ? 'Verificación en dos pasos' : (isLogin ? 'Bienvenido de vuelta' : 'Crea tu cuenta')}</h2>
        <p className="sub">
          {twofa
            ? 'Te enviamos un código por correo. Caduca en 5 minutos.'
            : isLogin
              ? 'Inicia sesión para continuar'
              : 'Únete a la conversación'}
        </p>

        {error ? <div className="alert err">{error}</div> : null}

        {twofa ? (
          <form onSubmit={submit2fa}>
            <Field label="Código de 6 dígitos" value={form.code} onChange={set('code')}
              className="input code-input" maxLength={6} autoFocus inputMode="numeric" required />
            <button className="btn btn-block btn-lg" disabled={busy}>
              {busy ? 'Verificando…' : 'Verificar'}
            </button>
          </form>
        ) : (
          <form onSubmit={submit}>
            <Field label="Usuario" value={form.username} onChange={set('username')}
              autoComplete="username" autoFocus={isLogin} required
              minLength={3} maxLength={24} />
            {!isLogin ? (
              <Field label="Correo electrónico" type="email" value={form.email} onChange={set('email')}
                autoComplete="email" required />
            ) : null}
            <Field label="Contraseña" type="password" value={form.password} onChange={set('password')}
              autoComplete={isLogin ? 'current-password' : 'new-password'} required
              minLength={8} maxLength={128} />
            {isLogin ? (
              <div style={{ textAlign: 'right', marginBottom: 14 }}>
                <a href="#/reset" style={{ fontSize: 13.5, fontWeight: 600 }}>¿Olvidaste tu contraseña?</a>
              </div>
            ) : null}
            <button className="btn btn-block btn-lg" disabled={busy}>
              {busy ? 'Procesando…' : (isLogin ? 'Entrar' : 'Crear cuenta')}
            </button>
          </form>
        )}

        <div className="switch">
          {isLogin ? (
            <>¿No tienes cuenta? <a href="#/register">Regístrate</a></>
          ) : (
            <>¿Ya tienes cuenta? <a href="#/login">Inicia sesión</a></>
          )}
        </div>
      </div>
    </div>
  );
}
