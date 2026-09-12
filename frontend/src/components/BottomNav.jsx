// Moon — Navegación inferior (móvil)
// ============================================================
// Cinco destinos como máximo, con el contador de no leídos y el estado
// activo marcado. La acción de publicar vive en el botón flotante
// (`FloatingCompose`), que es lo natural con el pulgar.

import React, { useEffect, useState } from 'react';
import { useAuth } from '../auth.jsx';
import { useUnread } from '../unread.js';
import {
  IconHome, IconExplore, IconBell, IconMail, IconUser, IconSettings, IconShield,
} from './Icons.jsx';

export default function BottomNav() {
  const { user, isAdmin } = useAuth();
  const unread = useUnread();
  const [hash, setHash] = useState(window.location.hash);

  useEffect(() => {
    const h = () => setHash(window.location.hash);
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);
  if (!user) return null;

  const active = hash.replace(/^#/, '').split('/')[1] || 'feed';

  const item = (to, label, icon, badge) => (
    <a key={to} href={`#/${to}`} className={active === to ? 'active' : ''} aria-current={active === to ? 'page' : undefined}>
      {icon}
      <span>{label}</span>
      {badge ? <span className="badge">{badge > 99 ? '99+' : badge}</span> : null}
    </a>
  );

  return (
    <nav className="bottom-nav" aria-label="Secciones">
      {item('feed', 'Inicio', <IconHome />)}
      {item('explore', 'Buscar', <IconExplore />)}
      {item('messages', 'Chat', <IconMail />, unread.messages)}
      {item('notifications', 'Alertas', <IconBell />, unread.notifications)}
      {isAdmin
        ? item('admin', 'Admin', <IconShield />)
        : item('settings', 'Ajustes', <IconSettings />)}
    </nav>
  );
}

/** Botón flotante de publicar (solo móvil). */
export function FloatingCompose() {
  const { user } = useAuth();
  if (!user) return null;
  return (
    <button
      className="fab"
      aria-label="Escribir una publicación"
      onClick={() => {
        if (window.location.hash.replace(/^#\/?/, '').split('/')[0] !== 'feed') {
          window.location.hash = '#/feed';
        }
        setTimeout(() => window.dispatchEvent(new CustomEvent('moon:componer')), 60);
      }}
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
        <path d="M12 5v14M5 12h14" />
      </svg>
    </button>
  );
}
