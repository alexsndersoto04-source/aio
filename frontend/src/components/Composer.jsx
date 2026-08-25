import React, { useState } from 'react';
import { useAuth } from '../auth.jsx';
import { api } from '../api.js';
import Avatar from './Avatar.jsx';

const MAX = 500;

export default function Composer({ onPublished }) {
  const { user } = useAuth();
  const [content, setContent] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  async function publish(e) {
    e.preventDefault();
    if (!content.trim() || busy) return;
    setBusy(true);
    setError('');
    try {
      await api('/api/posts', {
        method: 'POST',
        body: JSON.stringify({ content: content.trim() }),
      });
      setContent('');
      if (onPublished) onPublished();
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="card composer" onSubmit={publish}>
      <div className="composer-top">
        <Avatar src={user.avatar_url} name={user.display_name || user.username} size="sm" />
        <textarea
          className="composer-input"
          placeholder={`¿Qué estás haciendo, ${user.display_name || user.username}?`}
          value={content}
          onChange={e => setContent(e.target.value.slice(0, MAX))}
          rows={3}
        />
      </div>
      {error && <p className="form-error">{error}</p>}
      <div className="composer-bottom">
        <span className={`composer-count ${content.length >= MAX ? 'at-limit' : ''}`}>
          {content.length}/{MAX}
        </span>
        <button className="btn btn-primary" type="submit" disabled={busy || !content.trim()}>
          {busy ? 'Publicando…' : 'Publicar'}
        </button>
      </div>
    </form>
  );
}
