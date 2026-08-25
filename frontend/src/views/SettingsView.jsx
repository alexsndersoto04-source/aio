import React from 'react';
import { useAuth } from '../auth.jsx';
import { useTheme } from '../theme.jsx';
import { SunIcon, MoonIcon } from '../components/Icons.jsx';

export default function SettingsView() {
  const { user, logout } = useAuth();
  const { theme, setTheme } = useTheme();

  const memberSince = user.created_at
    ? new Date(user.created_at).toLocaleDateString('es-VE', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
      })
    : '—';

  return (
    <>
      <div className="page-head">
        <h2>Ajustes</h2>
      </div>

      <section className="card setting-card">
        <h3>Apariencia</h3>
        <div className="theme-options">
          <button
            className={`theme-opt ${theme === 'light' ? 'theme-opt-active' : ''}`}
            onClick={() => setTheme('light')}
            type="button"
          >
            <SunIcon size={20} />
            <span>Tema claro</span>
          </button>
          <button
            className={`theme-opt ${theme === 'dark' ? 'theme-opt-active' : ''}`}
            onClick={() => setTheme('dark')}
            type="button"
          >
            <MoonIcon size={20} />
            <span>Tema oscuro</span>
          </button>
        </div>
      </section>

      <section className="card setting-card">
        <h3>Cuenta</h3>
        <div className="setting-rows">
          <div>
            <span>Usuario</span>
            <strong>@{user.username}</strong>
          </div>
          <div>
            <span>Correo</span>
            <strong>{user.email}</strong>
          </div>
          <div>
            <span>Nombre</span>
            <strong>{user.display_name || '—'}</strong>
          </div>
          <div>
            <span>Miembro desde</span>
            <strong>{memberSince}</strong>
          </div>
        </div>
      </section>

      <section className="card setting-card">
        <h3>Sesión</h3>
        <p className="muted sm setting-note">Cierra tu sesión en este dispositivo.</p>
        <button className="btn btn-danger" onClick={logout} type="button">
          Cerrar sesión
        </button>
      </section>

      <section className="card setting-card">
        <h3>Próximamente</h3>
        <div className="soon-list">
          <span className="soon-chip">Editar perfil (bio, foto, link)</span>
          <span className="soon-chip">Privacidad (quién puede escribirte)</span>
          <span className="soon-chip">Verificación en dos pasos</span>
          <span className="soon-chip">Idioma (Español / English)</span>
        </div>
      </section>
    </>
  );
}
