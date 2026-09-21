// Moon — Navegación inferior (móvil)
// ============================================================
// Todas las secciones esenciales a la vista:
// Inicio, Videos, Explorar, Grupos, Mensajes, Perfil.

import React, { useEffect, useState } from 'react';
import { useAuth } from '../auth.jsx';
import { useUnread } from '../unread.js';
import {
  IconHome, IconExplore, IconMail, IconUser, IconLayers, IconPlay,
} from './Icons.jsx';

export default function BottomNav() {
  const { user } = useAuth();
  const unread = useUnread();
  const [hash, setHash] = useState(window.location.hash);

  useEffect(() => {
    const h = () => setHash(window.location.hash);
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);
  if (!user) return null;

  const ruta = hash.replace(/^#\/?/, '');
  const active = ruta.split('/')[0] || 'feed';

  const item = (to, label, icon, badge) => (
    <a
      key={to}
      href={`#/${to}`}
      className={active === to.split('/')[0] ? 'active' : ''}
      aria-current={active === to.split('/')[0] ? 'page' : undefined}
    >
      {icon}
      <span>{label}</span>
      {badge ? <span className="badge">{badge > 99 ? '99+' : badge}</span> : null}
    </a>
  );

  return (
    <nav className="bottom-nav bottom-nav--seis" aria-label="Secciones">
      {item('feed', 'Inicio', <IconHome />)}
      {item('videos', 'Video', <IconPlay />)}
      {item('explore', 'Explorar', <IconExplore />)}
      {item('grupos', 'Grupos', <IconLayers />)}
      {item('messages', 'Mensajes', <IconMail />, unread.messages)}
      {item(`user/${user.id}`, 'Perfil', <IconUser />)}
    </nav>
  );
}
