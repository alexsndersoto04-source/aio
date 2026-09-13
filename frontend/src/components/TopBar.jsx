// Moon — Barra superior (móvil y tablet)
// ============================================================
// Marca a la izquierda y accesos rápidos a la derecha. En escritorio no
// aparece: allí manda el menú lateral.

import React from 'react';
import { useAuth } from '../auth.jsx';
import { useUnread } from '../unread.js';
import { IconBell, IconMail, IconExplore } from './Icons.jsx';

export default function TopBar() {
  const { user } = useAuth();
  const unread = useUnread();
  if (!user) return null;

  return (
    <header className="topbar-app">
      <a className="marca-app" href="#/feed" aria-label="Moon, ir al inicio">
        <span className="punto" aria-hidden="true" />
        Moon
      </a>
      <div className="acciones">
        <a href="#/explore" aria-label="Buscar" title="Buscar"><IconExplore /></a>
        <a href="#/notifications" aria-label="Notificaciones" title="Notificaciones">
          <IconBell />
          {unread.notifications > 0 ? <span className="badge">{unread.notifications > 99 ? '99+' : unread.notifications}</span> : null}
        </a>
        <a href="#/messages" aria-label="Mensajes" title="Mensajes">
          <IconMail />
          {unread.messages > 0 ? <span className="badge">{unread.messages > 99 ? '99+' : unread.messages}</span> : null}
        </a>
      </div>
    </header>
  );
}
