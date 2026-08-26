// Moon — Explorar: búsqueda real (usuarios/posts/hashtags)

import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../api.js';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { debounce } from '../utils.js';
import { IconSearch } from '../components/Icons.jsx';

export default function ExploreView({ initialQ = '', initialType = 'users' }) {
  const [q, setQ] = useState(initialQ);
  const [type, setType] = useState(initialType === 'posts' ? 'posts' : 'users');
  const [users, setUsers] = useState([]);
  const [posts, setPosts] = useState([]);
  const [trendingTags, setTrendingTags] = useState([]);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);

  const runSearch = useCallback(debounce(async (query, t) => {
    if (!query.trim()) {
      setUsers([]);
      setPosts([]);
      setSearched(false);
      return;
    }
    setLoading(true);
    try {
      if (t === 'users') {
        const res = await api.get(`/api/search?q=${encodeURIComponent(query.trim())}&type=users`);
        setUsers(res || []);
        setPosts([]);
      } else {
        const res = await api.get(`/api/search?q=${encodeURIComponent(query.trim())}&type=posts`);
        setPosts(res || []);
        setUsers([]);
      }
      setSearched(true);
    } catch (e) {
      alert(e.message);
    } finally {
      setLoading(false);
    }
  }, 400), []);

  useEffect(() => {
    api.get('/api/hashtags').then(setTrendingTags).catch(() => {});
  }, []);

  // Sincroniza cuando la URL cambia (p. ej. clic en #hashtag de un post).
  useEffect(() => {
    if (!initialQ) return;
    const t = initialType === 'posts' ? 'posts' : 'users';
    setQ(initialQ);
    setType(t);
    runSearch(initialQ, t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialQ, initialType]);

  function onType(t) {
    setType(t);
    if (q.trim()) runSearch(q, t);
  }

  return (
    <>
      <div className="topbar"><h1>Explorar</h1></div>

      <div className="card" style={{ padding: 14, marginBottom: 16 }}>
        <div className="row" style={{ gap: 8 }}>
          <IconSearch style={{ color: 'var(--ink-3)', width: 20, height: 20 }} />
          <input
            className="input"
            style={{ border: 'none', boxShadow: 'none', padding: '8px 4px' }}
            placeholder="Busca personas, publicaciones…"
            value={q}
            onChange={(e) => { setQ(e.target.value); runSearch(e.target.value, type); }}
          />
        </div>
        <div className="tabs" style={{ borderBottom: 'none', marginBottom: 0, marginTop: 6 }}>
          <button className={type === 'users' ? 'active' : ''} onClick={() => onType('users')}>Personas</button>
          <button className={type === 'posts' ? 'active' : ''} onClick={() => onType('posts')}>Publicaciones</button>
        </div>
      </div>

      {loading ? <div className="spinner" /> : null}

      {!loading && !q.trim() ? (
        <div className="card" style={{ padding: 16 }}>
          <h3 style={{ margin: '0 0 8px', fontSize: 15, fontWeight: 800 }}>Tendencias</h3>
          {trendingTags.map((t) => (
            <a key={t.tag} href={`#/explore?q=${encodeURIComponent(t.tag)}&type=posts`}
              className="row" style={{ padding: '9px 4px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span className="hash">#{t.tag}</span>
              <span className="muted">{t.posts_count} publicaciones</span>
            </a>
          ))}
          {trendingTags.length === 0 ? <p className="muted" style={{ margin: 0 }}>Aún no hay hashtags.</p> : null}
        </div>
      ) : null}

      {!loading && searched && type === 'users' ? (
        users.length === 0 ? (
          <div className="card empty"><p>Sin resultados para «{q}».</p></div>
        ) : (
          users.map((u) => (
            <a key={u.id} href={`#/user/${u.username}`} className="card suggest" style={{ marginBottom: 10, textDecoration: 'none' }}>
              <Avatar user={u} />
              <span className="who">
                <span className="name" style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                  {u.display_name || u.username} <VerifiedBadge show={u.is_verified} />
                </span>
                <span className="at">@{u.username}</span>
              </span>
              <span className="muted">{u.followers_count} seguidores</span>
            </a>
          ))
        )
      ) : null}

      {!loading && searched && type === 'posts' ? (
        posts.length === 0 ? (
          <div className="card empty"><p>Sin publicaciones para «{q}».</p></div>
        ) : (
          posts.map((post) => <PostCard key={post.id} post={post} />)
        )
      ) : null}
    </>
  );
}
