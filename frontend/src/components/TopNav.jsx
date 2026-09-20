// Moon — Barra superior (navegación principal)
// ============================================================
// La barra de arriba de las redes grandes: marca a la izquierda, buscador
// central, pestañas de iconos con subrayado y menú de cuenta a la derecha.
// En móvil se reduce a marca, buscador y avisos; el resto baja a la barra
// inferior.

import React, { useEffect, useRef, useState } from 'react';
import { useAuth } from '../auth.jsx';
import { useUnread } from '../unread.js';
import { ThemeToggle } from './LeftNav.jsx';
import Avatar from './Avatar.jsx';
import LogoMoon from './LogoMoon.jsx';
import {
  IconHome, IconExplore, IconBookmark, IconBell, IconMail, IconUser,
  IconSettings, IconShield, IconSearch, IconX, IconLogout, IconLayers,
  IconGrid, IconPlus, IconUsers, IconMoon,
} from './Icons.jsx';

function useSeccion() {
  const [hash, setHash] = useState(window.location.hash);
  useEffect(() => {
    const h = () => setHash(window.location.hash);
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);
  return hash.replace(/^#\/?/, '').split('/')[0] || 'feed';
}

function numero(n) {
  const v = Number(n || 0);
  return v > 99 ? '99+' : String(v);
}

export default function TopNav() {
  const { user, logout, isAdmin } = useAuth();
  const unread = useUnread();
  const seccion = useSeccion();
  const [busqueda, setBusqueda] = useState('');
  const [menuAbierto, setMenuAbierto] = useState(false);
  const [buscando, setBuscando] = useState(false);
  const menuRef = useRef(null);
  const campoRef = useRef(null);

  // Cerrar el menú de cuenta al pulsar fuera o con Escape.
  useEffect(() => {
    if (!menuAbierto) return;
    const fuera = (e) => {
      if (menuRef.current && !menuRef.current.contains(e.target)) setMenuAbierto(false);
    };
    const tecla = (e) => { if (e.key === 'Escape') setMenuAbierto(false); };
    document.addEventListener('mousedown', fuera);
    document.addEventListener('keydown', tecla);
    return () => {
      document.removeEventListener('mousedown', fuera);
      document.removeEventListener('keydown', tecla);
    };
  }, [menuAbierto]);

  if (!user) return null;

  function buscar(e) {
    e.preventDefault();
    const q = busqueda.trim();
    if (!q) return;
    window.location.hash = `#/explore?q=${encodeURIComponent(q)}&type=users`;
    setBuscando(false);
  }

  const pestaña = (id, etiqueta, icono, contador) => (
    <a
      key={id}
      href={`#/${id}`}
      className={`nav-tab${seccion === id || (id === 'feed' && !seccion) ? ' active' : ''}`}
      aria-current={seccion === id ? 'page' : undefined}
      aria-label={etiqueta}
      title={etiqueta}
    >
      {icono}
      {contador > 0 ? <span className="badge">{numero(contador)}</span> : null}
      <span className="sr-only">{etiqueta}</span>
    </a>
  );

  return (
    <header className="topnav">
      <div className="topnav-izq">
        <a className="marca-app" href="#/feed" aria-label="Moon, ir al inicio">
          <LogoMoon tamano={24} />
          <span className="marca-texto">Moon</span>
        </a>
        <form className={`buscador-top${buscando ? ' abierto' : ''}`} onSubmit={buscar} role="search">
          <IconSearch />
          <input
            ref={campoRef}
            type="search"
            value={busqueda}
            onChange={(e) => setBusqueda(e.target.value)}
            placeholder="Buscar en Moon"
            aria-label="Buscar en Moon"
          />
          {busqueda ? (
            <button type="button" className="limpiar" onClick={() => setBusqueda('')} aria-label="Borrar la búsqueda">
              <IconX />
            </button>
          ) : null}
        </form>
      </div>

      <nav className="topnav-centro" aria-label="Secciones principales">
        {pestaña('feed', 'Inicio', <IconHome />)}
        {pestaña('explore', 'Explorar', <IconExplore />)}
        {pestaña('profile', 'Guardados', <IconBookmark />)}
        {pestaña('notifications', 'Notificaciones', <IconBell />, unread.notifications)}
        {pestaña('messages', 'Mensajes', <IconMail />, unread.messages)}
      </nav>

      <div className="topnav-der">
        <button
          type="button"
          className="solo-movil icono-redondo"
          onClick={() => { setBuscando((v) => !v); setTimeout(() => campoRef.current?.focus(), 40); }}
          aria-label="Buscar"
        >
          <IconSearch />
        </button>
        <a className="solo-movil icono-redondo" href="#/notifications" aria-label={`Avisos${unread.notifications ? ` (${unread.notifications})` : ''}`}>
          <IconBell />
          {unread.notifications > 0 ? <span className="badge">{numero(unread.notifications)}</span> : null}
        </a>
        <ThemeToggle compact />
        <button
          type="button"
          className="btn btn-primary btn-nueva"
          onClick={() => {
            if (!window.location.hash.startsWith('#/feed')) window.location.hash = '#/feed';
            setTimeout(() => window.dispatchEvent(new CustomEvent('moon:componer')), 60);
          }}
        >
          <IconPlus /> <span className="solo-escritorio">Crear</span>
        </button>

        <div className="menu-cuenta" ref={menuRef}>
          <button
            type="button"
            className="chip-cuenta"
            onClick={() => setMenuAbierto((v) => !v)}
            aria-haspopup="menu"
            aria-expanded={menuAbierto}
          >
            <Avatar user={user} size="sm" />
            <span className="nombre solo-escritorio">{user.display_name || user.username}</span>
          </button>

          {menuAbierto ? (
            <div className="menu-cuenta-panel" role="menu">
              <a className="menu-cuenta-cabecera" href={`#/user/${user.id}`} onClick={() => setMenuAbierto(false)}>
                <Avatar user={user} size="md" />
                <span>
                  <b>{user.display_name || user.username}</b>
                  <small>@{user.username}</small>
                </span>
              </a>
              <div className="menu-cuenta-linea" />
              <a href={`#/user/${user.id}`} role="menuitem" onClick={() => setMenuAbierto(false)}><IconUser /> Mi perfil</a>
              <a href="#/notifications" role="menuitem" onClick={() => setMenuAbierto(false)}>
                <IconBell /> Avisos
                {unread.notifications > 0 ? <span className="badge" style={{ marginLeft: 'auto' }}>{numero(unread.notifications)}</span> : null}
              </a>
              <a href="#/messages" role="menuitem" onClick={() => setMenuAbierto(false)}>
                <IconMail /> Mensajes
                {unread.messages > 0 ? <span className="badge" style={{ marginLeft: 'auto' }}>{numero(unread.messages)}</span> : null}
              </a>
              <a href="#/grupos" role="menuitem" onClick={() => setMenuAbierto(false)}><IconLayers /> Grupos</a>
              <a href="#/amigos" role="menuitem" onClick={() => setMenuAbierto(false)}><IconUsers /> Contactos</a>
              <a href="#/profile/saved" role="menuitem" onClick={() => setMenuAbierto(false)}><IconBookmark /> Guardados</a>
              <a href="#/settings" role="menuitem" onClick={() => setMenuAbierto(false)}><IconSettings /> Ajustes</a>
              {isAdmin ? (
                <a href="#/admin" role="menuitem" onClick={() => setMenuAbierto(false)}><IconShield /> Administración</a>
              ) : null}
              <div className="menu-cuenta-linea" />
              <div className="menu-cuenta-tema">
                <IconMoon />
                <span>Tema</span>
                <ThemeToggle compact />
              </div>
              <button
                type="button"
                role="menuitem"
                className="peligro"
                onClick={() => { setMenuAbierto(false); logout(); }}
              >
                <IconLogout /> Cerrar sesión
              </button>
            </div>
          ) : null}
        </div>
      </div>
    </header>
  );
}

/** Accesos rápidos (solo móvil): se despliega al pulsar la cuadrícula. */
export function AccesosRapidos() {
  const { user, isAdmin } = useAuth();
  const [abierto, setAbierto] = useState(false);
  if (!user) return null;
  return (
    <>
      <button
        type="button"
        className="solo-movil icono-redondo"
        onClick={() => setAbierto(true)}
        aria-label="Más secciones"
      >
        <IconGrid />
      </button>
      {abierto ? (
        <div className="hoja-accesos" role="dialog" aria-label="Más secciones">
          <button type="button" className="velo" onClick={() => setAbierto(false)} aria-label="Cerrar" />
          <div className="hoja-cuerpo">
            <span className="eyebrow">Ir a</span>
            <div className="hoja-rejilla">
              <a href="#/grupos" onClick={() => setAbierto(false)}><IconLayers /> Grupos</a>
              <a href="#/amigos" onClick={() => setAbierto(false)}><IconUsers /> Contactos</a>
              <a href={`#/user/${user.id}`} onClick={() => setAbierto(false)}><IconUser /> Mi perfil</a>
              <a href="#/profile/saved" onClick={() => setAbierto(false)}><IconBookmark /> Guardados</a>
              <a href="#/settings" onClick={() => setAbierto(false)}><IconSettings /> Ajustes</a>
              {isAdmin ? <a href="#/admin" onClick={() => setAbierto(false)}><IconShield /> Administración</a> : null}
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
