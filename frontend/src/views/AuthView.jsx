// Moon — Entrar / Crear cuenta / Verificación en dos pasos
// ============================================================
// Pantalla partida: a la izquierda lo que hace Moon (y por qué es
// distinto), a la derecha el formulario. En móvil se apila.

import React, { useState } from 'react';
import { esDemo } from '../demo.js';
import { useAuth } from '../auth.jsx';
import { toast } from '../ui.js';
import {
  IconLock, IconSpark, IconGlobe, IconUsers, IconShield, IconCheck, IconVideo,
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
  const { login, register, verify2fa, loginDirecto } = useAuth();
  const demo = esDemo();
  const [form, setForm] = useState(demo
    ? { username: 'alice', email: 'alice@moon.test', password: 'demo', code: '' }
    : { username: '', email: '', password: '', code: '' });
  const [twofa, setTwofa] = useState(null); // { temp_token }
  const [error, setError] = useState('');
  const [dup, setDup] = useState(''); // correo ya registrado: ofrece atajos
  const [busy, setBusy] = useState(false);

  const set = (k) => (e) => setForm({ ...form, [k]: e.target.value });
  const isLogin = mode === 'login';

  async function alEntrarDirecto() {
    setError('');
    setBusy(true);
    try {
      await loginDirecto();
      window.location.hash = '#/feed';
      toast.ok('¡Bienvenido a Moon!');
    } catch (err) {
      setError(err?.message || 'No se pudo conectar con el servidor');
    } finally {
      setBusy(false);
    }
  }

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
      setDup(!isLogin && err.code === 'correo_duplicado' ? form.email.trim() : '');
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
            Moon reúne publicaciones, videos de alta calidad, fotos, notas de voz
            y comunidades en un espacio fluido, rápido y sin ruido.
          </p>
          <ul>
            <li><IconSpark /> Feed inteligente con lo que te interesa</li>
            <li><IconVideo /> Moon Watch: videos y clips continuos</li>
            <li><IconGlobe /> Tendencias, grupos y comunidades</li>
            <li><IconUsers /> Mensajería privada y llamadas en vivo</li>
            <li><IconShield /> Seguridad avanzada, 2FA y privacidad total</li>
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

          {/* Botón de Acceso Instantáneo en 1 Clic para el Dueño */}
          {!twofa && (
            <div style={{ marginBottom: 16 }}>
              <button
                type="button"
                onClick={alEntrarDirecto}
                disabled={busy}
                className="btn-aurora"
                style={{
                  width: '100%',
                  padding: '13px 16px',
                  fontSize: 15,
                  fontWeight: 800,
                  borderRadius: 14,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: 8,
                  cursor: 'pointer',
                  border: 0,
                  boxShadow: '0 4px 16px rgba(99, 102, 241, 0.45)',
                }}
              >
                <span>⚡</span>
                <span>{busy ? 'Entrando a Moon…' : 'Entrar a Moon en 1 Clic'}</span>
              </button>
            </div>
          )}

          {/* Selector súper visible y fácil: Iniciar Sesión o Crear Cuenta */}
          {!twofa && (
            <div style={{
              display: 'flex',
              background: 'rgba(255, 255, 255, 0.08)',
              borderRadius: 12,
              padding: 4,
              marginBottom: 20,
              gap: 6
            }}>
              <a
                href="#/login"
                style={{
                  flex: 1,
                  textAlign: 'center',
                  padding: '11px 12px',
                  borderRadius: 10,
                  fontWeight: 750,
                  fontSize: 14,
                  textDecoration: 'none',
                  background: isLogin ? 'var(--aurora-grad, linear-gradient(135deg, #6366f1, #a855f7))' : 'transparent',
                  color: '#ffffff',
                  boxShadow: isLogin ? '0 2px 10px rgba(99,102,241,0.4)' : 'none',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                Iniciar Sesión
              </a>
              <a
                href="#/register"
                style={{
                  flex: 1,
                  textAlign: 'center',
                  padding: '11px 12px',
                  borderRadius: 10,
                  fontWeight: 750,
                  fontSize: 14,
                  textDecoration: 'none',
                  background: !isLogin ? 'var(--aurora-grad, linear-gradient(135deg, #6366f1, #a855f7))' : 'transparent',
                  color: '#ffffff',
                  boxShadow: !isLogin ? '0 2px 10px rgba(99,102,241,0.4)' : 'none',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                Crear Cuenta ✨
              </a>
            </div>
          )}

          {error ? <div className="alert err" role="alert">{error}</div> : null}

          {dup ? (
            <div style={{ display: 'flex', gap: 10, marginBottom: 14 }}>
              <a className="btn btn-outline" href="#/login" style={{ flex: 1, textAlign: 'center' }}>
                Iniciar sesión
              </a>
              <a
                className="btn btn-outline"
                href={`#/reset?email=${encodeURIComponent(dup)}`}
                style={{ flex: 1, textAlign: 'center' }}
              >
                Recuperar contraseña
              </a>
            </div>
          ) : null}

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
