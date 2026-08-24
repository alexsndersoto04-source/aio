import React, { useState } from 'react';
import { api } from './api.js';

export default function PostCard({ post, onRefresh }) {
  const [showComments, setShowComments] = useState(false);
  const [comments, setComments] = useState([]);
  const [commentText, setCommentText] = useState('');
  const [loadingComments, setLoadingComments] = useState(false);

  async function handleLike() {
    try {
      await api(`/api/posts/${post.id}/like`, { method: 'POST' });
    } catch (err) {
      if (!err.message.includes('409')) {
        alert(err.message);
      }
    }
    if (onRefresh) onRefresh();
  }

  async function toggleComments() {
    if (showComments) {
      setShowComments(false);
      return;
    }
    setLoadingComments(true);
    try {
      const data = await api(`/api/posts/${post.id}/comments`);
      setComments(data);
      setShowComments(true);
    } catch (err) {
      alert(err.message);
    } finally {
      setLoadingComments(false);
    }
  }

  async function submitComment(e) {
    e.preventDefault();
    if (!commentText.trim()) return;
    try {
      await api(`/api/posts/${post.id}/comment`, {
        method: 'POST',
        body: JSON.stringify({ content: commentText }),
      });
      setCommentText('');
      const data = await api(`/api/posts/${post.id}/comments`);
      setComments(data);
      if (onRefresh) onRefresh();
    } catch (err) {
      alert(err.message);
    }
  }

  const avatar = post.avatar_url
    ? <img src={post.avatar_url} alt="" className="avatar" />
    : <div className="avatar avatar-placeholder">{(post.username || '?')[0].toUpperCase()}</div>;

  return (
    <div className="post-card">
      <div className="post-header">
        <a href={`#/profile/${post.user_id}`} className="post-user-link">
          {avatar}
          <span className="post-username">@{post.username}</span>
        </a>
        <span className="post-date">{new Date(post.created_at).toLocaleString()}</span>
      </div>
      <p className="post-content">{post.content}</p>
      <div className="post-actions">
        <button className="btn btn-sm" onClick={handleLike}>
          {post.likes_count ?? 0} Like
        </button>
        <button className="btn btn-sm" onClick={toggleComments} disabled={loadingComments}>
          {post.comments_count ?? 0} Comentarios
        </button>
      </div>
      {showComments && (
        <div className="comments-section">
          {comments.length === 0 && <p className="muted">Sin comentarios</p>}
          {comments.map(c => (
            <div key={c.id} className="comment-item">
              <strong>@{c.username}</strong> {c.content}
            </div>
          ))}
          <form onSubmit={submitComment} className="comment-form">
            <input
              className="input"
              placeholder="Escribe un comentario..."
              value={commentText}
              onChange={e => setCommentText(e.target.value)}
            />
            <button className="btn btn-sm btn-primary" type="submit">Enviar</button>
          </form>
        </div>
      )}
    </div>
  );
}
