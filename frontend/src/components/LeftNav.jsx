import React, { useEffect, useState } from 'react';
import { useAuth } from '../auth.jsx';
import { api } from '../api.js';
import Avatar from './Avatar.jsx';
import {
  HomeIcon,
  CompassIcon,
  BellIcon,
  ChatIcon,
  BookmarkIcon,
  SettingsIcon,
  LogoutIcon,
  MoonLogo,
} from './Icons.jsx';

const NAV = [
  { route: 'feed', label: 'Inicio', Icon: HomeIcon },
  { route: 'explore', label: 'Explorar', Icon: CompassIcon },
  { route: 'notifications', label: 'Notificaciones', Icon: BellIcon, badge: true },
  { route: 'messages', label: 'Mensajes', Icon: ChatIcon },
  { route: 'saved', label: 'Guardados', Icon: BookmarkIcon },
  { route: 'settings', label: 'Ajustes', Icon: SettingsIcon },
];

export default function LeftNav() {
  const { user, logout } = useAuth();
  const [hash, setHash] = useState(window.location.hash || '#/feed');
  const [unread, setUnread] = useState(0);

  useEffect(() => {
    const f = () => setHash(window.location.hash || '#/feed');
    window.addEventListener('hashchange', f);
    return () => window.removeEventListener('hashchange', f);
  }, []);

  useEffect(() => {
    api('/api/notifications')
      .then(list => setUnread(list.filter(n => n.is_read == 0).length))
      .catch(() => {});
  }, [hash]);

  const active = r => hash.startsWith(`#/${r}`);

  return (
    <aside className="leftnav">
      <a className="brand" href="#/feed">
        <MoonLogo size={26} />
        <span>Moon</span>
      </a>
      <nav className="leftnav-items">
        {NAV.map(({ route, label, Icon, badge }) => (
          <a
            key={route}
            href={`#/${route}`}
            className={`nav-item ${active(route) ? 'nav-item-active' : ''}`}
          >
            <Icon size={20} />
            <span>{label}</span>
            {badge && unread > 0 && (
              <span className="nav-badge">{unread > 99 ? '99+' : unread}</span>
            )}
          </a>
        ))}
      </nav>
      <div className="leftnav-footer">
        <a className="user-chip" href={`#/profile/${user.id}`}>
          <Avatar src={user.avatar_url} name={user.display_name || user.username} size="sm" />
          <div className="user-chip-text">
            <strong>{user.display_name || user.username}</strong>
            <span>@{user.username}</span>
          </div>
          <button className="icon-btn" title="Cerrar sesión" onClick={logout}>
            <LogoutIcon size={17} />
          </button>
        </a>
      </div>
    </aside>
  );
}
