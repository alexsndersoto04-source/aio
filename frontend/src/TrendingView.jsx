import React, { useState, useEffect } from 'react';
import { api } from './api.js';
import PostCard from './PostCard.jsx';

export default function TrendingView() {
  const [posts, setPosts] = useState([]);
  const [loading, setLoading] = useState(true);

  async function refresh() {
    try {
      const data = await api('/api/feed/trending');
      setPosts(data);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { refresh(); }, []);

  return (
    <div>
      <h2>Tendencias</h2>
      {loading ? <p className="muted">Cargando...</p> : (
        posts.length === 0 ? <p className="muted">No hay publicaciones</p> :
        posts.map(p => <PostCard key={p.id} post={p} onRefresh={refresh} />)
      )}
    </div>
  );
}
