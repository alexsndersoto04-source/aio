// Moon — Un grupo, pantalla por pantalla
// ============================================================
// La portada del grupo muestra tarjetas independientes, una por sección.
// Cada sección (publicaciones, chat, eventos, archivos, miembros, reglas y
// ajustes) se abre en su propia pantalla completa, de igual a igual, con su
// cabecera y su espacio propio. Sin pestañas que sustituyen contenido, sin
// rail lateral y sin menús en capas.

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
  IconTrash, IconLogout, IconImage, IconShield, IconCalendar, IconFile,
  IconChevronLeft, IconLink,
} from '../components/Icons.jsx';
import ChatGrupo, { AjustesChatGrupo } from '../components/ChatGrupo.jsx';
import {
  AnuncioGrupo, EventosGrupo, ArchivosGrupo, ReglasGrupo,
  SolicitudesGrupo, SancionesGrupo, HojaEntrar,
} from '../components/GrupoExtra.jsx';

const SECCIONES = [
  'publicaciones', 'chat', 'eventos', 'archivos', 'miembros', 'reglas', 'ajustes',
];

export default function GrupoView({ id, seccion, sub }) {
  const { user } = useAuth();
  const [grupo, setGrupo] = useState(null);
  const [posts, setPosts] = useState([]);
  const [cargando, setCargando] = useState(true);
  const [error, setError] = useState('');
  const [chatNuevo, setChatNuevo] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [preguntando, setPreguntando] = useState(false);

  // Enlace viejo «?chat»: se respeta llevando a la pantalla del chat.
  useEffect(() => {
    if (!seccion && /[?&]chat/.test(window.location.hash)) {
      window.location.hash = `#/grupo/${id}/chat`;
    }
  }, [id, seccion]);

  const cargar = useCallback(async () => {
    setCargando(true);
    try {
      const g = await api.get(`/api/groups/${id}`);
      setGrupo(g);
      try {
        const feed = await api.get(`/api/groups/${id}/posts?page=1&limit=20`);
        setPosts(feed.items || []);
      } catch (e) {
        setPosts([]);
      }
    } catch (e) {
      setError(e.message || 'No se pudo abrir el grupo');
    } finally {
      setCargando(false);
    }
  }, [id]);

  useEffect(() => { cargar(); }, [cargar]);

  // Punto de «mensajes nuevos» para la tarjeta del chat en la portada.
  useEffect(() => {
    if (!grupo?.soy_miembro) return undefined;
    let vivo = true;
    const marca = () => Number(localStorage.getItem(`moon_chat_visto_${grupo.id}`) || 0);
    api.get(`/api/groups/${grupo.id}/messages?limite=1`)
      .then((r) => {
        const lista = r?.mensajes || [];
        const ultimo = Number(lista[lista.length - 1]?.id || 0);
        if (vivo) setChatNuevo(ultimo > marca());
      })
      .catch(() => {});
    const off = realtime.on((ev) => {
      if (Number(ev.group_id) !== Number(grupo.id)) return;
      if (ev.type === 'group_message') setChatNuevo(Number(ev.message?.user_id) !== Number(user?.id));
      if (ev.type === 'group_message_deleted') setChatNuevo(false);
    });
    return () => { vivo = false; off(); };
  }, [grupo?.id, grupo?.soy_miembro]);

  function marcarChatVisto(ultimoId) {
    if (!grupo?.id || ultimoId) {
      try { localStorage.setItem(`moon_chat_visto_${grupo.id}`, String(ultimoId)); } catch { /* sin almacén */ }
    }
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

  const activa = SECCIONES.includes(seccion) ? seccion : null;

  return (
    <>
      {activa ? (
        <PantallaSeccion
          grupo={grupo}
          seccion={activa}
          sub={sub}
          posts={posts}
          cargar={cargar}
          entrarOSalir={entrarOSalir}
          ocupado={ocupado}
          chatNuevo={chatNuevo}
          marcarChatVisto={marcarChatVisto}
        />
      ) : (
        <PortadaGrupo
          grupo={grupo}
          chatNuevo={chatNuevo}
          entrarOSalir={entrarOSalir}
          ocupado={ocupado}
        />
      )}

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

/* ---------------------------------------------------------------- portada */

function PortadaGrupo({ grupo, chatNuevo, entrarOSalir, ocupado }) {
  const anticipo = (grupo.miembros_lista || []).slice(0, 3);
  const tarjetas = [
    {
      id: 'publicaciones', icono: <IconLayers />, titulo: 'Publicaciones',
      detalle: 'Lo que se comparte en el grupo, con sus me gusta y comentarios.',
    },
    {
      id: 'chat', icono: <IconChat />, titulo: 'Chat',
      detalle: 'Mensajes y notas de voz entre los miembros, en vivo.',
      punto: chatNuevo,
    },
    {
      id: 'eventos', icono: <IconCalendar />, titulo: 'Eventos',
      detalle: 'Quedadas y citas del grupo, con fecha y lugar.',
    },
    {
      id: 'archivos', icono: <IconFile />, titulo: 'Archivos',
      detalle: 'Los documentos y archivos compartidos, en un solo sitio.',
    },
    {
      id: 'miembros', icono: <IconUsers />, titulo: 'Miembros',
      detalle: `${grupo.miembros} ${grupo.miembros === 1 ? 'persona' : 'personas'} dentro del grupo.`,
      caras: anticipo,
    },
    {
      id: 'reglas', icono: <IconShield />, titulo: 'Reglas y anuncio',
      detalle: grupo.announcement ? 'Hay un anuncio fijado para el grupo.' : 'Las normas que rigen el grupo.',
    },
    {
      id: 'ajustes', icono: <IconSettings />, titulo: 'Ajustes del grupo',
      detalle: 'Nombre, portada, privacidad, miembros y zona sensible.',
    },
  ];

  return (
    <>
      <header className="g-portada">
        <div className="g-cubierta">
          {grupo.cover_url ? <img src={grupo.cover_url} alt="" /> : <span className="trama" aria-hidden="true" />}
        </div>
        <div className="g-datos">
          <h1>{grupo.name}</h1>
          <p className="muted">
            <IconUsers /> {grupo.miembros} {grupo.miembros === 1 ? 'miembro' : 'miembros'}
            {grupo.privacy === 'private' ? ' · grupo privado' : ' · grupo abierto'}
            {' · '}
            <a href={`#/user/${grupo.owner_username}`}>creado por {grupo.owner_display_name || grupo.owner_username}</a>
          </p>
          {grupo.about ? <p className="g-sobre">{grupo.about}</p> : null}
          <div className="g-acciones">
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

      <div className="g-cards">
        {tarjetas.map((t) => (
          <a key={t.id} className="g-card" href={`#/grupo/${grupo.id}/${t.id}`}>
            <span className="g-card-ic">{t.icono}</span>
            <span className="g-card-tx">
              <b>{t.titulo}</b>
              <small>{t.detalle}</small>
            </span>
            {t.caras?.length ? (
              <span className="g-card-caras" aria-hidden="true">
                {t.caras.map((m) => <Avatar key={m.id} user={m} size="sm" />)}
              </span>
            ) : null}
            {t.punto ? <span className="punto-nuevo" aria-label="mensajes nuevos" /> : null}
            <IconChevronLeft className="g-card-flecha" />
          </a>
        ))}
      </div>
    </>
  );
}

/* ------------------------------------------------------- pantalla compartida */

function CabGrupo({ grupo, icono, titulo, extra }) {
  return (
    <header className="g-cab">
      <a className="g-volver" href={`#/grupo/${grupo.id}`}>
        <IconChevronLeft /> Volver al grupo
      </a>
      <h2>{icono} {titulo}</h2>
      {extra}
    </header>
  );
}

function AvisoEntrar({ grupo, icono, texto, entrarOSalir, ocupado }) {
  return (
    <div className="aviso-grupo">
      {icono}
      <p>{texto}</p>
      <button type="button" className="btn btn-aurora btn-sm" onClick={entrarOSalir} disabled={ocupado}>
        Entrar al grupo
      </button>
    </div>
  );
}

function PantallaSeccion(props) {
  const { grupo, seccion, sub, posts, cargar, entrarOSalir, ocupado, marcarChatVisto } = props;
  const abiertoOPublico = grupo.soy_miembro || grupo.privacy !== 'private';

  if (seccion === 'publicaciones') {
    return (
      <div className="g-pantalla">
        <CabGrupo grupo={grupo} icono={<IconLayers />} titulo="Publicaciones" />
        {grupo.soy_miembro ? (
          <Composer
            destino={`/api/groups/${grupo.id}/posts`}
            placeholder={`Comparte algo con ${grupo.name}…`}
            etiquetaBoton="Publicar"
            onCreated={(post) => cargar()}
          />
        ) : (
          <AvisoEntrar
            grupo={grupo} icono={<IconUsers />} ocupado={ocupado} entrarOSalir={entrarOSalir}
            texto={grupo.privacy === 'private'
              ? 'Este grupo es privado: entra para ver y compartir lo que se publica.'
              : 'Entra al grupo para publicar y participar.'}
          />
        )}
        {posts.length === 0 ? (
          <div className="empty">
            <IconLayers />
            <h3>Aquí todavía no ha publicado nadie</h3>
            <p>{grupo.soy_miembro ? 'Escribe la primera publicación del grupo.' : 'Cuando entres, podrás ser tú quien empiece.'}</p>
          </div>
        ) : (
          posts.map((p) => <PostCard key={p.id} post={p} onChanged={() => cargar()} />)
        )}
      </div>
    );
  }

  if (seccion === 'chat') {
    if (sub === 'ajustes') {
      return (
        <div className="g-pantalla">
          <CabGrupo grupo={grupo} icono={<IconSettings />} titulo="Ajustes del chat" />
          <AjustesChatGrupo />
        </div>
      );
    }
    return (
      <div className="g-pantalla g-pantalla-chat">
        <CabGrupo
          grupo={grupo}
          icono={<IconChat />}
          titulo="Chat del grupo"
          extra={(
            <a className="g-cab-extra" href={`#/grupo/${grupo.id}/chat/ajustes`}>
              <IconSettings /> Ajustes
            </a>
          )}
        />
        {grupo.soy_miembro ? (
          <ChatGrupo grupo={grupo} esMiembro onVistos={marcarChatVisto} />
        ) : (
          <AvisoEntrar
            grupo={grupo} icono={<IconChat />} ocupado={ocupado} entrarOSalir={entrarOSalir}
            texto="El chat es solo para los miembros del grupo. Entra para participar."
          />
        )}
      </div>
    );
  }

  if (seccion === 'eventos') {
    return (
      <div className="g-pantalla">
        <CabGrupo grupo={grupo} icono={<IconCalendar />} titulo="Eventos" />
        {abiertoOPublico ? (
          <EventosGrupo grupoId={grupo.id} mando={!!grupo.mando} soyMiembro={!!grupo.soy_miembro} />
        ) : (
          <AvisoEntrar
            grupo={grupo} icono={<IconCalendar />} ocupado={ocupado} entrarOSalir={entrarOSalir}
            texto="Este grupo es privado: entra para ver y crear eventos."
          />
        )}
      </div>
    );
  }

  if (seccion === 'archivos') {
    return (
      <div className="g-pantalla">
        <CabGrupo grupo={grupo} icono={<IconFile />} titulo="Archivos" />
        {abiertoOPublico ? (
          <ArchivosGrupo grupoId={grupo.id} soyMiembro={!!grupo.soy_miembro} />
        ) : (
          <AvisoEntrar
            grupo={grupo} icono={<IconFile />} ocupado={ocupado} entrarOSalir={entrarOSalir}
            texto="Entra al grupo para compartir y descargar archivos."
          />
        )}
      </div>
    );
  }

  if (seccion === 'miembros') {
    return (
      <div className="g-pantalla">
        <CabGrupo grupo={grupo} icono={<IconUsers />} titulo={`Miembros · ${grupo.miembros}`} />
        <div className="card seccion-miembros">
          <ul className="lista-miembros-v">
            {(grupo.miembros_lista || []).map((m) => (
              <li key={m.id}>
                <a href={`#/user/${m.id}`}>
                  <Avatar user={m} size="md" />
                  <span className="quien">
                    <b>{m.display_name}<VerifiedBadge show={m.is_verified} /></b>
                    <small>@{m.username}{m.papel === 'owner' ? ' · lo creó' : m.papel === 'admin' ? ' · administra' : ''}</small>
                  </span>
                </a>
              </li>
            ))}
          </ul>
        </div>
      </div>
    );
  }

  if (seccion === 'reglas') {
    return (
      <div className="g-pantalla">
        <CabGrupo grupo={grupo} icono={<IconShield />} titulo="Reglas y anuncio" />
        {grupo.announcement ? (
          <AnuncioGrupo
            texto={grupo.announcement}
            cuando={grupo.announcement_at}
            puedoQuitar={!!grupo.mando}
            onQuitar={async () => {
              try {
                const r = await api.patch(`/api/groups/${grupo.id}/rules`, { announcement: '' });
                cargar();
                toast.ok('Anuncio quitado');
              } catch (e) { avisoError(e); }
            }}
          />
        ) : null}
        <ReglasGrupo grupoId={grupo.id} grupo={grupo} mando={!!grupo.mando} onCambio={() => cargar()} />
      </div>
    );
  }

  // ajustes: área de configuración completa del grupo.
  return (
    <div className="g-pantalla">
      <CabGrupo grupo={grupo} icono={<IconSettings />} titulo="Ajustes del grupo" />
      <AjustesGrupo grupo={grupo} cargar={cargar} entrarOSalir={entrarOSalir} ocupado={ocupado} />
    </div>
  );
}

/* ------------------------------------------- ajustes: configuración completa */

function AjustesGrupo({ grupo, cargar, entrarOSalir, ocupado }) {
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

  async function cambiarPortada() {
    const entrada = document.createElement('input');
    entrada.type = 'file';
    entrada.accept = 'image/*';
    entrada.onchange = async () => {
      const archivo = entrada.files && entrada.files[0];
      if (!archivo) return;
      try {
        const subida = await uploadMedia('post', archivo);
        await api.patch(`/api/groups/${grupo.id}`, { cover_url: subida.url });
        toast.ok('Portada cambiada');
        cargar();
      } catch (e) {
        avisoError(e);
      }
    };
    entrada.click();
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

  async function eliminarGrupo() {
    const ok = await confirmar({
      title: `¿Eliminar «${grupo.name}»?`,
      message: 'Se borran el grupo, sus publicaciones y su chat. No se puede deshacer.',
      confirmText: 'Eliminar el grupo',
      danger: true,
    });
    if (!ok) return;
    try {
      await api.del(`/api/groups/${grupo.id}`);
      toast.ok('Grupo eliminado');
      window.location.hash = '#/grupos';
    } catch (e) {
      avisoError(e);
    }
  }

  return (
    <>
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
                className="textarea" value={sobre} onChange={(e) => setSobre(e.target.value)}
                maxLength={400} rows={3} placeholder="Cuéntale a la gente de qué va el grupo"
              />
            </label>
            <label className="campo">
              <span>Tipo de grupo</span>
              <div className="tabs">
                <button type="button" className={privacidad === 'public' ? 'active' : ''} onClick={() => setPrivacidad('public')}>
                  Abierto
                </button>
                <button type="button" className={privacidad === 'private' ? 'active' : ''} onClick={() => setPrivacidad('private')}>
                  Privado
                </button>
              </div>
            </label>
            <div className="row" style={{ gap: 8, flexWrap: 'wrap', padding: '0 12px 12px' }}>
              <button className="btn btn-primary" disabled={guardando}>
                {guardando ? 'Guardando…' : 'Guardar cambios'}
              </button>
              <button type="button" className="btn btn-outline" onClick={cambiarPortada}>
                <IconImage /> Cambiar la portada
              </button>
            </div>
          </form>
        ) : (
          <p className="muted small" style={{ padding: '0 12px 12px', margin: 0 }}>
            Quien creó el grupo es quien puede cambiar su nombre, su descripción y su portada.
          </p>
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
                  type="button" className="icon-btn"
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
            <button type="button" className="btn btn-danger btn-sm" onClick={eliminarGrupo}>
              Eliminar
            </button>
          </div>
        ) : null}
      </section>
    </>
  );
}
