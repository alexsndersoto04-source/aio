import React, { useState, useEffect } from 'react';
import { api } from '../api.js';
import PostCard from '../components/PostCard.jsx';
import EmptyState from '../components/EmptyState.jsx';
import { CompassIcon } from '../components/Icons.jsx';

export default function ExploreView() {
  const [posts, setPosts] = useState(null);

  async function refresh() {
    try {
      setPosts(await api('/api/feed/trending'));
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
      <div className="page-head">
        <h2>Explorar</h2>
        <p className="muted">Lo más activo de la comunidad ahora mismo.</p>
      </div>
      {posts === null ? (
        <div className="skeleton-list">
          <div className="card sk" />
          <div className="card sk" />
        </div>
      ) : posts.length === 0 ? (
        <EmptyState
          icon={CompassIcon}
          title="Todavía no hay tendencias"
          sub="Cuando la comunidad empiece a publicar, lo más activo aparecerá aquí."
        />
      ) : (
        posts.map(p => <PostCard key={p.id} post={p} onRefresh={refresh} />)
      )}
    </>
  );
}
