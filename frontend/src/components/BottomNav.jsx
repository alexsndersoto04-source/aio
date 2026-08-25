import React, { useEffect, useState } from 'react';
import { useAuth } from '../auth.jsx';
import { HomeIcon, CompassIcon, ChatIcon, BellIcon, UserIcon } from './Icons.jsx';

const ITEMS = [
  { route: 'feed', label: 'Inicio', Icon: HomeIcon },
  { route: 'explore', label: 'Explorar', Icon: CompassIcon },
  { route: 'messages', label: 'Mensajes', Icon: ChatIcon },
  { route: 'notifications', label: 'Alertas', Icon: BellIcon },
  { route: 'profile', label: 'Perfil', Icon: UserIcon },
];

export default function BottomNav() {
  const { user } = useAuth();
  const [hash, setHash] = useState(window.location.hash || '#/feed');

  useEffect(() => {
    const f = () => setHash(window.location.hash || '#/feed');
    window.addEventListener('hashchange', f);
    return () => window.removeEventListener('hashchange', f);
  }, []);

  return (
    <nav className="bottomnav">
      {ITEMS.map(({ route, label, Icon }) => {
        const href = route === 'profile' ? `#/profile/${user.id}` : `#/${route}`;
        const active = hash.startsWith(`#/${route}`);
        return (
          <a key={route} href={href} className={`bn-item ${active ? 'bn-active' : ''}`}>
            <Icon size={21} />
            <span>{label}</span>
          </a>
        );
      })}
    </nav>
  );
}
