// Moon — Navegación izquierda (escritorio)

import React from 'react';
import { useAuth } from '../auth.jsx';
import { useUnread } from '../unread.js';
import Avatar from './Avatar.jsx';
import {
  IconHome, IconExplore, IconBell, IconMail, IconUser, IconSettings, IconShield, IconLogout,
} from './Icons.jsx';

function useActive() {
  const [hash, setHash] = React.useState(window.location.hash);
  React.useEffect(() => {
    const h = () => setHash(window.location.hash);
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);
  return hash.replace(/^#/, '').split('/')[1] || 'feed';
}

export default function LeftNav() {
  const { user, logout, isAdmin } = useAuth();
  const active = useActive();
  const unread = useUnread();
  if (!user) return null;

  const item = (to, label, icon, badge) => {
    const cls = active === to ? 'active' : '';
    return (
      <a key={to} href={`#/${to}`} className={cls}>
        {icon}
        <span>{label}</span>
        {badge ? <span className="badge">{badge}</span> : null}
      </a>
    );
  };

  return (
    <aside className="sidebar">
      <a className="brand" href="#/feed">
        <span className="dot" />
        Moon
      </a>
      <nav className="nav">
        {item('feed', 'Inicio', <IconHome />)}
        {item('explore', 'Explorar', <IconExplore />)}
        {item('notifications', 'Notificaciones', <IconBell />, unread.notifications || null)}
        {item('messages', 'Mensajes', <IconMail />, unread.messages || null)}
        {item('profile', 'Perfil', <IconUser />)}
        {item('settings', 'Ajustes', <IconSettings />)}
        {isAdmin ? item('admin', 'Admin', <IconShield />) : null}
      </nav>
      <div className="nav-foot">
        <div className="row" style={{ marginBottom: 10 }}>
          <a href="#/profile" className="row" style={{ flex: 1, minWidth: 0 }}>
            <Avatar user={user} size="sm" />
            <span className="grow ellipsis" style={{ fontWeight: 600 }}>
              {user.display_name || user.username}
            </span>
          </a>
          <button className="btn-ghost btn-sm" onClick={() => logout()} title="Cerrar sesión">
            <IconLogout />
          </button>
        </div>
        <div>© {new Date().getFullYear()} Moon</div>
      </div>
    </aside>
  );
}
