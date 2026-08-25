import React, { useState, useEffect } from 'react';
import PostCard from '../components/PostCard.jsx';
import EmptyState from '../components/EmptyState.jsx';
import { BookmarkIcon } from '../components/Icons.jsx';
import { loadSaved } from '../utils.js';

export default function SavedView() {
  const [items, setItems] = useState(loadSaved);

  useEffect(() => {
    setItems(loadSaved());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [window.location.hash]);

  return (
    <>
      <div className="page-head">
        <h2>Guardados</h2>
        <p className="muted">Se guardan en este dispositivo: solo tú los ves aquí.</p>
      </div>
      {items.length === 0 ? (
        <EmptyState
          icon={BookmarkIcon}
          title="Aún no guardas publicaciones"
          sub="Toca el icono de marcador en cualquier post para tenerlo a la mano."
        />
      ) : (
        items.map(s => <PostCard key={s.post.id} post={s.post} />)
      )}
    </>
  );
}
