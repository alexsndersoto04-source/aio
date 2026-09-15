// Moon — Un grupo
// ============================================================
// La portada del grupo, quién está dentro y dos pestañas: lo que se publica
// (con los mismos me gusta, comentarios y guardados de siempre) y el chat
// entre los miembros, con texto y notas de voz, en vivo.
//
// Al tocar un aviso del chat se abre directamente en la pestaña del chat:
// la dirección llega con «?chat» al final.

import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../api.js';
import { realtime } from '../realtime.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError, confirmar } from '../ui.js';
import Composer from '../components/Composer.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { PostSkeleton } from '../components/Skeleton.jsx';
import { IconUsers, IconPlus, IconCheck, IconSettings, IconLayers, IconChat } from '../components/Icons.jsx';
import ChatGrupo from '../components/ChatGrupo.jsx';

export default function GrupoView({ id }) {
  const { user } = useAuth();
  const [grupo, setGrupo] = useState(null);
  const [posts, setPosts] = useState([]);
  const [cargando, setCargando] = useState(true);
  const [error, setError] = useState('');
  // 'publicaciones' o 'chat'; si la dirección trae «?chat» se abre el chat.
  const [vista, setVista] = useState(() => (/[?&]chat/.test(window.location.hash) ? 'chat' : 'publicaciones'));
  // Aviso de «hay mensajes nuevos en el chat» cuando estás en Publicaciones.
  const [chatNuevo, setChatNuevo] = useState(false);
  const [ocupado, setOcupado] = useState(false);

  const cargar = useCallback(async () => {
    setCargando(true);
    try {
      const g = await api.get(`/api/groups/${id}`);
      setGrupo(g);
      try {
        const feed = await api.get(`/api/groups/${id}/posts?page=1&limit=20`);
        setPosts(feed.items || []);
      } catch (e) {
        // Grupo privado al que todavía no pertenezco: no es un error, es un aviso.
        setPosts([]);
      }
    } catch (e) {
      setError(e.message || 'No se pudo abrir el grupo');
    } finally {
      setCargando(false);
    }
  }, [id]);

  useEffect(() => { cargar(); }, [cargar]);

  // ¿Hay algo nuevo en el chat del grupo que no haya visto en este teléfono?
  useEffect(() => {
    if (!grupo?.soy_miembro) return undefined;
    let vivo = true;
    const marca = () => Number(localStorage.getItem(`moon_chat_visto_${grupo.id}`) || 0);
    api.get(`/api/groups/${grupo.id}/messages?limite=1`)
      .then((r) => {
        // La lista llega del más viejo al más nuevo: el último es el más reciente.
        const lista = r?.mensajes || [];
        const ultimo = Number(lista[lista.length - 1]?.id || 0);
        if (vivo && vista !== 'chat') setChatNuevo(ultimo > marca());
      })
      .catch(() => {});
    const off = realtime.on((ev) => {
      if (Number(ev.group_id) !== Number(grupo.id)) return;
      if (ev.type === 'group_message') setChatNuevo(Number(ev.message?.user_id) !== Number(user?.id));
      if (ev.type === 'group_message_deleted') setChatNuevo(false);
    });
    return () => { vivo = false; off(); };
  }, [grupo?.id, grupo?.soy_miembro, vista]);

  /** El chat avisa de por dónde va para no repetir el aviso en cada visita. */
  function marcarChatVisto(ultimoId) {
    if (!grupo?.id || !ultimoId) return;
    try { localStorage.setItem(`moon_chat_visto_${grupo.id}`, String(ultimoId)); } catch { /* sin almacén */ }
    setChatNuevo(false);
  }

  async function entrarOSalir() {
    if (!grupo || ocupado) return;
    setOcupado(true);
    try {
      if (grupo.soy_miembro) {
        const ok = await confirmar({
          title: '¿Salir del grupo?',
          message: `Dejarás de ver lo que se publica en «${grupo.name}».`,
          confirmText: 'Salir',
          danger: true,
        });
        if (!ok) return;
        await api.del(`/api/groups/${grupo.id}/join`);
        toast.info('Has salido del grupo');
      } else {
        await api.post(`/api/groups/${grupo.id}/join`, {});
        toast.ok('Ya estás dentro');
      }
      cargar();
    } catch (e) {
      avisoError(e);
    } finally {
      setOcupado(false);
    }
  }

  if (cargando) return <PostSkeleton etiqueta="Abriendo el grupo…" alto="40vh" />;

  if (error || !grupo) {
    return (
      <div className="empty">
        <IconLayers />
        <h3>{error || 'Este grupo no existe'}</h3>
        <p>Puede que se haya eliminado o que el enlace esté mal escrito.</p>
        <a className="btn btn-aurora" href="#/grupos">Ver todos los grupos</a>
      </div>
    );
  }

  return (
    <>
      <header className="portada-grupo">
        <div className="cubierta-grupo">
          {grupo.cover_url ? <img src={grupo.cover_url} alt="" /> : <span className="trama" aria-hidden="true" />}
        </div>
        <div className="datos-grupo">
          <h1>{grupo.name}</h1>
          <p className="muted">
            <IconUsers /> {grupo.miembros} {grupo.miembros === 1 ? 'miembro' : 'miembros'}
            {grupo.privacy === 'private' ? ' · grupo privado' : ' · grupo abierto'}
            {' · '}
            <a href={`#/user/${grupo.owner_username}`}>creado por {grupo.owner_display_name || grupo.owner_username}</a>
          </p>
          {grupo.about ? <p className="sobre">{grupo.about}</p> : null}
          <div className="acciones-grupo">
            <button
              type="button"
              className={`btn ${grupo.soy_miembro ? 'btn-outline' : 'btn-aurora'}`}
              onClick={entrarOSalir}
              disabled={ocupado}
            >
              {grupo.soy_miembro ? (<><IconCheck /> Estás dentro</>) : (<><IconPlus /> Entrar al grupo</>)}
            </button>
            <a className="btn btn-ghost" href="#/grupos">Todos los grupos</a>
          </div>
        </div>
      </header>

      <div className="cuerpo-grupo">
        <div className="columna-grupo">
          <div className="pestanas-grupo" role="tablist" aria-label="Secciones del grupo">
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'publicaciones'}
              className={vista === 'publicaciones' ? 'activa' : ''}
              onClick={() => setVista('publicaciones')}
            >
              <IconLayers /> Publicaciones
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'chat'}
              className={vista === 'chat' ? 'activa' : ''}
              onClick={() => setVista('chat')}
            >
              <IconChat /> Chat
              {chatNuevo ? <span className="punto-nuevo" aria-label="mensajes nuevos" /> : null}
            </button>
          </div>

          {vista === 'publicaciones' ? (grupo.soy_miembro ? (
            <Composer
              destino={`/api/groups/${grupo.id}/posts`}
              placeholder={`Comparte algo con ${grupo.name}…`}
              etiquetaBoton="Publicar"
              onCreated={(post) => setPosts((prev) => [post, ...prev])}
            />
          ) : (
            <div className="aviso-grupo">
              <IconUsers />
              <p>
                {grupo.privacy === 'private'
                  ? 'Este grupo es privado: entra para ver y compartir lo que se publica.'
                  : 'Entra al grupo para publicar y participar.'}
              </p>
              <button type="button" className="btn btn-aurora btn-sm" onClick={entrarOSalir} disabled={ocupado}>
                Entrar al grupo
              </button>
            </div>
          )) : null}

          {vista === 'chat' ? (
            <ChatGrupo
              grupo={grupo}
              esMiembro={!!grupo.soy_miembro}
              onNecesitaEntrar={() => setVista('publicaciones')}
              onVistos={marcarChatVisto}
            />
          ) : posts.length === 0 ? (
            <div className="empty">
              <IconLayers />
              <h3>Aquí todavía no ha publicado nadie</h3>
              <p>{grupo.soy_miembro ? 'Escribe la primera publicación del grupo.' : 'Cuando entres, podrás ser tú quien empiece.'}</p>
            </div>
          ) : (
            posts.map((p) => <PostCard key={p.id} post={p} onChanged={() => cargar()} />)
          )}
        </div>

        <aside className="aside-grupo">
          <section className="tarjeta-rail">
            <h3>Miembros · {grupo.miembros}</h3>
            <ul className="lista-miembros">
              {grupo.miembros_lista.map((m) => (
                <li key={m.id}>
                  <a href={`#/user/${m.id}`}>
                    <Avatar user={m} size="sm" />
                    <span className="quien">
                      <b>
                        {m.display_name}
                        <VerifiedBadge show={m.is_verified} />
                      </b>
                      <small>
                        @{m.username}
                        {m.papel === 'owner' ? ' · lo creó' : m.papel === 'admin' ? ' · administra' : ''}
                      </small>
                    </span>
                  </a>
                </li>
              ))}
            </ul>
          </section>

          {grupo.es_mio ? (
            <section className="tarjeta-rail">
              <h3>
                <IconSettings /> Lo que puedes hacer
              </h3>
              <p className="muted small">
                Eres quien creó este grupo. Puedes cambiar su descripción cuando quieras.
              </p>
              <button
                type="button"
                className="btn btn-outline btn-sm btn-block"
                onClick={async () => {
                  const { pedirTexto } = await import('../ui.js');
                  const texto = await pedirTexto({
                    title: 'Descripción del grupo',
                    label: 'Cuéntale a la gente de qué va',
                    value: grupo.about || '',
                    confirmText: 'Guardar',
                    requerido: false,
                  });
                  if (texto === null) return;
                  try {
                    await api.patch(`/api/groups/${grupo.id}`, { about: texto || '' });
                    toast.ok('Descripción guardada');
                    cargar();
                  } catch (e) {
                    avisoError(e);
                  }
                }}
              >
                Cambiar la descripción
              </button>
            </section>
          ) : null}
        </aside>
      </div>
    </>
  );
}
