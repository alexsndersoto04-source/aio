// Moon — Tarjeta de publicación (feed, perfil, búsqueda)
// ============================================================
// Like / save / comentar / menú (editar, eliminar, reportar) — todo
// contra el backend real, con optimismo controlado.

import React, { useState } from 'react';
import { api, ApiError } from '../api.js';
import { useAuth } from '../auth.jsx';
import { timeAgo, linkify } from '../utils.js';
import Avatar, { VerifiedBadge } from './Avatar.jsx';
import {
  IconHeart, IconBookmark, IconComment, IconMore, IconTrash, IconEdit, IconReport,
} from './Icons.jsx';

function PostMenu({ post, onDelete, onEdit, onReport }) {
  const { user } = useAuth();
  const [open, setOpen] = useState(false);
  const isMine = user && post.is_mine;
  return (
    <div style={{ position: 'relative' }}>
      <button className="btn-ghost btn-sm" onClick={() => setOpen(!open)} aria-label="Más opciones">
        <IconMore />
      </button>
      {open ? (
        <div className="card" style={{ position: 'absolute', right: 0, top: 34, zIndex: 20, minWidth: 170, padding: 6 }}>
          {isMine ? (
            <>
              <button className="btn-ghost btn-sm" style={{ width: '100%', justifyContent: 'flex-start' }}
                onClick={() => { setOpen(false); onEdit(); }}>
                <IconEdit /> Editar
              </button>
              <button className="btn-ghost btn-sm" style={{ width: '100%', justifyContent: 'flex-start', color: 'var(--danger)' }}
                onClick={() => { setOpen(false); onDelete(); }}>
                <IconTrash /> Eliminar
              </button>
            </>
          ) : (
            <button className="btn-ghost btn-sm" style={{ width: '100%', justifyContent: 'flex-start' }}
              onClick={() => { setOpen(false); onReport(); }}>
              <IconReport /> Reportar
            </button>
          )}
        </div>
      ) : null}
    </div>
  );
}

export default function PostCard({ post, onChanged, compact = false }) {
  const { user, refreshMe } = useAuth();
  const [p, setP] = useState(post);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(p.content || '');

  const apply = (next) => { setP(next); if (onChanged) onChanged(next); };

  async function toggle(kind) {
    if (busy) return;
    setBusy(true);
    try {
      if (kind === 'like') {
        const res = await api.post(`/api/posts/${p.id}/like`, {});
        apply({ ...p, is_liked: true, likes_count: p.likes_count + 1 });
      } else if (kind === 'unlike') {
        await api.del(`/api/posts/${p.id}/like`);
        apply({ ...p, is_liked: false, likes_count: Math.max(0, p.likes_count - 1) });
      } else if (kind === 'save') {
        const res = await api.post(`/api/posts/${p.id}/save`, {});
        apply({ ...p, is_saved: res.saved, saves_count: res.saved ? p.saves_count + 1 : Math.max(0, p.saves_count - 1) });
      }
    } catch (e) {
      alert(e.message);
    } finally {
      setBusy(false);
    }
  }

  async function del() {
    if (!window.confirm('¿Eliminar esta publicación?')) return;
    try {
      await api.del(`/api/posts/${p.id}`);
      apply({ ...p, deleted: true });
      refreshMe();
    } catch (e) {
      alert(e.message);
    }
  }

  async function saveEdit() {
    if (!draft.trim()) return;
    try {
      await api.patch(`/api/posts/${p.id}`, { content: draft.trim() });
      apply({ ...p, content: draft.trim() });
      setEditing(false);
    } catch (e) {
      alert(e.message);
    }
  }

  function report() {
    const reason = window.prompt('Motivo del reporte (ej. spam, acoso, contenido inapropiado):', '');
    if (!reason) return;
    api.post('/api/reports', { target_type: 'post', target_id: p.id, reason, detail: '' })
      .then(() => alert('Reporte enviado. Gracias por ayudar a mantener Moon seguro.'))
      .catch((e) => alert(e.message));
  }

  if (p.deleted) return null;

  const likeClass = p.is_liked ? 'liked' : '';
  const saveClass = p.is_saved ? 'saved' : '';

  return (
    <article className="post">
      <div className="post-head">
        <a href={`#/user/${p.author_username}`}><Avatar user={{ username: p.author_username, display_name: p.author_display_name, avatar_url: p.author_avatar_url }} /></a>
        <div className="who">
          <div className="name">
            <a href={`#/user/${p.author_username}`}>{p.author_display_name || p.author_username}</a>
            <VerifiedBadge show={p.author_is_verified} />
            <span className="at">@{p.author_username} · {timeAgo(p.created_at)}</span>
          </div>
        </div>
        <PostMenu post={p} onDelete={del} onEdit={() => setEditing(true)} onReport={report} />
      </div>

      {editing ? (
        <div className="mb">
          <textarea className="textarea" value={draft} maxLength={2000}
            onChange={(e) => setDraft(e.target.value)} rows={3} />
          <div className="row mt" style={{ justifyContent: 'flex-end' }}>
            <button className="btn-ghost btn-sm" onClick={() => { setEditing(false); setDraft(p.content); }}>Cancelar</button>
            <button className="btn btn-sm" onClick={saveEdit}>Guardar</button>
          </div>
        </div>
      ) : (
        <p className="post-body" dangerouslySetInnerHTML={{ __html: linkify(p.content) }} />
      )}

      {p.images && p.images.length > 0 ? (
        <div className="post-images" style={{ gridTemplateColumns: p.images.length > 1 ? '1fr 1fr' : '1fr' }}>
          {p.images.map((img, i) => (
            <img key={i} src={img.original_url} alt="" loading="lazy"
              style={p.images.length > 1 ? { maxHeight: 240 } : undefined} />
          ))}
        </div>
      ) : null}

      <div className="post-actions">
        <button className={likeClass} onClick={() => toggle(p.is_liked ? 'unlike' : 'like')}>
          <IconHeart filled={p.is_liked} />
          <span className="count">{p.likes_count || 0}</span>
        </button>
        <a className="row" style={{ textDecoration: 'none', color: 'var(--ink-2)' }} href={`#/post/${p.id}`}
          onClick={(e) => e.stopPropagation()}>
          <span className="row">
            <IconComment />
            <span className="count">{p.comments_count || 0}</span>
          </span>
        </a>
        <button className={saveClass} style={{ marginLeft: 'auto' }} onClick={() => toggle('save')}>
          <IconBookmark filled={p.is_saved} />
          <span>{p.is_saved ? 'Guardado' : 'Guardar'}</span>
        </button>
      </div>
    </article>
  );
}
