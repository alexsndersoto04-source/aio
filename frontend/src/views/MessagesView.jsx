// Moon — Mensajería (conversaciones + hilo con WebSocket en vivo)
// ============================================================
// Dos paneles: a la izquierda las conversaciones, a la derecha el hilo.
// En móvil se muestra uno cada vez (la flecha vuelve a la lista).
// Todo en vivo: mensajes nuevos, reacciones, borrados, «escribiendo…» y
// confirmación de lectura.

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api.js';
import { toast, confirmar, avisoError } from '../ui.js';
import { useAuth } from '../auth.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { timeAgo, horaMensaje } from '../utils.js';
import { realtime } from '../realtime.js';
import { ListSkeleton } from '../components/Skeleton.jsx';
import {
  IconSend, IconSearch, IconChevronLeft, IconTrash, IconMail, IconCheck, IconAt,
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
  const [reaccionando, setReaccionando] = useState(null);
  const endRef = useRef(null);
  // Solo se entra solo en la primera conversación la primera vez que se abre
  // la pantalla: si la persona pulsa «volver», se queda en la lista (antes
  // volvía a entrar y en el teléfono no había manera de salir del hilo).
  const entroSolo = useRef(!!conversationId);

  const loadConvs = useCallback(() => {
    api.get('/api/messages/conversations')
      .then((rows) => {
        const lista = rows || [];
        setConvs(lista);
        if (!entroSolo.current && lista.length > 0) {
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

  const visibles = (convs || []).filter((c) => {
    const t = search.trim().toLowerCase();
    if (!t) return true;
    return (c.username || '').toLowerCase().includes(t) || (c.display_name || '').toLowerCase().includes(t);
  });

  return (
    <>
      <div className="page-head">
        <h1>Mensajes</h1>
        <span className="spacer" />
        <a className="btn btn-outline btn-sm" href="#/explore">Nueva conversación</a>
      </div>

      <div className={`chat ${convId ? 'hide-list' : 'hide-thread'}`}>
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

          {convs === null ? <ListSkeleton rows={5} /> : null}

          {convs && visibles.length === 0 ? (
            <div className="empty">
              <IconMail />
              <h3>Sin conversaciones</h3>
              <p>Busca a alguien en Explorar y toca «Mensaje» para empezar.</p>
            </div>
          ) : null}

          {visibles.map((c) => (
            <button
              key={c.id}
              type="button"
              className={`conv ${convId === c.id ? 'active' : ''}`}
              onClick={() => loadThread(c.id)}
            >
              <span className="avatar-con-estado">
                <Avatar user={c} size="sm" />
                {enLinea.has(Number(c.id)) ? <span className="punto-online" /> : null}
              </span>
              <span className="body">
                <b className="ellipsis">
                  {c.display_name || c.username}
                  <VerifiedBadge show={c.is_verified} />
                </b>
                <span className="last ellipsis">
                  {c.last_message ? c.last_message : `@${c.username}`}
                </span>
              </span>
              {c.unread > 0 ? <span className="badge">{c.unread > 99 ? '99+' : c.unread}</span> : null}
            </button>
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
                    {borrado ? <em style={{ opacity: 0.65 }}>Mensaje eliminado</em> : m.content}
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
