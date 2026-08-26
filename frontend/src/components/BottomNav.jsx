// Moon — Navegación inferior (móvil)

import React from 'react';
import { useAuth } from '../auth.jsx';
import {
  IconHome, IconExplore, IconBell, IconMail, IconUser, IconSettings, IconShield,
} from './Icons.jsx';

export default function BottomNav() {
  const { user, isAdmin } = useAuth();
  const [hash, setHash] = React.useState(window.location.hash);
  React.useEffect(() => {
    const h = () => setHash(window.location.hash);
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);
  if (!user) return null;
  const active = hash.replace(/^#/, '').split('/')[1] || 'feed';

  const item = (to, label, icon) => (
    <a key={to} href={`#/${to}`} className={active === to ? 'active' : ''}>
      {icon}
      <span>{label}</span>
    </a>
  );

  return (
    <nav className="bottom-nav">
      {item('feed', 'Inicio', <IconHome />)}
      {item('explore', 'Buscar', <IconExplore />)}
      {item('messages', 'Chat', <IconMail />)}
      {item('notifications', 'Alertas', <IconBell />)}
      {item('profile', 'Perfil', <IconUser />)}
      {isAdmin ? item('admin', 'Admin', <IconShield />) : item('settings', 'Más', <IconSettings />)}
    </nav>
  );
}
