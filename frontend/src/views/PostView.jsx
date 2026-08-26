// Moon — Publicación individual + comentarios (tiempo real)

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { timeAgo, linkify } from '../utils.js';
import { realtime } from '../realtime.js';

function CommentRow({ c }) {
  return (
    <div className="row" style={{ alignItems: 'flex-start', gap: 10, padding: '12px 0' }}>
      <Avatar user={{ username: c.username, display_name: c.display_name, avatar_url: c.avatar_url }} size="sm" />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div className="row" style={{ gap: 6 }}>
          <a href={`#/user/${c.username}`} style={{ fontWeight: 700, fontSize: 14 }}>
            {c.display_name || c.username}
          </a>
          <VerifiedBadge show={c.is_verified} />
          <span className="muted">{timeAgo(c.created_at)}</span>
        </div>
        <p style={{ margin: '2px 0 0', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}
          dangerouslySetInnerHTML={{ __html: linkify(c.content) }} />
      </div>
    </div>
  );
}

export default function PostView({ id }) {
  const { user } = useAuth();
  const [post, setPost] = useState(null);
  const [comments, setComments] = useState([]);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [notFound, setNotFound] = useState(false);

  async function load() {
    try {
      const p = await api.get(`/api/posts/${id}`);
      setPost(p);
      const res = await api.get(`/api/posts/${id}/comments`);
      setComments(res || []);
    } catch (e) {
      if (e.status === 404) setNotFound(true);
      else alert(e.message);
    }
  }

  useEffect(() => { load(); /* eslint-disable-next-line */ }, [id]);

  async function sendComment(e) {
    e.preventDefault();
    if (!draft.trim()) return;
    setBusy(true);
    try {
      const created = await api.post(`/api/posts/${id}/comments`, { content: draft.trim() });
      setComments((prev) => [...prev, created]);
      setDraft('');
      setPost((p) => ({ ...p, comments_count: (p.comments_count || 0) + 1 }));
    } catch (err) {
      alert(err.message);
    } finally {
      setBusy(false);
    }
  }

  function report() {
    const reason = window.prompt('Motivo del reporte:', '');
    if (!reason) return;
    api.post('/api/reports', { target_type: 'post', target_id: id, reason, detail: '' })
      .then(() => alert('Reporte enviado.'))
      .catch((e) => alert(e.message));
  }

  if (notFound) return <div className="card empty"><h3>Publicación no encontrada</h3></div>;
  if (!post) return <div className="spinner" />;

  return (
    <>
      <a href="#/feed" className="muted" style={{ display: 'inline-block', marginBottom: 10 }}>← Volver</a>
      <PostCard post={post} onChanged={setPost} />

      <div className="card" style={{ padding: '8px 20px 16px' }}>
        <h3 style={{ margin: '10px 0 4px', fontSize: 16, fontWeight: 800 }}>
          Comentarios <span className="muted" style={{ fontWeight: 500 }}>({comments.length})</span>
        </h3>

        <form className="row" style={{ padding: '10px 0', borderBottom: '1px solid var(--line-2)', marginBottom: 6 }} onSubmit={sendComment}>
          <Avatar user={user} size="sm" />
          <input
            className="input"
            style={{ border: 'none', boxShadow: 'none', background: 'var(--surface-2)', borderRadius: 999, padding: '9px 16px' }}
            placeholder="Escribe un comentario…"
            value={draft}
            maxLength={600}
            onChange={(e) => setDraft(e.target.value)}
          />
          <button className="btn btn-sm" disabled={busy || !draft.trim()}>Enviar</button>
        </form>

        {comments.length === 0 ? (
          <p className="muted" style={{ padding: '14px 0' }}>Sé el primero en comentar.</p>
        ) : (
          comments.map((c) => <CommentRow key={c.id} c={c} />)
        )}
      </div>
    </>
  );
}
