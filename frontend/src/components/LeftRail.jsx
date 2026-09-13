// Moon — Columna izquierda (escritorio)
// ============================================================
// La columna de accesos directos de las redes grandes: tu cuenta arriba y
// los destinos con su icono en círculo. Los contadores (avisos, mensajes)
// son reales: vienen del WebSocket.

import React from 'react';
import { useAuth } from '../auth.jsx';
import { useUnread } from '../unread.js';
import Avatar from './Avatar.jsx';
import {
  IconHome, IconExplore, IconBookmark, IconBell, IconMail, IconUser,
  IconSettings, IconShield, IconUsers, IconTrend, IconLayers,
} from './Icons.jsx';

function useSeccion() {
  const [hash, setHash] = React.useState(window.location.hash);
  React.useEffect(() => {
    const h = () => setHash(window.location.hash);
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);
  return hash.replace(/^#\/?/, '').split('/').slice(0, 2).join('/');
}

export default function LeftRail() {
  const { user, isAdmin } = useAuth();
  const unread = useUnread();
  const seccion = useSeccion();
  if (!user) return null;

  const fila = (a, etiqueta, icono, contador) => (
    <a
      key={a}
      href={`#/${a}`}
      className={`fila-acceso${seccion === a ? ' active' : ''}`}
      aria-current={seccion === a ? 'page' : undefined}
    >
      <span className="icono-circulo">{icono}</span>
      <span className="texto">{etiqueta}</span>
      {contador > 0 ? <span className="badge">{contador > 99 ? '99+' : contador}</span> : null}
    </a>
  );

  return (
    <aside className="rail-izq" aria-label="Accesos directos">
      <a className="cuenta" href={`#/user/${user.id}`}>
        <Avatar user={user} size="md" />
        <span>
          <b>{user.display_name || user.username}</b>
          <small>Ver mi perfil</small>
        </span>
      </a>

      <nav className="accesos">
        {fila('feed', 'Inicio', <IconHome />)}
        {fila('explore', 'Explorar', <IconExplore />)}
        {fila('notifications', 'Notificaciones', <IconBell />, unread.notifications)}
        {fila('messages', 'Mensajes', <IconMail />, unread.messages)}
        {fila(`user/${user.id}`, 'Mi perfil', <IconUser />)}
        {fila('profile/saved', 'Guardados', <IconBookmark />)}
        {fila('amigos', 'Contactos', <IconUsers />)}
        {fila('explore?type=posts', 'Tendencias', <IconTrend />)}
        {fila('settings', 'Ajustes', <IconSettings />)}
        {isAdmin ? fila('admin', 'Administración', <IconShield />) : null}
      </nav>

      <div className="rail-izq-pie">
        <p className="marca-casa">
          <IconLayers /> Moon · Órbita
        </p>
        <p className="legal">
          Hecho para conectar personas.
          <br />
          Privacidad · Condiciones · Ayuda
        </p>
      </div>
    </aside>
  );
}
