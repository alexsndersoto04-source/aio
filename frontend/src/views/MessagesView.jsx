import React, { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import Avatar from '../components/Avatar.jsx';
import { ChatIcon, SendIcon, BackIcon } from '../components/Icons.jsx';
import { timeAgo } from '../utils.js';

function Conversations({ activeId }) {
  const [convs, setConvs] = useState(null);
  const [hash, setHash] = useState(window.location.hash);

  useEffect(() => {
    const f = () => setHash(window.location.hash);
    window.addEventListener('hashchange', f);
    return () => window.removeEventListener('hashchange', f);
  }, []);

  useEffect(() => {
    api('/api/messages/conversations')
      .then(setConvs)
      .catch(() => setConvs([]));
  }, [hash]);

  return (
    <aside className="card conv-list">
      <h3 className="panel-title">Mensajes</h3>
      {convs === null ? (
        <p className="muted sm conv-empty">Cargando…</p>
      ) : convs.length === 0 ? (
        <p className="muted sm conv-empty">
          Aún no tienes conversaciones. Visita un perfil y presiona “Mensaje”.
        </p>
      ) : (
        convs.map(c => (
          <a
            key={c.partner_id}
            href={`#/messages/${c.partner_id}`}
            className={`conv ${String(activeId) === String(c.partner_id) ? 'conv-active' : ''}`}
          >
            <Avatar name={c.username} size="sm" />
            <div className="conv-text">
              <strong>@{c.username}</strong>
              <span>{c.last_message}</span>
            </div>
            <span className="muted xs">{timeAgo(c.last_created_at)}</span>
          </a>
        ))
      )}
    </aside>
  );
}

function Chat({ partnerId }) {
  const { user } = useAuth();
  const [data, setData] = useState(null);
  const [text, setText] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const boxRef = useRef(null);

  const load = useCallback(async () => {
    try {
      setData(await api(`/api/messages/${partnerId}`));
    } catch (err) {
      setError(err.message);
    }
  }, [partnerId]);

  useEffect(() => {
    setData(null);
    load();
    const t = setInterval(load, 8000);
    return () => clearInterval(t);
  }, [load]);

  useEffect(() => {
    if (boxRef.current) boxRef.current.scrollTop = boxRef.current.scrollHeight;
  }, [data]);

  async function send(e) {
    e.preventDefault();
    if (!text.trim() || sending) return;
    setSending(true);
    setError('');
    try {
      await api('/api/messages/send', {
        method: 'POST',
        body: JSON.stringify({ receiver_id: Number(partnerId), content: text.trim() }),
      });
      setText('');
      await load();
    } catch (err) {
      setError(err.message);
    } finally {
      setSending(false);
    }
  }

  const msgs = (data && data.messages) || [];

  return (
    <section className="card chat">
      <header className="chat-head">
        <a className="icon-btn back-only" href="#/messages" title="Volver">
          <BackIcon size={18} />
        </a>
        {data ? (
          <>
            <Avatar
              src={data.partner.avatar_url}
              name={data.partner.display_name || data.partner.username}
              size="sm"
            />
            <div className="chat-who">
              <strong>{data.partner.display_name || data.partner.username}</strong>
              <span className="muted sm">@{data.partner.username}</span>
            </div>
          </>
        ) : (
          <div className="chat-who">
            <strong>Cargando…</strong>
          </div>
        )}
      </header>
      <div className="chat-body" ref={boxRef}>
        {msgs.length === 0 ? (
          <p className="muted chat-empty-msg">No hay mensajes todavía. Escribe el primero.</p>
        ) : (
          msgs.map(m => {
            const mine = String(m.sender_id) === String(user.id);
            return (
              <div key={m.id} className={`bubble-row ${mine ? 'mine' : ''}`}>
                <div className="bubble">
                  {m.content}
                  <span className="bubble-time">{timeAgo(m.created_at)}</span>
                </div>
              </div>
            );
          })
        )}
      </div>
      {error && <p className="form-error chat-error">{error}</p>}
      <form className="chat-form" onSubmit={send}>
        <input
          className="input"
          placeholder="Escribe un mensaje…"
          value={text}
          onChange={e => setText(e.target.value)}
        />
        <button className="btn btn-primary" type="submit" disabled={sending || !text.trim()}>
          <SendIcon size={15} /> Enviar
        </button>
      </form>
    </section>
  );
}

export default function MessagesView({ partnerId }) {
  return (
    <div className={`messages ${partnerId ? 'messages-open' : ''}`}>
      <Conversations activeId={partnerId} />
      {partnerId ? (
        <Chat partnerId={partnerId} />
      ) : (
        <div className="card chat-placeholder">
          <ChatIcon size={38} />
          <p>Elige una conversación para empezar.</p>
        </div>
      )}
    </div>
  );
}
