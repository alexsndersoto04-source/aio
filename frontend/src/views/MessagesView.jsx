// Moon — Mensajería (conversaciones + hilo con WebSocket en vivo)

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';
import { realtime } from '../realtime.js';
import { IconSend } from '../components/Icons.jsx';

const REACTIONS = ['👍', '❤️', '😂', '😮', '😢', '🙏'];

export default function MessagesView({ conversationId }) {
  const { user } = useAuth();
  const [convs, setConvs] = useState([]);
  const [convId, setConvId] = useState(conversationId ? Number(conversationId) : null);
  const [thread, setThread] = useState(null); // {partner, messages}
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [search, setSearch] = useState('');
  const endRef = useRef(null);

  const loadConvs = useCallback(() => {
    api.get('/api/messages/conversations')
      .then((rows) => {
        setConvs(rows || []);
        if (!convId && rows && rows.length > 0) setConvId(rows[0].id);
      })
      .catch((e) => alert(e.message));
  }, [convId]);

  useEffect(() => { loadConvs(); }, [loadConvs]);

  const loadThread = useCallback((id) => {
    if (!id) return;
    api.get(`/api/messages/conversations/${id}`)
      .then((data) => {
        setThread(data);
        api.post(`/api/messages/conversations/${id}/read`, {}).catch(() => {});
        setConvId(id);
        // limpiar hash para que el thread sea el estado de la app
        if (window.location.hash !== `#/messages/${id}`) {
          history.replaceState(null, '', `#/messages/${id}`);
        }
      })
      .catch((e) => alert(e.message));
  }, []);

  useEffect(() => { loadThread(convId); }, [convId, loadThread]);

  useEffect(() => {
    if (endRef.current) endRef.current.scrollIntoView({ behavior: 'smooth' });
  }, [thread?.messages?.length]);

  // Tiempo real: mensajes nuevos, lectura, reacciones, borrados
  useEffect(() => {
    const off = realtime.on((ev) => {
      if (ev.type === 'message' && ev.conversation_id === convId && ev.message?.sender_id !== user?.id) {
        setThread((t) => (t ? { ...t, messages: [...t.messages, ev.message] } : t));
        api.post(`/api/messages/conversations/${convId}/read`, {}).catch(() => {});
      } else if (ev.type === 'message_reacted' && ev.conversation_id === convId) {
        setThread((t) => t ? {
          ...t,
          messages: t.messages.map((m) => m.id === ev.message_id ? { ...m, reaction: ev.reaction } : m),
        } : t);
      } else if (ev.type === 'message_deleted' && ev.conversation_id === convId) {
        setThread((t) => t ? {
          ...t,
          messages: t.messages.map((m) => m.id === ev.message_id ? { ...m, content: '', status: 'deleted' } : m),
        } : t);
      } else if (ev.type === 'typing') {
        setThread((t) => t ? { ...t, typing: ev.user_id === t.partner?.id } : t);
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
      alert(err.message);
    } finally {
      setBusy(false);
    }
  }

  async function startChat(uid) {
    try {
      const res = await api.post('/api/messages/conversations', { user_id: uid });
      setConvId(res.conversation_id);
      loadThread(res.conversation_id);
      loadConvs();
    } catch (e) { alert(e.message); }
  }

  function react(msg, reaction) {
    api.post(`/api/messages/${msg.id}/react`, { reaction }).catch((e) => alert(e.message));
  }

  function delMsg(msg) {
    if (msg.sender_id !== user?.id) return;
    if (!window.confirm('¿Eliminar este mensaje?')) return;
    api.del(`/api/messages/${msg.id}`).catch((e) => alert(e.message));
  }

  const visibleConvs = convs.filter((c) =>
    (c.username || '').toLowerCase().includes(search.toLowerCase()) ||
    (c.display_name || '').toLowerCase().includes(search.toLowerCase())
  );

  return (
    <>
      <div className="topbar"><h1>Mensajes</h1></div>
      <div className="card" style={{ overflow: 'hidden' }}>
        <div style={{ display: 'grid', gridTemplateColumns: convId ? '280px 1fr' : '1fr', minHeight: 420 }}>
          {/* Lista de conversaciones */}
          <div style={{ borderRight: convId ? '1px solid var(--line-2)' : 'none', padding: 12, display: 'flex', flexDirection: 'column', gap: 2 }}>
            <input className="input" placeholder="Buscar…" value={search}
              onChange={(e) => setSearch(e.target.value)} style={{ marginBottom: 8 }} />
            {visibleConvs.length === 0 ? (
              <p className="muted" style={{ padding: 16, textAlign: 'center' }}>
                Sin conversaciones. Busca un perfil y toca «Mensaje».
              </p>
            ) : (
              visibleConvs.map((c) => (
                <div key={c.id} className="conv" style={{ background: convId === c.id ? 'var(--surface-2)' : 'transparent' }}
                  onClick={() => loadThread(c.id)}>
                  <Avatar user={c} size="sm" />
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontWeight: 600, fontSize: 14, display: 'flex', alignItems: 'center', gap: 4 }}>
                      {c.display_name || c.username} <VerifiedBadge show={c.is_verified} />
                    </div>
                    <div className="last">@{c.username}{c.last_message ? ` · ${c.last_message}` : ''}</div>
                  </div>
                  {c.unread > 0 ? <span className="unread">{c.unread}</span> : null}
                </div>
              ))
            )}
            <div style={{ marginTop: 'auto', paddingTop: 10 }}>
              <a className="btn btn-outline btn-sm btn-block" href="#/explore">Nueva conversación</a>
            </div>
          </div>

          {/* Hilo */}
          {convId && thread ? (
            <div className="chat" style={{ padding: '0 16px' }}>
              <div className="row" style={{ padding: '12px 0', borderBottom: '1px solid var(--line-2)' }}>
                <a href={`#/user/${thread.partner?.username}`} className="row">
                  <Avatar user={thread.partner} />
                  <div>
                    <div style={{ fontWeight: 700, display: 'flex', alignItems: 'center', gap: 4 }}>
                      {thread.partner?.display_name || thread.partner?.username}
                      <VerifiedBadge show={thread.partner?.is_verified} />
                    </div>
                    <div className="muted">@{thread.partner?.username}</div>
                  </div>
                </a>
              </div>

              <div className="chat-list">
                {thread.messages.map((m) => {
                  const mine = m.sender_id === user?.id;
                  const deleted = m.status === 'deleted';
                  return (
                    <div key={m.id} className={`msg ${mine ? 'mine' : ''}`}
                      style={{ position: 'relative', cursor: mine ? 'pointer' : 'default' }}
                      onClick={(e) => { if (mine && e.shiftKey) delMsg(m); }}
                      title={mine ? 'Click con Shift para eliminar' : undefined}>
                      {deleted ? <em style={{ opacity: 0.6 }}>Mensaje eliminado</em> : m.content}
                      <span className="time">{timeAgo(m.created_at)}</span>
                      {m.reaction ? <span className="react">{m.reaction}</span> : null}
                      {!mine && m.status === 'read' ? <span className="time">✓✓</span> : null}
                      <span className="react" style={{ position: 'absolute', bottom: -10, right: 8 }}>
                        {REACTIONS.map((r) => (
                          <button key={r} onClick={() => react(m, r)} style={{ background: 'none', border: 'none', fontSize: 13, padding: 0, margin: '0 1px' }}>
                            {r}
                          </button>
                        ))}
                      </span>
                    </div>
                  );
                })}
                {thread.typing ? <span className="muted" style={{ alignSelf: 'flex-start', padding: '6px 4px' }}>escribiendo…</span> : null}
                <div ref={endRef} />
              </div>

              <form className="chat-input" onSubmit={send}>
                <input
                  className="input"
                  placeholder="Escribe un mensaje…"
                  value={draft}
                  maxLength={2000}
                  onChange={(e) => {
                    setDraft(e.target.value);
                    realtime.send({ type: 'typing', conversation_id: convId });
                  }}
                />
                <button className="btn" disabled={busy || !draft.trim()} aria-label="Enviar">
                  <IconSend />
                </button>
              </form>
            </div>
          ) : convId && !thread ? (
            <div className="spinner" />
          ) : (
            <div className="empty" style={{ display: 'grid', placeItems: 'center' }}>
              <div>
                <div className="moon-emoji">🌙</div>
                <h3>Selecciona una conversación</h3>
                <p className="muted">O inicia una nueva desde un perfil.</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}
