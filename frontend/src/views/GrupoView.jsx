// Moon — Un grupo
// ============================================================
// La portada del grupo, quién está dentro y dos pestañas: lo que se publica
// (con los mismos me gusta, comentarios y guardados de siempre) y el chat
// entre los miembros, con texto y notas de voz, en vivo.
//
// Al tocar un aviso del chat se abre directamente en la pestaña del chat:
// la dirección llega con «?chat» al final.

import React, { useCallback, useEffect, useState } from 'react';
import { api, uploadMedia } from '../api.js';
import { realtime } from '../realtime.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError, confirmar } from '../ui.js';
import Composer from '../components/Composer.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { PostSkeleton } from '../components/Skeleton.jsx';
import {
  IconUsers, IconPlus, IconCheck, IconSettings, IconLayers, IconChat, IconMore,
  IconEdit, IconLink, IconTrash, IconLogout, IconImage, IconShield,
  IconCalendar, IconFile,
} from '../components/Icons.jsx';
import ChatGrupo from '../components/ChatGrupo.jsx';
import {
  AnuncioGrupo, EventosGrupo, ArchivosGrupo, ReglasGrupo,
  SolicitudesGrupo, SancionesGrupo, HojaEntrar,
} from '../components/GrupoExtra.jsx';

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
  // Menú de opciones del grupo (editar, portada, enlace, salir, eliminar).
  const [menu, setMenu] = useState(false);
  const [ocupadoMenu, setOcupadoMenu] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  // Hoja de preguntas para entrar (solo cuando el grupo las pide).
  const [preguntando, setPreguntando] = useState(false);

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
      } else if ((grupo.join_questions || []).length > 0) {
        // El grupo pide explicaciones: se contestan antes de entrar.
        setPreguntando(true);
        return;
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

  async function editarNombre() {
    setMenu(false);
    const { pedirTexto } = await import('../ui.js');
    const nombre = await pedirTexto({
      title: 'Nombre del grupo',
      label: 'Cómo se llama',
      value: grupo.name || '',
      confirmText: 'Siguiente',
    });
    if (nombre === null) return;
    const sobre = await pedirTexto({
      title: 'Descripción del grupo',
      label: 'De qué va el grupo',
      value: grupo.about || '',
      confirmText: 'Guardar',
      requerido: false,
    });
    if (sobre === null) return;
    setOcupadoMenu(true);
    try {
      await api.patch(`/api/groups/${grupo.id}`, { name: nombre, about: sobre || '' });
      toast.ok('Grupo actualizado');
      cargar();
    } catch (e) {
      avisoError(e);
    } finally {
      setOcupadoMenu(false);
    }
  }

  async function cambiarPrivacidad(g) {
    setMenu(false);
    const nuevo = g.privacy === 'private' ? 'public' : 'private';
    setOcupadoMenu(true);
    try {
      await api.patch(`/api/groups/${g.id}`, { privacy: nuevo });
      toast.ok(nuevo === 'private' ? 'Ahora el grupo es privado' : 'Ahora el grupo es abierto');
      cargar();
    } catch (e) {
      avisoError(e);
    } finally {
      setOcupadoMenu(false);
    }
  }

  async function cambiarPortada() {
    setMenu(false);
    const entrada = document.createElement('input');
    entrada.type = 'file';
    entrada.accept = 'image/*';
    entrada.onchange = async () => {
      const archivo = entrada.files && entrada.files[0];
      if (!archivo) return;
      setOcupadoMenu(true);
      try {
        const subida = await uploadMedia('post', archivo);
        await api.patch(`/api/groups/${grupo.id}`, { cover_url: subida.url });
        toast.ok('Portada cambiada');
        cargar();
      } catch (e) {
        avisoError(e);
      } finally {
        setOcupadoMenu(false);
      }
    };
    entrada.click();
  }

  async function eliminarGrupo() {
    setMenu(false);
    const ok = await confirmar({
      title: `¿Eliminar «${grupo.name}»?`,
      message: 'Se borran el grupo, sus publicaciones y su chat. No se puede deshacer.',
      confirmText: 'Eliminar el grupo',
      danger: true,
    });
    if (!ok) return;
    setOcupadoMenu(true);
    try {
      await api.del(`/api/groups/${grupo.id}`);
      toast.ok('Grupo eliminado');
      window.location.hash = '#/grupos';
    } catch (e) {
      avisoError(e);
      setOcupadoMenu(false);
    }
  }

  /** Panel de ajustes del grupo: todo lo configurable, en un solo sitio. */
  function PanelAjustes() {
    const esMio = !!grupo.es_mio;
    const [nombre, setNombre] = useState(grupo.name || '');
    const [sobre, setSobre] = useState(grupo.about || '');
    const [privacidad, setPrivacidad] = useState(grupo.privacy || 'public');
    const [guardando, setGuardando] = useState(false);
    const [menuMiembro, setMenuMiembro] = useState(null);

    async function guardar(e) {
      e.preventDefault();
      if (!esMio) return;
      setGuardando(true);
      try {
        await api.patch(`/api/groups/${grupo.id}`, { name: nombre, about: sobre, privacy: privacidad });
        toast.ok('Grupo actualizado');
        cargar();
      } catch (err) {
        avisoError(err);
      } finally {
        setGuardando(false);
      }
    }

    async function sacar(m) {
      const ok = await confirmar({
        title: `¿Sacar a ${m.display_name} del grupo?`,
        message: 'Podrá volver a entrar cuando quiera si el grupo es abierto.',
        confirmText: 'Sacar del grupo',
        danger: true,
      });
      if (!ok) return;
      try {
        await api.del(`/api/groups/${grupo.id}/members/${m.id}`);
        toast.ok('Miembro fuera del grupo');
        setMenuMiembro(null);
        cargar();
      } catch (err) {
        avisoError(err);
      }
    }

    async function darMando(m, papel) {
      try {
        await api.post(`/api/groups/${grupo.id}/members/${m.id}/role`, { papel });
        toast.ok(papel === 'admin' ? 'Ahora administra el grupo' : 'Ya no administra el grupo');
        setMenuMiembro(null);
        cargar();
      } catch (err) {
        avisoError(err);
      }
    }

    return (
      <div className="panel-ajustes-grupo">
        <section className="card ajustes-bloque">
          <div className="titulo">Cómo se ve el grupo</div>
          {esMio ? (
            <form onSubmit={guardar}>
              <label className="campo">
                <span>Nombre</span>
                <input className="input" value={nombre} onChange={(e) => setNombre(e.target.value)} maxLength={80} />
              </label>
              <label className="campo">
                <span>Descripción</span>
                <textarea
                  className="textarea"
                  value={sobre}
                  onChange={(e) => setSobre(e.target.value)}
                  maxLength={400}
                  rows={3}
                  placeholder="Cuéntale a la gente de qué va el grupo"
                />
              </label>
              <label className="campo">
                <span>Tipo de grupo</span>
                <div className="tabs">
                  <button
                    type="button"
                    className={privacidad === 'public' ? 'active' : ''}
                    onClick={() => setPrivacidad('public')}
                  >
                    Abierto
                  </button>
                  <button
                    type="button"
                    className={privacidad === 'private' ? 'active' : ''}
                    onClick={() => setPrivacidad('private')}
                  >
                    Privado
                  </button>
                </div>
              </label>
              <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
                <button className="btn btn-primary" disabled={guardando}>
                  {guardando ? 'Guardando…' : 'Guardar cambios'}
                </button>
                <button type="button" className="btn btn-outline" onClick={cambiarPortada} disabled={ocupadoMenu}>
                  <IconImage /> Cambiar la portada
                </button>
              </div>
            </form>
          ) : (
            <p className="muted small">Eres miembro del grupo. Quien lo creó es quien puede cambiar su nombre, su descripción y su portada.</p>
          )}
        </section>

        <section className="card ajustes-bloque">
          <div className="titulo">Invitar</div>
          <div className="fila-ajuste">
            <span className="icono"><IconLink /></span>
            <span className="texto">
              <b>Enlace del grupo</b>
              <small>Compártelo para que entren directo.</small>
            </span>
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={async () => {
                const enlace = `${window.location.origin}/#/grupo/${grupo.id}`;
                try { await navigator.clipboard.writeText(enlace); toast.ok('Enlace copiado'); }
                catch { toast.info(enlace); }
              }}
            >
              Copiar
            </button>
          </div>
        </section>

        <section className="card ajustes-bloque">
          <div className="titulo">Miembros · {grupo.miembros}</div>
          {(grupo.miembros_lista || []).map((m) => (
            <div className="fila-ajuste" key={m.id}>
              <Avatar user={m} size="sm" />
              <span className="texto">
                <b>{m.display_name}<VerifiedBadge show={m.is_verified} /></b>
                <small>
                  @{m.username}
                  {m.papel === 'owner' ? ' · lo creó' : m.papel === 'admin' ? ' · administra' : ''}
                </small>
              </span>
              {esMio && m.papel !== 'owner' ? (
                <div className="menu-grupo-caja">
                  <button
                    type="button"
                    className="icon-btn"
                    onClick={() => setMenuMiembro(menuMiembro === m.id ? null : m.id)}
                    aria-label={`Opciones de ${m.display_name}`}
                  >
                    <IconMore />
                  </button>
                  {menuMiembro === m.id ? (
                    <>
                      <span className="hoja-fondo" role="presentation" onClick={() => setMenuMiembro(null)} />
                      <div className="menu-conv menu-grupo" role="menu">
                        {m.papel === 'admin' ? (
                          <button type="button" onClick={() => darMando(m, 'member')}>
                            <IconUsers /> Quitarle el mando
                          </button>
                        ) : (
                          <button type="button" onClick={() => darMando(m, 'admin')}>
                            <IconShield /> Darle el mando del grupo
                          </button>
                        )}
                        <button type="button" className="peligro" onClick={() => sacar(m)}>
                          <IconTrash /> Sacar del grupo
                        </button>
                      </div>
                    </>
                  ) : null}
                </div>
              ) : null}
            </div>
          ))}
        </section>

        {grupo.mando ? (
          <section className="card ajustes-bloque">
            <div className="titulo">Quién entra y quién cumple</div>
            <SolicitudesGrupo grupoId={grupo.id} />
            <SancionesGrupo grupoId={grupo.id} miembros={grupo.miembros_lista} />
          </section>
        ) : null}

        <section className="card ajustes-bloque">
          <div className="titulo">Zona sensible</div>
          {grupo.soy_miembro && !esMio ? (
            <div className="fila-ajuste peligro">
              <span className="icono"><IconLogout /></span>
              <span className="texto">
                <b>Salir del grupo</b>
                <small>Dejarás de ver sus publicaciones y su chat.</small>
              </span>
              <button type="button" className="btn btn-danger btn-sm" onClick={entrarOSalir} disabled={ocupado}>
                Salir
              </button>
            </div>
          ) : null}
          {esMio ? (
            <div className="fila-ajuste peligro">
              <span className="icono"><IconTrash /></span>
              <span className="texto">
                <b>Eliminar el grupo</b>
                <small>Se borran el grupo, sus publicaciones y su chat. No se puede deshacer.</small>
              </span>
              <button type="button" className="btn btn-danger btn-sm" onClick={eliminarGrupo} disabled={ocupadoMenu}>
                Eliminar
              </button>
            </div>
          ) : null}
        </section>
      </div>
    );
  }

  /** Botón «…» del grupo: el menú con todo lo que se puede hacer. */
  function OpcionesGrupo({ grupo: g }) {
    const esMio = !!g.es_mio;
    return (
      <div className="menu-grupo-caja">
        <button
          type="button"
          className="icon-btn"
          onClick={() => setMenu((v) => !v)}
          aria-expanded={menu}
          aria-label="Opciones del grupo"
          title="Opciones del grupo"
        >
          <IconMore />
        </button>
        {menu ? (
          <>
            <span className="hoja-fondo" role="presentation" onClick={() => setMenu(false)} />
            <div className="menu-conv menu-grupo" role="menu">
            {esMio ? (
              <button type="button" onClick={editarNombre} disabled={ocupadoMenu}>
                <IconEdit /> Cambiar el nombre y la descripción
              </button>
            ) : null}
            {esMio ? (
              <button type="button" onClick={cambiarPortada} disabled={ocupadoMenu}>
                <IconImage /> Cambiar la portada
              </button>
            ) : null}
            {esMio ? (
              <button type="button" onClick={() => cambiarPrivacidad(g)} disabled={ocupadoMenu}>
                <IconSettings />
                {g.privacy === 'private' ? 'Hacerlo abierto' : 'Hacerlo privado'}
              </button>
            ) : null}
            <button
              type="button"
              onClick={async () => {
                setMenu(false);
                const enlace = `${window.location.origin}/#/grupo/${g.id}`;
                try { await navigator.clipboard.writeText(enlace); toast.ok('Enlace del grupo copiado'); }
                catch { toast.info(enlace); }
              }}
            >
              <IconLink /> Copiar el enlace para invitar
            </button>
            {g.soy_miembro ? (
              <button type="button" onClick={async () => { setMenu(false); await entrarOSalir(); }} disabled={ocupado}>
                <IconLogout /> Salir del grupo
              </button>
            ) : null}
            {esMio ? (
              <button type="button" className="peligro" onClick={eliminarGrupo} disabled={ocupadoMenu}>
                <IconTrash /> Eliminar el grupo
              </button>
            ) : null}
            </div>
          </>
        ) : null}
      </div>
    );
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
            <OpcionesGrupo grupo={grupo} />
          </div>
        </div>
      </header>

      {grupo.announcement ? (
        <AnuncioGrupo
          texto={grupo.announcement}
          cuando={grupo.announcement_at}
          puedoQuitar={!!grupo.mando}
          onQuitar={async () => {
            try {
              const r = await api.patch(`/api/groups/${grupo.id}/rules`, { announcement: '' });
              setGrupo((g) => ({ ...g, announcement: r.announcement, announcement_at: r.announcement_at }));
              toast.ok('Anuncio quitado');
            } catch (e) { avisoError(e); }
          }}
        />
      ) : null}

      <div className="cuerpo-grupo">
        <div className="columna-grupo">
          <div className="pestanas-grupo" role="tablist" aria-label="Secciones del grupo">
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'publicaciones'}
              className={vista === 'publicaciones' ? 'activa' : ''}
              onClick={(e) => { setVista('publicaciones'); e.currentTarget.scrollIntoView({ block: 'nearest', inline: 'center' }); }}
            >
              <IconLayers /> Publicaciones
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'chat'}
              className={vista === 'chat' ? 'activa' : ''}
              onClick={(e) => { setVista('chat'); e.currentTarget.scrollIntoView({ block: 'nearest', inline: 'center' }); }}
            >
              <IconChat /> Chat
              {chatNuevo ? <span className="punto-nuevo" aria-label="mensajes nuevos" /> : null}
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'eventos'}
              className={vista === 'eventos' ? 'activa' : ''}
              onClick={(e) => { setVista('eventos'); e.currentTarget.scrollIntoView({ block: 'nearest', inline: 'center' }); }}
            >
              <IconCalendar /> Eventos
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'archivos'}
              className={vista === 'archivos' ? 'activa' : ''}
              onClick={(e) => { setVista('archivos'); e.currentTarget.scrollIntoView({ block: 'nearest', inline: 'center' }); }}
            >
              <IconFile /> Archivos
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'reglas'}
              className={vista === 'reglas' ? 'activa' : ''}
              onClick={(e) => { setVista('reglas'); e.currentTarget.scrollIntoView({ block: 'nearest', inline: 'center' }); }}
            >
              <IconShield /> Reglas
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={vista === 'ajustes'}
              className={vista === 'ajustes' ? 'activa' : ''}
              onClick={(e) => { setVista('ajustes'); e.currentTarget.scrollIntoView({ block: 'nearest', inline: 'center' }); }}
            >
              <IconSettings /> Ajustes
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

          {vista === 'ajustes' ? (
            <PanelAjustes />
          ) : vista === 'eventos' ? (
            grupo.soy_miembro || grupo.privacy !== 'private' ? (
              <EventosGrupo grupoId={grupo.id} mando={!!grupo.mando} soyMiembro={!!grupo.soy_miembro} />
            ) : (
              <div className="aviso-grupo">
                <IconCalendar />
                <p>Este grupo es privado: entra para ver y crear eventos.</p>
                <button type="button" className="btn btn-aurora btn-sm" onClick={entrarOSalir} disabled={ocupado}>
                  Entrar al grupo
                </button>
              </div>
            )
          ) : vista === 'archivos' ? (
            grupo.soy_miembro || grupo.privacy !== 'private' ? (
              <ArchivosGrupo grupoId={grupo.id} soyMiembro={!!grupo.soy_miembro} />
            ) : (
              <div className="aviso-grupo">
                <IconFile />
                <p>Entra al grupo para compartir y descargar archivos.</p>
                <button type="button" className="btn btn-aurora btn-sm" onClick={entrarOSalir} disabled={ocupado}>
                  Entrar al grupo
                </button>
              </div>
            )
          ) : vista === 'reglas' ? (
            <ReglasGrupo
              grupoId={grupo.id}
              grupo={grupo}
              mando={!!grupo.mando}
              onCambio={(r) => setGrupo((g) => ({ ...g, ...r }))}
            />
          ) : vista === 'chat' ? (
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

      {preguntando ? (
        <HojaEntrar
          grupo={grupo}
          onCerrar={() => setPreguntando(false)}
          onDentro={() => cargar()}
        />
      ) : null}
    </>
  );
}
