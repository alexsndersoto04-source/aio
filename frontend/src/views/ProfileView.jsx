// Moon — Mi perfil (posts / guardados)

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';
import { IconBookmark } from '../components/Icons.jsx';

export default function ProfileView({ tab }) {
  const { user, refreshMe } = useAuth();
  const [me, setMe] = useState(user);
  const [posts, setPosts] = useState([]);
  const [saved, setSaved] = useState([]);
  const [loading, setLoading] = useState(true);
  const [section, setSection] = useState(tab === 'saved' ? 'saved' : 'posts');

  useEffect(() => {
    api.get('/api/auth/me').then(setMe).catch(() => {});
    refreshMe();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    setLoading(true);
    if (section === 'posts') {
      api.get(`/api/users/${me?.id}/posts?page=1&limit=20`)
        .then((res) => setPosts(res.items || []))
        .catch(() => {})
        .finally(() => setLoading(false));
    } else {
      api.get('/api/me/saved?page=1&limit=20')
        .then((res) => setSaved(res.items || []))
        .catch(() => {})
        .finally(() => setLoading(false));
    }
  }, [section, me?.id]);

  if (!me) return <div className="spinner" />;

  return (
    <>
      <div className="card profile-head" style={{ overflow: 'hidden' }}>
        {me.cover_url ? (
          <div className="profile-cover"><img src={me.cover_url} alt="" /></div>
        ) : <div className="profile-cover" />}
        <div className="profile-row">
          <Avatar user={me} size="xl" className="ring" />
          <div style={{ paddingBottom: 10 }}>
            <h2 style={{ margin: 0, fontSize: 21, display: 'flex', alignItems: 'center', gap: 6 }}>
              {me.display_name || me.username} <VerifiedBadge show={me.is_verified} />
            </h2>
            <div className="muted">@{me.username}{me.is_private ? ' · 🔒 privada' : ''}</div>
          </div>
          <a className="btn btn-outline btn-sm" style={{ marginLeft: 'auto', marginBottom: 10 }} href="#/settings">
            Editar perfil
          </a>
        </div>
        <div className="profile-info">
          {me.bio ? <p className="profile-bio">{me.bio}</p> : null}
          <div className="profile-meta">
            {me.location ? <>📍 {me.location} · </> : null}
            Se unió {me.created_at ? timeAgo(me.created_at) : ''} · {me.link ? <a href={me.link.startsWith('http') ? me.link : `https://${me.link}`} target="_blank" rel="noopener noreferrer">{me.link}</a> : null}
          </div>
          <div className="profile-stats">
            <span><b>{me.posts_count}</b><span>publicaciones</span></span>
            <span><b>{me.followers_count}</b><span>seguidores</span></span>
            <span><b>{me.following_count}</b><span>siguiendo</span></span>
          </div>
        </div>
      </div>

      <div className="tabs">
        <button className={section === 'posts' ? 'active' : ''} onClick={() => setSection('posts')}>
          Publicaciones
        </button>
        <button className={section === 'saved' ? 'active' : ''} onClick={() => setSection('saved')}>
          <IconBookmark /> Guardados
        </button>
      </div>

      {loading ? <div className="spinner" /> : null}
      {!loading && section === 'posts' && posts.length === 0 ? (
        <div className="card empty">
          <div className="moon-emoji">🌙</div>
          <h3>Aún no publicaste nada</h3>
          <p>Tu primera publicación aparecerá aquí.</p>
        </div>
      ) : null}
      {!loading && section === 'saved' && saved.length === 0 ? (
        <div className="card empty"><h3>Sin guardados</h3><p>Toca el icono de guardar en cualquier publicación.</p></div>
      ) : null}

      {(section === 'posts' ? posts : saved).map((post) => (
        <PostCard key={post.id} post={post} />
      ))}
    </>
  );
}
