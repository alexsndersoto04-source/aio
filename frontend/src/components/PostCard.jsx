import React, { useState, useEffect } from 'react';
import { useAuth } from '../auth.jsx';
import { api } from '../api.js';
import Avatar from './Avatar.jsx';
import { HeartIcon, CommentIcon, BookmarkIcon, SendIcon } from './Icons.jsx';
import { timeAgo, toggleSaved, isSaved } from '../utils.js';

export default function PostCard({ post, onRefresh, showSave = true }) {
  const { user } = useAuth();
  const [liked, setLiked] = useState(null);
  const [likesCount, setLikesCount] = useState(post.likes_count ?? 0);
  const [saved, setSaved] = useState(() => isSaved(post));
  const [open, setOpen] = useState(false);
  const [comments, setComments] = useState([]);
  const [loadingComments, setLoadingComments] = useState(false);
  const [text, setText] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    api(`/api/posts/${post.id}`)
      .then(d => setLiked(!!d.is_liked))
      .catch(() => {});
  }, [post.id]);

  async function toggleLike() {
    if (liked === null) return;
    setError('');
    try {
      if (liked) {
        await api(`/api/posts/${post.id}/unlike`, { method: 'POST' });
        setLiked(false);
        setLikesCount(c => Math.max(0, c - 1));
      } else {
        await api(`/api/posts/${post.id}/like`, { method: 'POST' });
        setLiked(true);
        setLikesCount(c => c + 1);
      }
      if (onRefresh) onRefresh();
    } catch (err) {
      if (err.message.includes('409')) setLiked(true);
      else setError(err.message);
    }
  }

  function onToggleSave() {
    setError('');
    setSaved(toggleSaved(post));
  }

  async function loadComments() {
    setLoadingComments(true);
    try {
      setComments(await api(`/api/posts/${post.id}/comments`));
    } catch (err) {
      setError(err.message);
    } finally {
      setLoadingComments(false);
    }
  }

  function toggleComments() {
    if (open) {
      setOpen(false);
      return;
    }
    setOpen(true);
    if (comments.length === 0 && !loadingComments) loadComments();
  }

  async function submitComment(e) {
    e.preventDefault();
    if (!text.trim() || sending) return;
    setSending(true);
    setError('');
    try {
      await api(`/api/posts/${post.id}/comment`, {
        method: 'POST',
        body: JSON.stringify({ content: text.trim() }),
      });
      setText('');
      await loadComments();
      if (onRefresh) onRefresh();
    } catch (err) {
      setError(err.message);
    } finally {
      setSending(false);
    }
  }

  return (
    <article className="card post-card">
      <div className="post-head">
        <a href={`#/profile/${post.user_id}`}>
          <Avatar src={post.avatar_url} name={post.display_name || post.username} size="md" />
        </a>
        <div className="post-who">
          <a className="post-name" href={`#/profile/${post.user_id}`}>
            {post.display_name || post.username}
          </a>
          <span className="post-sub">
            @{post.username} · {timeAgo(post.created_at)}
          </span>
        </div>
      </div>
      <p className="post-text">{post.content}</p>
      <div className="post-actions">
        <button
          className={`action-btn ${liked ? 'is-liked' : ''}`}
          onClick={toggleLike}
          disabled={liked === null}
          title="Me gusta"
        >
          <HeartIcon size={17} filled={!!liked} /> {likesCount}
        </button>
        <button className="action-btn" onClick={toggleComments} title="Comentarios">
          <CommentIcon size={17} /> {post.comments_count ?? 0}
        </button>
        {showSave && (
          <button
            className={`action-btn post-save ${saved ? 'is-saved' : ''}`}
            onClick={onToggleSave}
            title={saved ? 'Quitar de guardados' : 'Guardar'}
          >
            <BookmarkIcon size={17} filled={saved} />
          </button>
        )}
      </div>
      {error && <p className="form-error">{error}</p>}
      {open && (
        <div className="comments">
          {loadingComments ? (
            <p className="muted sm">Cargando comentarios…</p>
          ) : comments.length === 0 ? (
            <p className="muted sm">Sé la primera persona en comentar.</p>
          ) : (
            comments.map(c => (
              <div key={c.id} className="comment">
                <Avatar src={c.avatar_url} name={c.display_name || c.username} size="xs" />
                <div className="comment-body">
                  <span className="comment-name">{c.display_name || c.username}</span>
                  <span className="comment-text">{c.content}</span>
                </div>
                <span className="muted xs">{timeAgo(c.created_at)}</span>
              </div>
            ))
          )}
          <form className="comment-form" onSubmit={submitComment}>
            <Avatar src={user.avatar_url} name={user.display_name || user.username} size="xs" />
            <input
              className="input sm"
              placeholder="Escribe un comentario…"
              value={text}
              onChange={e => setText(e.target.value)}
            />
            <button
              className="icon-btn icon-btn-primary"
              type="submit"
              disabled={sending || !text.trim()}
              title="Comentar"
            >
              <SendIcon size={15} />
            </button>
          </form>
        </div>
      )}
    </article>
  );
}
