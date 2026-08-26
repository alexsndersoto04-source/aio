// Moon — Notificaciones (tiempo real)

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import Avatar from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';
import { realtime } from '../realtime.js';

const KIND_LABEL = {
  follow: 'te siguió',
  like: 'le gusta tu publicación',
  comment: 'comentó tu publicación',
  reply: 'respondió tu comentario',
  mention: 'te mencionó',
  message: 'te envió un mensaje',
  system: '',
};

export default function NotificationsView() {
  const [items, setItems] = useState([]);
  const [loading, setLoading] = useState(true);

  async function load() {
    try {
      const res = await api.get('/api/notifications?page=1&limit=30');
      setItems(res.items || []);
    } catch (e) {
      alert(e.message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, []);

  useEffect(() => {
    const off = realtime.on((ev) => {
      if (ev.type === 'notification') {
        setItems((prev) => [normalize(ev), ...prev]);
      }
    });
    return off;
  }, []);

  function normalize(ev) {
    return {
      id: Date.now(), // temporal hasta el próximo sync
      type: ev.kind,
      content: ev.content,
      is_read: false,
      created_at: new Date().toISOString(),
      from_username: '',
      from_display_name: '',
      from_avatar_url: '',
      post_id: ev.post_id,
    };
  }

  async function markAll() {
    try {
      await api.post('/api/notifications/read-all', {});
      setItems((prev) => prev.map((n) => ({ ...n, is_read: true })));
      realtime.send({ type: 'sync' });
    } catch (e) { /* noop */ }
  }

  async function markOne(n) {
    if (n.is_read) return;
    try {
      await api.post(`/api/notifications/${n.id}/read`, {});
      setItems((prev) => prev.map((x) => (x.id === n.id ? { ...x, is_read: true } : x)));
    } catch (e) { /* noop */ }
  }

  function href(n) {
    if (n.post_id) return `#/post/${n.post_id}`;
    if (n.from_username) return `#/user/${n.from_username}`;
    if (n.type === 'message') return '#/messages';
    return '#/notifications';
  }

  if (loading) return <div className="spinner" />;

  return (
    <>
      <div className="topbar">
        <h1>Notificaciones</h1>
        <button className="btn-ghost btn-sm" onClick={markAll}>Marcar todas como leídas</button>
      </div>

      <div className="card" style={{ padding: '4px 0' }}>
        {items.length === 0 ? (
          <div className="empty">
            <div className="moon-emoji">🔔</div>
            <h3>Sin notificaciones</h3>
            <p>Cuando alguien interactúe contigo, aparecerá aquí.</p>
          </div>
        ) : (
          items.map((n) => (
            <a key={n.id} href={href(n)} className={`notif ${n.is_read ? '' : 'unread'}`} onClick={() => markOne(n)}>
              <Avatar user={{ username: n.from_username, display_name: n.from_display_name, avatar_url: n.from_avatar_url }} size="sm" />
              <div className="text">
                <span>
                  <b>{n.from_display_name || n.from_username || 'Moon'}</b>{' '}
                  {KIND_LABEL[n.type] || n.content || ''}
                </span>
                {n.type === 'system' && n.content ? <div>{n.content}</div> : null}
                <div className="time">{timeAgo(n.created_at)}</div>
              </div>
              {!n.is_read ? <span className="dot" /> : null}
            </a>
          ))
        )}
      </div>
    </>
  );
}
