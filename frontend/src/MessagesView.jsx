import React, { useState, useEffect } from 'react';
import { api } from './api.js';
import { useAuth } from './auth.jsx';

function ConversationsList() {
  const [convs, setConvs] = useState([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api('/api/messages/conversations')
      .then(setConvs)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <p className="muted">Cargando...</p>;

  return (
    <div>
      <h2>Mensajes</h2>
      {convs.length === 0 ? <p className="muted">Sin conversaciones</p> : (
        <div className="conv-list">
          {convs.map((c, i) => (
            <a key={i} href={`#/messages/${c.partner_id}`} className="conv-item">
              <strong>@{c.username}</strong>
              <span className="muted">{c.last_message}</span>
            </a>
          ))}
        </div>
      )}
    </div>
  );
}

function ChatView({ partnerId }) {
  const { user } = useAuth();
  const [partner, setPartner] = useState(null);
  const [messages, setMessages] = useState([]);
  const [text, setText] = useState('');
  const [loading, setLoading] = useState(true);

  async function refresh() {
    try {
      const data = await api(`/api/messages/${partnerId}`);
      setPartner(data.partner);
      setMessages(data.messages);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { setLoading(true); refresh(); }, [partnerId]);

  async function handleSend(e) {
    e.preventDefault();
    if (!text.trim()) return;
    try {
      await api('/api/messages/send', {
        method: 'POST',
        body: JSON.stringify({ receiver_id: Number(partnerId), content: text }),
      });
      setText('');
      refresh();
    } catch (err) {
      alert(err.message);
    }
  }

  if (loading) return <p className="muted">Cargando...</p>;

  return (
    <div>
      <a href="#/messages" className="back-link">&larr; Conversaciones</a>
      <h2>{partner ? `@${partner.username}` : 'Chat'}</h2>
      <div className="chat-messages">
        {messages.map(m => (
          <div key={m.id} className={`chat-msg ${String(m.sender_id) === String(user.id) ? 'chat-msg-mine' : ''}`}>
            <p>{m.content}</p>
            <span className="post-date">{new Date(m.created_at).toLocaleString()}</span>
          </div>
        ))}
        {messages.length === 0 && <p className="muted">Sin mensajes</p>}
      </div>
      <form onSubmit={handleSend} className="chat-form">
        <input
          className="input"
          placeholder="Escribe un mensaje..."
          value={text}
          onChange={e => setText(e.target.value)}
        />
        <button className="btn btn-primary" type="submit">Enviar</button>
      </form>
    </div>
  );
}

export default function MessagesView({ partnerId }) {
  if (partnerId) return <ChatView partnerId={partnerId} />;
  return <ConversationsList />;
}
