// Moon — Perfil de otro usuario (seguir / bloquear / mensaje)

import React, { useCallback, useEffect, useState } from 'react';
import { api, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';

export default function UserView({ id }) {
  const { user } = useAuth();
  const [profile, setProfile] = useState(null);
  const [posts, setPosts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [notFound, setNotFound] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      // Resolver username -> id vía búsqueda exacta
      let uid = Number(id);
      if (isNaN(uid)) {
        const res = await api.get(`/api/search?q=${encodeURIComponent(id)}&type=users`);
        const hit = (res || []).find((u) => u.username.toLowerCase() === id.toLowerCase());
        if (!hit) { setNotFound(true); return; }
        uid = hit.id;
      }
      const prof = await api.get(`/api/users/${uid}`);
      setProfile(prof);
      const p = await api.get(`/api/users/${uid}/posts?page=1&limit=20`);
      setPosts(p.items || []);
    } catch (e) {
      if (e.status === 403) {
        setProfile({ blocked: true });
      } else if (e.status === 404) {
        setNotFound(true);
      } else {
        alert(e.message);
      }
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => { load(); }, [load]);

  async function follow() {
    setBusy(true);
    try {
      if (profile.is_following) await api.del(`/api/users/${profile.id}/follow`);
      else await api.post(`/api/users/${profile.id}/follow`, {});
      setProfile({ ...profile, is_following: !profile.is_following });
    } catch (e) { alert(e.message); }
    finally { setBusy(false); }
  }

  async function block() {
    if (!window.confirm(profile.is_blocked ? '¿Desbloquear a este usuario?' : '¿Bloquear a este usuario?')) return;
    setBusy(true);
    try {
      if (profile.is_blocked) await api.del(`/api/users/${profile.id}/block`);
      else await api.post(`/api/users/${profile.id}/block`, {});
      setProfile({ ...profile, is_blocked: !profile.is_blocked, is_following: false });
    } catch (e) { alert(e.message); }
    finally { setBusy(false); }
  }

  async function message() {
    try {
      const res = await api.post('/api/messages/conversations', { user_id: profile.id });
      window.location.hash = `#/messages/${res.conversation_id}`;
    } catch (e) { alert(e.message); }
  }

  if (loading) return <div className="spinner" />;
  if (notFound) return <div className="card empty"><h3>Usuario no encontrado</h3></div>;
  if (profile?.blocked) {
    return (
      <div className="card empty">
        <h3>No puedes ver este perfil</h3>
        <p>Estás bloqueado por este usuario o el contenido no está disponible.</p>
      </div>
    );
  }

  const isMe = user && user.id === profile.id;

  return (
    <>
      <div className="card profile-head" style={{ overflow: 'hidden' }}>
        {profile.cover_url ? <div className="profile-cover"><img src={imgUrl(profile.cover_url)} alt="" /></div> : <div className="profile-cover" />}
        <div className="profile-row">
          <Avatar user={profile} size="xl" className="ring" />
          <div style={{ paddingBottom: 10 }}>
            <h2 style={{ margin: 0, fontSize: 21, display: 'flex', alignItems: 'center', gap: 6 }}>
              {profile.display_name || profile.username} <VerifiedBadge show={profile.is_verified} />
            </h2>
            <div className="muted">@{profile.username}{profile.is_private ? ' · 🔒 privada' : ''}</div>
          </div>
          <div className="row" style={{ marginLeft: 'auto', marginBottom: 10 }}>
            {!isMe ? (
              <>
                <button className="btn btn-sm" disabled={busy} onClick={message}>Mensaje</button>
                <button className={profile.is_following ? 'btn btn-outline btn-sm' : 'btn btn-sm'} disabled={busy} onClick={follow}>
                  {profile.is_following ? 'Siguiendo' : 'Seguir'}
                </button>
                <button className="btn btn-ghost btn-sm" disabled={busy} onClick={block}>
                  {profile.is_blocked ? 'Desbloquear' : 'Bloquear'}
                </button>
              </>
            ) : null}
          </div>
        </div>
        <div className="profile-info">
          {profile.bio ? <p className="profile-bio">{profile.bio}</p> : null}
          <div className="profile-meta">
            {profile.location ? <>📍 {profile.location} · </> : null}
            Se unió {timeAgo(profile.created_at)}
          </div>
          <div className="profile-stats">
            <span><b>{profile.posts_count}</b><span>publicaciones</span></span>
            <span><b>{profile.followers_count}</b><span>seguidores</span></span>
            <span><b>{profile.following_count}</b><span>siguiendo</span></span>
          </div>
        </div>
      </div>

      {posts.length === 0 ? (
        <div className="card empty"><p>{isMe ? 'Sin publicaciones todavía.' : 'Este usuario aún no publica.'}</p></div>
      ) : (
        posts.map((post) => <PostCard key={post.id} post={post} />)
      )}
    </>
  );
}
