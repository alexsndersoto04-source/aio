// Moon — Feed principal (Inicio)

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api.js';
import PostCard from '../components/PostCard.jsx';
import Composer from '../components/Composer.jsx';
import { useAuth } from '../auth.jsx';

const TABS = [
  { id: 'feed', label: 'Para ti' },
  { id: 'trending', label: 'Tendencias' },
  { id: 'latest', label: 'Recientes' },
];

export default function FeedView() {
  const { user } = useAuth();
  const [tab, setTab] = useState('feed');
  const [posts, setPosts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [more, setMore] = useState(false);
  const loadRef = useRef(null);

  const load = useCallback(async (t, p, append) => {
    try {
      const path = t === 'feed' ? '/api/feed' : `/api/feed/${t}`;
      const res = await api.get(`${path}?page=${p}&limit=10`);
      const items = res.items || [];
      setPosts((prev) => (append ? [...prev, ...items] : items));
      setTotal(res.total || items.length);
      setMore(items.length === 10 && (p * 10) < (res.total || 0));
    } catch (e) {
      alert(e.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    setLoading(true);
    setPage(1);
    load(tab, 1, false);
  }, [tab, load]);

  // Scroll infinito
  useEffect(() => {
    const el = loadRef.current;
    if (!el) return;
    const obs = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting && !loading && more) {
        setPage((p) => {
          const next = p + 1;
          load(tab, next, true);
          return next;
        });
      }
    }, { rootMargin: '300px' });
    obs.observe(el);
    return () => obs.disconnect();
  }, [loading, more, tab, load]);

  function onCreated(post) {
    setPosts((prev) => [post, ...prev]);
  }

  return (
    <>
      <div className="topbar">
        <h1>{tab === 'feed' ? 'Inicio' : tab === 'trending' ? 'Tendencias' : 'Recientes'}</h1>
      </div>

      {tab === 'feed' ? <Composer onCreated={onCreated} /> : null}

      <div className="tabs">
        {TABS.map((t) => (
          <button key={t.id} className={tab === t.id ? 'active' : ''} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>

      {loading ? <div className="spinner" /> : null}

      {!loading && posts.length === 0 ? (
        <div className="card empty">
          <div className="moon-emoji">🌙</div>
          <h3>Sin publicaciones todavía</h3>
          <p>Sigue a personas para llenar tu feed, o publica algo tú mismo.</p>
          <a className="btn" href="#/explore">Explorar</a>
        </div>
      ) : null}

      {posts.map((post) => (
        <PostCard key={post.id} post={post}
          onChanged={(next) => setPosts((prev) => prev.map((x) => (x.id === next.id ? next : x)))} />
      ))}

      <div ref={loadRef} />
      {more ? <div className="spinner" style={{ margin: '12px auto' }} /> : null}
    </>
  );
}
