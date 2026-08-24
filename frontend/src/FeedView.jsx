import React, { useState, useEffect, useCallback } from 'react';
import { api } from './api.js';
import PostCard from './PostCard.jsx';

export default function FeedView() {
  const [posts, setPosts] = useState([]);
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const data = await api('/api/feed');
      setPosts(data);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  async function handlePublish(e) {
    e.preventDefault();
    if (!content.trim()) return;
    try {
      await api('/api/posts', {
        method: 'POST',
        body: JSON.stringify({ content }),
      });
      setContent('');
      refresh();
    } catch (err) {
      alert(err.message);
    }
  }

  return (
    <div>
      <h2>Feed</h2>
      <form onSubmit={handlePublish} className="composer">
        <textarea
          className="input textarea"
          placeholder="Que estas pensando?"
          value={content}
          onChange={e => setContent(e.target.value)}
          rows={3}
        />
        <button className="btn btn-primary" type="submit">Publicar</button>
      </form>
      {loading ? <p className="muted">Cargando...</p> : (
        posts.length === 0 ? <p className="muted">No hay publicaciones</p> :
        posts.map(p => <PostCard key={p.id} post={p} onRefresh={refresh} />)
      )}
    </div>
  );
}
