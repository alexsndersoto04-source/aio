// Moon — Mensajería (conversaciones + hilo con WebSocket en vivo)
// ============================================================
// Dos paneles: a la izquierda las conversaciones, a la derecha el hilo.
// En móvil se muestra uno cada vez (la flecha vuelve a la lista).
// Todo en vivo: mensajes nuevos, reacciones, borrados, «escribiendo…» y
// confirmación de lectura.

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Grabador, AudioMensaje, puedeGrabar } from '../components/NotaVoz.jsx';
import { api, uploadMedia, imgUrl } from '../api.js';
import { toast, confirmar, avisoError } from '../ui.js';
import { useAuth } from '../auth.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { timeAgo, horaMensaje } from '../utils.js';
import { realtime } from '../realtime.js';
import { ListSkeleton } from '../components/Skeleton.jsx';
import {
  IconSend, IconSearch, IconChevronLeft, IconTrash, IconMail, IconCheck, IconAt,
  IconMore, IconBell, IconLayers, IconEye, IconComment, IconExplore, IconUsers,
} from '../components/Icons.jsx';

const REACCIONES = ['👍', '❤️', '😂', '😮', '😢', '🙏'];

export default function MessagesView({ conversationId }) {
  const { user } = useAuth();
  const [convs, setConvs] = useState(null);
  const [convId, setConvId] = useState(conversationId ? Number(conversationId) : null);
  // Quién está conectado ahora mismo (presencia real del servidor).
  const [enLinea, setEnLinea] = useState(() => new Set());
  const [thread, setThread] = useState(null);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [search, setSearch] = useState('');
  // Filtro de la lista: todas · sin leer · archivadas
  const [filtro, setFiltro] = useState('todas');
  // Qué conversación tiene el menú de opciones abierto
  const [menuDe, setMenuDe] = useState(null);
  // Preferencias por conversación (silenciada / archivada) traídas del servidor
  const [prefs, setPrefs] = useState({});
  const [reaccionando, setReaccionando] = useState(null);
  const endRef = useRef(null);
  // En el teléfono se abre SIEMPRE la lista: así se ven el buscador, los
  // filtros (sin leer / archivadas) y las opciones de cada conversación.
  // En pantalla ancha, donde la lista y el hilo se ven a la vez, se entra
  // solo en la primera conversación para no dejar el lado derecho vacío.
  const entroSolo = useRef(!!conversationId);

  const loadConvs = useCallback(() => {
    api.get('/api/messages/conversations')
      .then((rows) => {
        const lista = rows || [];
        setConvs(lista);
        const anchoDeSobra = typeof window !== 'undefined' && window.innerWidth >= 900;
        if (anchoDeSobra && !entroSolo.current && lista.length > 0) {
          entroSolo.current = true;
          setConvId(lista[0].id);
        }
      })
      .catch((e) => { setConvs([]); avisoError(e); });
  }, []);

  useEffect(() => { loadConvs(); }, [loadConvs]);

  const loadThread = useCallback((id) => {
    if (!id) return;
    api.get(`/api/messages/conversations/${id}`)
      .then((data) => {
        setThread(data);
        api.post(`/api/messages/conversations/${id}/read`, {}).catch(() => {});
        setConvId(id);
        if (window.location.hash !== `#/messages/${id}`) {
          history.replaceState(null, '', `#/messages/${id}`);
        }
      })
      .catch(avisoError);
  }, []);

  useEffect(() => { loadThread(convId); }, [convId, loadThread]);

  // Presencia: quién está conectado, para el punto verde de la lista.
  useEffect(() => {
    let vivo = true;
    const cargarPresencia = () => api.get('/api/users/presence')
      .then((res) => {
        if (!vivo) return;
        setEnLinea(new Set((res.en_linea || []).map((u) => Number(u.id))));
      })
      .catch(() => {});
    cargarPresencia();
    const off = realtime.on((ev) => { if (ev.type === 'presence') cargarPresencia(); });
    return () => { vivo = false; off(); };
  }, []);

  useEffect(() => {
    if (endRef.current) endRef.current.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [thread?.messages?.length, thread?.typing]);

  // Tiempo real: mensajes nuevos, lectura, reacciones, borrados
  useEffect(() => {
    const off = realtime.on((ev) => {
      if (ev.type === 'message' && ev.conversation_id === convId && ev.message?.sender_id !== user?.id) {
        setThread((t) => (t ? { ...t, messages: [...t.messages, ev.message] } : t));
        api.post(`/api/messages/conversations/${convId}/read`, {}).catch(() => {});
      } else if (ev.type === 'message_reacted' && ev.conversation_id === convId) {
        setThread((t) => (t ? {
          ...t,
          messages: t.messages.map((m) => (m.id === ev.message_id ? { ...m, reaction: ev.reaction } : m)),
        } : t));
      } else if (ev.type === 'message_deleted' && ev.conversation_id === convId) {
        setThread((t) => (t ? {
          ...t,
          messages: t.messages.map((m) => (m.id === ev.message_id ? { ...m, content: '', status: 'deleted' } : m)),
        } : t));
      } else if (ev.type === 'typing') {
        setThread((t) => (t ? { ...t, typing: ev.user_id === t.partner?.id } : t));
      }
      loadConvs();
    });
    return off;
  }, [convId, user?.id, loadConvs]);

  async function send(e) {
    e.preventDefault();
    if (!draft.trim() || !convId) return;
    setBusy(true);
    try {
      const created = await api.post(`/api/messages/conversations/${convId}/messages`, { content: draft.trim() });
      setThread((t) => (t ? { ...t, messages: [...t.messages, created] } : t));
      setDraft('');
      loadConvs();
    } catch (err) {
      avisoError(err);
    } finally {
      setBusy(false);
    }
  }

  async function enviarNota(blob, duracionMs) {
    if (!convId) return;
    setBusy(true);
    try {
      const subida = await uploadMedia('audio', blob);
      const creado = await api.post(`/api/messages/conversations/${convId}/messages`, {
        audio_url: subida.url,
        duracion_ms: duracionMs,
      });
      setThread((t) => (t ? { ...t, messages: [...t.messages, creado] } : t));
      loadConvs();
    } catch (err) {
      avisoError(err);
    } finally {
      setBusy(false);
    }
  }

  function react(msg, reaction) {
    setReaccionando(null);
    api.post(`/api/messages/${msg.id}/react`, { reaction }).catch(avisoError);
  }

  async function delMsg(msg) {
    if (msg.sender_id !== user?.id) return;
    const ok = await confirmar({
      title: '¿Eliminar este mensaje?',
      message: 'Desaparecerá de la conversación para los dos.',
      confirmText: 'Eliminar',
      danger: true,
    });
    if (!ok) return;
    api.del(`/api/messages/${msg.id}`)
      .then(() => toast.ok('Mensaje eliminado'))
      .catch(avisoError);
  }

  /** La hora de la última vez: hoy → «9:55»; ayer → «ayer»; antes → fecha corta. */
  function cuandoConv(iso) {
    if (!iso) return '';
    const d = new Date(String(iso).replace(' ', 'T'));
    if (Number.isNaN(d.getTime())) return '';
    const hoy = new Date();
    const mismoDia = d.toDateString() === hoy.toDateString();
    if (mismoDia) return d.toLocaleTimeString('es-VE', { hour: '2-digit', minute: '2-digit', hour12: false });
    const ayer = new Date(hoy); ayer.setDate(hoy.getDate() - 1);
    if (d.toDateString() === ayer.toDateString()) return 'ayer';
    const mismoAnio = d.getFullYear() === hoy.getFullYear();
    return d.toLocaleDateString('es-VE', mismoAnio ? { day: 'numeric', month: 'short' } : { day: '2-digit', month: '2-digit', year: '2-digit' });
  }

  /** Cuántas conversaciones del filtro activo hay sin abrir. */
  const sinLeer = (convs || []).filter((c) => c.unread > 0).length;
  const archivadas = (convs || []).filter((c) => c.archivada).length;

  const visibles = (convs || []).filter((c) => {
    const t = search.trim().toLowerCase();
    if (t) {
      const coincide = (c.username || '').toLowerCase().includes(t) || (c.display_name || '').toLowerCase().includes(t);
      if (!coincide) return false;
    }
    if (filtro === 'sin_leer') return c.unread > 0;
    if (filtro === 'archivadas') return !!c.archivada;
    return !c.archivada;
  });

  /** Silenciar, archivar u ocultar: se guarda en el servidor y se ve al instante. */
  async function cambiarPref(conv, cambios) {
    const antes = { silenciada: !!conv.silenciada, archivada: !!conv.archivada };
    setConvs((lista) => (lista || []).map((c) => (c.id === conv.id ? { ...c, ...cambios } : c)));
    setMenuDe(null);
    try {
      const r = await api.post(`/api/messages/conversations/${conv.id}/prefs`, cambios);
      setPrefs((p) => ({ ...p, [conv.id]: r }));
      if (cambios.archivada === true) toast.ok('Conversación archivada');
      else if (cambios.archivada === false && antes.archivada) toast.ok('Conversación de vuelta en la lista');
      else if (cambios.silenciada === true) toast.ok('Avisos silenciados');
      else if (cambios.silenciada === false) toast.ok('Avisos activados');
      else if (cambios.oculta === true) toast.ok('Conversación oculta');
    } catch (e) {
      setConvs((lista) => (lista || []).map((c) => (c.id === conv.id ? { ...c, ...antes } : c)));
      avisoError(e);
    }
  }

  return (
    <>
      <div className="page-head">
        <h1>Mensajes</h1>
        <span className="spacer" />
        <a className="btn btn-outline btn-sm" href="#/explore">Nueva conversación</a>
      </div>

      <div className={`chat ${convId ? 'hide-list' : 'hide-thread'}`} data-con-hilo={convId ? 'si' : 'no'}>
        {/* ---- Conversaciones ---- */}
        <aside className="chat-list" aria-label="Conversaciones">
          <div className="head">
            <div className="rail-search">
              <IconSearch />
              <input
                className="input"
                type="search"
                placeholder="Buscar conversación"
                aria-label="Buscar conversación"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
          </div>

          <div className="chips-filtro chips-chat" role="group" aria-label="Filtrar conversaciones">
            <button type="button" className={filtro === 'todas' ? 'activo' : ''} onClick={() => setFiltro('todas')}>
              Todas
            </button>
            <button type="button" className={filtro === 'sin_leer' ? 'activo' : ''} onClick={() => setFiltro('sin_leer')}>
              Sin leer{sinLeer > 0 ? ` (${sinLeer})` : ''}
            </button>
            <button type="button" className={filtro === 'archivadas' ? 'activo' : ''} onClick={() => setFiltro('archivadas')}>
              Archivadas{archivadas > 0 ? ` (${archivadas})` : ''}
            </button>
          </div>

          {convs === null ? <ListSkeleton rows={5} /> : null}

          {convs && visibles.length === 0 ? (
            <div className="empty">
              <IconMail />
              <h3>{filtro === 'sin_leer' ? 'Nada sin leer' : filtro === 'archivadas' ? 'Sin conversaciones archivadas' : 'Sin conversaciones'}</h3>
              <p>
                {filtro === 'sin_leer'
                  ? 'Estás al día con todos tus mensajes.'
                  : filtro === 'archivadas'
                    ? 'Archiva una conversación desde su menú (los tres puntos) para quitarla de en medio sin perderla.'
                    : 'Busca a alguien en Explorar y toca «Mensaje» para empezar.'}
              </p>
              {filtro === 'todas' ? (
                <a className="btn btn-outline btn-sm" href="#/explore">
                  <IconExplore /> Buscar personas
                </a>
              ) : null}
            </div>
          ) : null}

          {visibles.map((c) => (
            <div className={`fila-conv ${convId === c.id ? 'active' : ''}`} key={c.id}>
              <button type="button" className="conv" onClick={() => loadThread(c.id)}>
                <span className="avatar-con-estado">
                  <Avatar user={c} size="md" />
                  {enLinea.has(Number(c.id)) ? <span className="punto-online" /> : null}
                </span>
                <span className="body">
                  <span className="linea-superior">
                    <b className="ellipsis">
                      {c.display_name || c.username}
                      <VerifiedBadge show={c.is_verified} />
                      {c.silenciada ? <span className="icono-silencio" title="Avisos silenciados"><IconBell /></span> : null}
                    </b>
                    <span className="hora-conv">{cuandoConv(c.updated_at)}</span>
                  </span>
                  <span className="linea-inferior">
                    <span className="last ellipsis">
                      {c.last_message ? c.last_message : `@${c.username}`}
                    </span>
                    {c.unread > 0 ? <span className="sin-leer">{c.unread > 99 ? '99+' : c.unread}</span> : null}
                  </span>
                </span>
              </button>

              <button
                type="button"
                className="mas-conv"
                onClick={() => setMenuDe(menuDe === c.id ? null : c.id)}
                aria-expanded={menuDe === c.id}
                aria-label={`Opciones de la conversación con ${c.display_name || c.username}`}
              >
                <IconMore />
              </button>

              {menuDe === c.id ? (
                <>
                  <span className="hoja-fondo" role="presentation" onClick={() => setMenuDe(null)} />
                  <div className="menu-conv" role="menu">
                  <button type="button" onClick={() => cambiarPref(c, { silenciada: !c.silenciada })}>
                    <IconBell />
                    {c.silenciada ? 'Activar los avisos' : 'Silenciar los avisos'}
                  </button>
                  <button type="button" onClick={() => cambiarPref(c, { archivada: !c.archivada })}>
                    <IconLayers />
                    {c.archivada ? 'Sacar de archivadas' : 'Archivar conversación'}
                  </button>
                  <a href={`#/user/${c.username}`} onClick={() => setMenuDe(null)}>
                    <IconUsers /> Ver el perfil
                  </a>
                  <button type="button" onClick={() => cambiarPref(c, { oculta: true })}>
                    <IconEye /> Ocultar de la lista
                  </button>
                  </div>
                </>
              ) : null}
            </div>
          ))}
        </aside>

        {/* ---- Hilo ---- */}
        {convId && thread ? (
          <section className="chat-thread" aria-label={`Conversación con ${thread.partner?.username || ''}`}>
            <header className="head">
              <button
                className="icon-btn"
                onClick={() => {
                  setConvId(null);
                  setThread(null);
                  if (window.location.hash !== '#/messages') {
                    history.replaceState(null, '', '#/messages');
                  }
                }}
                aria-label="Volver a las conversaciones"
                style={{ display: 'none' }}
                data-volver
              >
                <IconChevronLeft />
              </button>
              <a href={`#/user/${thread.partner?.username}`} className="row" style={{ gap: 11, minWidth: 0 }}>
                <Avatar user={thread.partner} />
                <span style={{ minWidth: 0 }}>
                  <b className="ellipsis" style={{ display: 'block' }}>
                    {thread.partner?.display_name || thread.partner?.username}
                    <VerifiedBadge show={thread.partner?.is_verified} />
                  </b>
                  <span className="muted" style={{ fontSize: 12.5 }}>
                    <IconAt style={{ width: 11, height: 11, verticalAlign: -1 }} />
                    {thread.partner?.username}
                    {enLinea.has(Number(thread.partner?.id)) ? (
                      <span className="estado-linea"><span className="punto-online" /> en línea</span>
                    ) : null}
                  </span>
                </span>
              </a>
              <span className="spacer" />
              <button
                type="button"
                className="icon-btn"
                onClick={() => setMenuDe(menuDe === `hilo-${convId}` ? null : `hilo-${convId}`)}
                aria-expanded={menuDe === `hilo-${convId}`}
                aria-label="Opciones de la conversación"
              >
                <IconMore />
              </button>
              {menuDe === `hilo-${convId}` ? (
                <>
                  <span className="hoja-fondo" role="presentation" onClick={() => setMenuDe(null)} />
                  <div className="menu-conv menu-hilo" role="menu">
                  <button type="button" onClick={() => cambiarPref(thread.partner, { silenciada: !(prefs[convId]?.silenciada ?? false) })}>
                    <IconBell />
                    {(prefs[convId]?.silenciada ?? false) ? 'Activar los avisos' : 'Silenciar los avisos'}
                  </button>
                  <button type="button" onClick={() => cambiarPref(thread.partner, { archivada: !(prefs[convId]?.archivada ?? false) })}>
                    <IconLayers /> Archivar conversación
                  </button>
                  <a href={`#/user/${thread.partner?.username}`} onClick={() => setMenuDe(null)}>
                    <IconUsers /> Ver el perfil
                  </a>
                  </div>
                </>
              ) : null}
            </header>

            <div className="chat-messages">
              {thread.messages.length === 0 ? (
                <div className="empty">
                  <IconMail />
                  <h3>Empieza la conversación</h3>
                  <p>Escribe el primer mensaje a {thread.partner?.display_name || thread.partner?.username}.</p>
                </div>
              ) : null}

              {thread.messages.map((m) => {
                const mio = m.sender_id === user?.id;
                const borrado = m.status === 'deleted';
                return (
                  <div className={`msg ${mio ? 'mine' : ''}`} key={m.id}>
                    {borrado ? (
                      <em style={{ opacity: 0.65 }}>Mensaje eliminado</em>
                    ) : (
                      <>
                        {m.audio_url ? <AudioMensaje url={m.audio_url} duracionMs={m.duracion_ms} mio={mio} /> : null}
                        {m.image_url ? <img className="img-msg" src={imgUrl(m.image_url)} alt="" /> : null}
                        {m.content ? <span className="texto-msg">{m.content}</span> : null}
                      </>
                    )}
                    {m.reaction ? <span className="react">{m.reaction}</span> : null}
                    <span className="time">
                      {horaMensaje(m.created_at)}
                      {mio && m.status === 'read' ? ' · leído' : null}
                    </span>

                    {borrado ? null : (
                      <span className="msg-actions" style={{ position: 'absolute', top: -12, [mio ? 'right' : 'left']: 6 }}>
                        <button
                          onClick={() => setReaccionando(reaccionando === m.id ? null : m.id)}
                          aria-label="Reaccionar al mensaje"
                          title="Reaccionar"
                        >
                          <span className="moon-emoji">🙂</span>
                        </button>
                        {mio ? (
                          <button onClick={() => delMsg(m)} aria-label="Eliminar mensaje" title="Eliminar">
                            <IconTrash />
                          </button>
                        ) : null}
                      </span>
                    )}

                    {reaccionando === m.id ? (
                      <span className="menu" style={{ top: -46, bottom: 'auto', [mio ? 'right' : 'left']: 0 }}>
                        <span className="row" style={{ gap: 2, padding: 4 }}>
                          {REACCIONES.map((r) => (
                            <button key={r} onClick={() => react(m, r)} style={{ width: 32, height: 32, justifyContent: 'center', padding: 0 }}>
                              <span className="moon-emoji" style={{ fontSize: 18 }}>{r}</span>
                            </button>
                          ))}
                        </span>
                      </span>
                    ) : null}
                  </div>
                );
              })}

              {thread.typing ? (
                <span className="typing" aria-live="polite">
                  escribiendo<i /><i /><i />
                </span>
              ) : null}

              <div ref={endRef} />
            </div>

            <form className="chat-input" onSubmit={send}>
              <textarea
                className="textarea"
                rows={1}
                placeholder="Escribe un mensaje…"
                aria-label="Escribe un mensaje"
                value={draft}
                maxLength={2000}
                onChange={(e) => {
                  setDraft(e.target.value);
                  realtime.send({ type: 'typing', conversation_id: convId });
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(e); }
                }}
              />
              {puedeGrabar() ? <Grabador onListo={enviarNota} disabled={busy} /> : null}
              <button className="btn btn-primary" disabled={busy || !draft.trim()} aria-label="Enviar mensaje">
                {busy ? <IconCheck /> : <IconSend />}
              </button>
            </form>
          </section>
        ) : convId && !thread ? (
          <section className="chat-thread"><div className="spinner" /></section>
        ) : (
          <section className="chat-thread">
            <div className="empty empty-state">
              <span className="moon-emoji" style={{ fontSize: 30 }}>🌙</span>
              <h3>Elige una conversación</h3>
              <p>O empieza una nueva desde el perfil de alguien.</p>
              <a className="btn btn-outline btn-sm" href="#/explore">Explorar personas</a>
            </div>
          </section>
        )}
      </div>
    </>
  );
}
