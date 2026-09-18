// Moon — Entrar / Crear cuenta / Verificación en dos pasos
// ============================================================
// Pantalla partida: a la izquierda lo que hace Moon (y por qué es
// distinto), a la derecha el formulario. En móvil se apila.

import React, { useState } from 'react';
import { esDemo } from '../demo.js';
import { useAuth } from '../auth.jsx';
import { toast } from '../ui.js';
import {
  IconLock, IconSpark, IconGlobe, IconUsers, IconShield, IconCheck,
} from '../components/Icons.jsx';

function Campo({ label, hint, ...props }) {
  return (
    <div className="field">
      <label htmlFor={props.id}>{label}</label>
      <input className="input" {...props} />
      {hint ? <span className="hint">{hint}</span> : null}
    </div>
  );
}

export default function AuthView({ mode }) {
  const { login, register, verify2fa } = useAuth();
  const demo = esDemo();
  const [form, setForm] = useState(demo
    ? { username: 'alice', email: 'alice@moon.test', password: 'demo', code: '' }
    : { username: '', email: '', password: '', code: '' });
  const [twofa, setTwofa] = useState(null); // { temp_token }
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });
  const isLogin = mode === 'login';

  async function submit(e) {
    e.preventDefault();
    setError('');
    setBusy(true);
    try {
      if (isLogin) {
        const res = await login(form.username.trim(), form.password);
        if (res.twofa) {
          setTwofa(res);
          toast.info('Te enviamos un código de 6 dígitos por correo');
        }
      } else {
        await register(form.username.trim(), form.email.trim(), form.password);
        toast.ok('Cuenta creada. ¡Bienvenido a Moon!');
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
      toast.ok('Identidad verificada');
    } catch (err) {
      setError(err.message);
      setBusy(false);
    }
  }

  return (
    <div className="auth-shell">
      <div className="auth-layout">
        <section className="auth-pitch">
          <div className="auth-orb" aria-hidden="true" />
          <h1>Tu órbita,<br />tu conversación.</h1>
          <p>
            Moon reúne publicaciones, fotos, mensajes en vivo y comunidades en un
            espacio tranquilo, rápido y sin ruido.
          </p>
          <ul>
            <li><IconSpark /> Feed con lo que de verdad te interesa</li>
            <li><IconGlobe /> Descubre tendencias y personas nuevas</li>
            <li><IconUsers /> Mensajería 1 a 1 en tiempo real</li>
            <li><IconShield /> Cuenta protegida: 2FA, sesiones y privacidad</li>
          </ul>
        </section>

        <section className="auth-card">
          <div className="brand" style={{ padding: 0, marginBottom: 18 }}>
            <span className="dot" aria-hidden="true" />
            <span>Moon<small>Red social</small></span>
          </div>

          <h2>{twofa ? 'Verificación en dos pasos' : (isLogin ? 'Bienvenido de vuelta' : 'Crea tu cuenta')}</h2>
          <p className="sub">
            {twofa
              ? 'Escribe el código que enviamos a tu correo. Caduca en 5 minutos.'
              : isLogin
                ? 'Entra con tu usuario o tu correo electrónico.'
                : 'Elige un usuario, tu correo y una contraseña segura.'}
          </p>

          {error ? <div className="alert err" role="alert">{error}</div> : null}

          {twofa ? (
            <form onSubmit={submit2fa}>
              <Campo
                id="code" label="Código de 6 dígitos" className="input code-input"
                value={form.code} onChange={set('code')} maxLength={6} autoFocus
                inputMode="numeric" autoComplete="one-time-code" required
              />
              <button className="btn btn-primary btn-block btn-lg" disabled={busy}>
                {busy ? 'Verificando…' : (<><IconLock /> Verificar</>)}
              </button>
            </form>
          ) : (
            <form onSubmit={submit}>
              <Campo
                id="username" label={isLogin ? 'Usuario o correo' : 'Usuario'}
                value={form.username} onChange={set('username')} autoFocus={isLogin} required
                autoComplete="username" minLength={3} maxLength={24}
                hint={isLogin ? null : 'Entre 3 y 24 caracteres. Sin espacios.'}
              />
              {!isLogin ? (
                <Campo
                  id="email" label="Correo electrónico" type="email"
                  value={form.email} onChange={set('email')} autoComplete="email" required
                />
              ) : null}
              <Campo
                id="password" label="Contraseña" type="password"
                value={form.password} onChange={set('password')} required
                autoComplete={isLogin ? 'current-password' : 'new-password'}
                minLength={8} maxLength={128}
                hint={isLogin ? null : 'Mínimo 8 caracteres.'}
              />

              {isLogin ? (
                <div style={{ textAlign: 'right', marginBottom: 14 }}>
                  <a href="#/reset" style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--accent)' }}>
                    ¿Olvidaste tu contraseña?
                  </a>
                </div>
              ) : null}

              <button className="btn btn-primary btn-block btn-lg" disabled={busy}>
                {busy ? 'Procesando…' : (isLogin ? 'Entrar' : (<><IconCheck /> Crear cuenta</>))}
              </button>
            </form>
          )}

          <p className="sub" style={{ textAlign: 'center', marginTop: 18, marginBottom: 0 }}>
            {isLogin ? '¿No tienes cuenta? ' : '¿Ya tienes cuenta? '}
            <a href={isLogin ? '#/register' : '#/login'} style={{ color: 'var(--accent)', fontWeight: 650 }}>
              {isLogin ? 'Regístrate' : 'Inicia sesión'}
            </a>
          </p>
        </section>
      </div>
    </div>
  );
}
