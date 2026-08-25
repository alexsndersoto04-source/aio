import React, { useState, useEffect } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import Avatar from '../components/Avatar.jsx';
import PostCard from '../components/PostCard.jsx';
import EmptyState from '../components/EmptyState.jsx';
import { ChatIcon } from '../components/Icons.jsx';

export default function ProfileView({ userId }) {
  const { user: me } = useAuth();
  const [profile, setProfile] = useState(null);
  const [isFollowing, setIsFollowing] = useState(false);
  const [posts, setPosts] = useState([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  async function refresh() {
    try {
      const d = await api(`/api/users/${userId}`);
      setProfile(d.user);
      setIsFollowing(!!d.is_following);
      setPosts(await api(`/api/users/${userId}/posts`));
      setError('');
    } catch (err) {
      setProfile(null);
      setError(err.message);
    }
  }

  useEffect(() => {
    setProfile(null);
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [userId]);

  async function toggleFollow() {
    if (busy) return;
    setBusy(true);
    setError('');
    try {
      await api(isFollowing ? `/api/unfollow/${userId}` : `/api/follow/${userId}`, {
        method: 'POST',
      });
      await refresh();
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  if (error && !profile) {
    return <EmptyState title="No se pudo cargar el perfil" sub={error} />;
  }
  if (!profile) {
    return (
      <div className="skeleton-list">
        <div className="card sk" style={{ height: 220 }} />
        <div className="card sk" />
      </div>
    );
  }

  const isMe = String(me.id) === String(userId);

  return (
    <>
      <div className="card profile-card">
        <div className="profile-cover" />
        <div className="profile-body">
          <Avatar
            src={profile.avatar_url}
            name={profile.display_name || profile.username}
            size="xl"
            className="profile-avatar"
          />
          <div className="profile-head">
            <h2>{profile.display_name || profile.username}</h2>
            <span className="muted">@{profile.username}</span>
            <div className="profile-actions">
              {isMe ? (
                <a className="btn btn-ghost" href="#/settings">
                  Ajustes
                </a>
              ) : (
                <>
                  <button
                    className={`btn ${isFollowing ? 'btn-ghost' : 'btn-primary'}`}
                    onClick={toggleFollow}
                    disabled={busy}
                  >
                    {isFollowing ? 'Siguiendo' : 'Seguir'}
                  </button>
                  <a className="btn btn-ghost" href={`#/messages/${userId}`}>
                    <ChatIcon size={15} /> Mensaje
                  </a>
                </>
              )}
            </div>
          </div>
          {profile.bio && <p className="profile-bio">{profile.bio}</p>}
          <div className="profile-stats">
            <span>
              <strong>{profile.posts_count ?? 0}</strong> Publicaciones
            </span>
            <span>
              <strong>{profile.following_count ?? 0}</strong> Siguiendo
            </span>
            <span>
              <strong>{profile.followers_count ?? 0}</strong> Seguidores
            </span>
          </div>
        </div>
      </div>
      {error && <p className="form-error">{error}</p>}
      <h3 className="section-title">Publicaciones</h3>
      {posts.length === 0 ? (
        <EmptyState
          title="Sin publicaciones"
          sub={isMe ? 'Tu contenido aparecerá aquí.' : 'Este usuario aún no ha publicado.'}
        />
      ) : (
        posts.map(p => <PostCard key={p.id} post={p} onRefresh={refresh} />)
      )}
    </>
  );
}
