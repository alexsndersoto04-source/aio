import React from 'react';
import { useAuth } from './auth.jsx';

export default function Sidebar() {
  const { user, logout } = useAuth();

  return (
    <aside className="sidebar">
      <div className="sidebar-logo">Moon</div>
      <nav className="sidebar-nav">
        <a href="#/feed" className="nav-link">Feed</a>
        <a href="#/trending" className="nav-link">Tendencias</a>
        <a href="#/notifications" className="nav-link">Notificaciones</a>
        <a href="#/messages" className="nav-link">Mensajes</a>
        <a href={`#/profile/${user.id}`} className="nav-link">Mi perfil</a>
      </nav>
      <div className="sidebar-user">
        <span className="sidebar-username">@{user.username}</span>
        <button className="btn btn-sm" onClick={logout}>Salir</button>
      </div>
    </aside>
  );
}
