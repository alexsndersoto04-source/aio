import React, { useState, useEffect } from 'react';
import { api } from '../api.js';
import Composer from '../components/Composer.jsx';
import PostCard from '../components/PostCard.jsx';
import EmptyState from '../components/EmptyState.jsx';

export default function FeedView() {
  const [posts, setPosts] = useState(null); // null = cargando

  async function refresh() {
    try {
      setPosts(await api('/api/feed'));
    } catch {
      setPosts(p => (p === null ? [] : p));
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <>
      <Composer onPublished={refresh} />
      {posts === null ? (
        <div className="skeleton-list">
          <div className="card sk" />
          <div className="card sk" />
          <div className="card sk" />
        </div>
      ) : posts.length === 0 ? (
        <EmptyState
          title="Tu inicio está en silencio"
          sub="Sigue a alguien desde Explorar o crea tu primera publicación."
        />
      ) : (
        posts.map(p => <PostCard key={p.id} post={p} onRefresh={refresh} />)
      )}
    </>
  );
}
