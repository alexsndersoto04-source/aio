// Moon — Contactos
// ============================================================
// La página de personas: quién está conectado ahora mismo (presencia real
// del WebSocket) y el resto de tus contactos, con acceso directo al chat.

import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../api.js';
import { avisoError } from '../ui.js';
import { realtime } from '../realtime.js';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { ListSkeleton } from '../components/Skeleton.jsx';
import { IconMail, IconUsers } from '../components/Icons.jsx';

export default function ContactosView() {
  const [datos, setDatos] = useState(null);
  const [filtro, setFiltro] = useState('');

  const cargar = useCallback(() => {
    api.get('/api/users/presence').then(setDatos).catch(() => setDatos({ en_linea: [], otros: [] }));
  }, []);

  useEffect(() => {
    cargar();
    const off = realtime.on((ev) => {
      if (ev.type === 'presence') cargar();
    });
    return off;
  }, [cargar]);

  async function abrirChat(usuario) {
    try {
      const res = await api.post('/api/messages/conversations', { user_id: usuario.id });
      window.location.hash = `#/messages/${res.conversation_id}`;
    } catch (e) {
      avisoError(e);
    }
  }

  const filtrar = (lista) =>
    lista.filter((u) =>
      !filtro ||
      u.username.toLowerCase().includes(filtro.toLowerCase()) ||
      (u.display_name || '').toLowerCase().includes(filtro.toLowerCase())
    );

  const fila = (u) => (
    <li key={u.id} className="contacto-fila">
      <a href={`#/user/${u.id}`} className="quien">
        <span className={`punto-estado${u.online ? ' en-linea' : ''}`} aria-hidden="true" />
        <Avatar user={{ username: u.username, display_name: u.display_name, avatar_url: u.avatar_url }} size="md" />
        <span className="datos">
          <b>
            {u.display_name || u.username}
            <VerifiedBadge show={u.is_verified} />
          </b>
          <small>@{u.username}{u.online ? ' · en línea' : ''}</small>
        </span>
      </a>
      <div className="acciones">
        <a className="btn btn-outline btn-sm" href={`#/user/${u.id}`}>Ver perfil</a>
        <button type="button" className="btn btn-primary btn-sm" onClick={() => abrirChat(u)}>
          <IconMail /> Mensaje
        </button>
      </div>
    </li>
  );

  return (
    <>
      <div className="topbar">
        <h1>Contactos</h1>
      </div>

      {datos === null ? (
        <ListSkeleton etiqueta="Cargando tus contactos…" />
      ) : (
        <>
          <div className="barra-filtro">
            <input
              className="input"
              type="search"
              value={filtro}
              onChange={(e) => setFiltro(e.target.value)}
              placeholder="Buscar entre tus contactos"
              aria-label="Buscar entre tus contactos"
            />
          </div>

          {filtrar(datos.en_linea).length > 0 ? (
            <section className="bloque-contactos">
              <p className="eyebrow-linea">En línea ahora · {filtrar(datos.en_linea).length}</p>
              <ul className="lista-contactos-pagina">{filtrar(datos.en_linea).map(fila)}</ul>
            </section>
          ) : null}

          {filtrar(datos.otros).length > 0 ? (
            <section className="bloque-contactos">
              <p className="eyebrow-linea">{datos.en_linea.length > 0 ? 'Otros contactos' : 'Tus contactos'}</p>
              <ul className="lista-contactos-pagina">{filtrar(datos.otros).map(fila)}</ul>
            </section>
          ) : null}

          {filtrar(datos.en_linea).length === 0 && filtrar(datos.otros).length === 0 ? (
            <div className="empty">
              <IconUsers />
              <h3>Sin contactos todavía</h3>
              <p>Cuando sigas a alguien o alguien te siga, aparecerá aquí. También puedes buscarlos en Explorar.</p>
              <a className="btn btn-primary" href="#/explore">Explorar personas</a>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
