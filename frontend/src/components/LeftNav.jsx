// Moon — Navegación izquierda (escritorio)
// ============================================================
// Menú fijo con: marca, acción principal (Publicar), secciones, contadores
// en vivo y la cuenta abajo. El selector de tema vive aquí (claro / oscuro /
// sistema) y se recuerda en el navegador.

import React, { useEffect, useState } from 'react';
import { useAuth } from '../auth.jsx';
import { useUnread } from '../unread.js';
import Avatar from './Avatar.jsx';
import { aplicarTema, getPreferencia, setPreferencia, observarSistema } from '../theme.js';
import {
  IconHome, IconExplore, IconBell, IconMail, IconUser, IconSettings, IconShield,
  IconLogout, IconPlus, IconSun, IconMoon, IconLayers,
} from './Icons.jsx';

function useActive() {
  const [hash, setHash] = useState(window.location.hash);
  useEffect(() => {
    const h = () => setHash(window.location.hash);
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);
  return hash.replace(/^#/, '').split('/')[1] || 'feed';
}

export function ThemeToggle({ compact = false }) {
  const [pref, setPref] = useState(getPreferencia());

  useEffect(() => {
    aplicarTema(pref);
    const dejarDeObservar = observarSistema();
    const alCambiar = () => setPref(getPreferencia());
    window.addEventListener('moon:tema', alCambiar);
    return () => { dejarDeObservar(); window.removeEventListener('moon:tema', alCambiar); };
  }, [pref]);

  const opciones = [
    ['light', 'Claro', <IconSun key="s" />],
    ['dark', 'Oscuro', <IconMoon key="m" />],
    ['system', 'Sistema', <IconLayers key="l" />],
  ];

  return (
    <div className="theme-toggle" role="group" aria-label="Tema de la interfaz">
      {opciones.map(([valor, etiqueta, icono]) => (
        <button
          key={valor}
          type="button"
          className={pref === valor ? 'active' : ''}
          onClick={() => setPreferencia(valor)}
          title={`Tema ${etiqueta.toLowerCase()}`}
          aria-label={`Tema ${etiqueta.toLowerCase()}`}
          aria-pressed={pref === valor}
        >
          {icono}
          {compact ? null : <span className="sr-only">{etiqueta}</span>}
        </button>
      ))}
    </div>
  );
}

export default function LeftNav() {
  const { user, logout, isAdmin } = useAuth();
  const active = useActive();
  const unread = useUnread();
  if (!user) return null;

  const item = (to, label, icon, badge) => (
    <a key={to} href={`#/${to}`} className={active === to ? 'active' : ''} aria-current={active === to ? 'page' : undefined}>
      {icon}
      <span>{label}</span>
      {badge ? <span className="badge">{badge > 99 ? '99+' : badge}</span> : null}
    </a>
  );

  return (
    <aside className="sidebar">
      <a className="brand" href="#/feed" aria-label="Moon, ir al inicio">
        <LogoMoon tamano={30} />
        <span>
          Moon
          <small>Red social</small>
        </span>
      </a>

      <a className="nav-cta" href="#/feed" onClick={() => window.dispatchEvent(new CustomEvent('moon:componer'))}>
        <IconPlus />
        <span>Publicar</span>
      </a>

      <nav className="nav" aria-label="Secciones">
        {item('feed', 'Inicio', <IconHome />)}
        {item('explore', 'Explorar', <IconExplore />)}
        {item('notifications', 'Notificaciones', <IconBell />, unread.notifications)}
        {item('messages', 'Mensajes', <IconMail />, unread.messages)}
        {item('profile', 'Perfil', <IconUser />)}
        {item('settings', 'Ajustes', <IconSettings />)}
        {isAdmin ? item('admin', 'Administración', <IconShield />) : null}
      </nav>

      <div className="nav-foot">
        <ThemeToggle />
        <a className="nav-user" href="#/profile" style={{ marginTop: 10 }}>
          <Avatar user={user} size="sm" />
          <span className="meta">
            <b className="ellipsis">{user.display_name || user.username}</b>
            <span className="ellipsis">@{user.username}</span>
          </span>
        </a>
        <button className="icon-btn" onClick={() => logout()} title="Cerrar sesión" aria-label="Cerrar sesión">
          <IconLogout />
        </button>
      </div>
    </aside>
  );
}
