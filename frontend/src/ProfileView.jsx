import React, { useState, useEffect } from 'react';
import { api } from './api.js';
import { useAuth } from './auth.jsx';
import PostCard from './PostCard.jsx';

export default function ProfileView({ userId }) {
  const { user: me } = useAuth();
  const [profile, setProfile] = useState(null);
  const [isFollowing, setIsFollowing] = useState(false);
  const [posts, setPosts] = useState([]);
  const [loading, setLoading] = useState(true);

  async function refresh() {
    try {
      const data = await api(`/api/users/${userId}`);
      setProfile(data.user);
      setIsFollowing(data.is_following);
      const postData = await api(`/api/users/${userId}/posts`);
      setPosts(postData);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { setLoading(true); refresh(); }, [userId]);

  async function toggleFollow() {
    try {
      if (isFollowing) {
        await api(`/api/unfollow/${userId}`, { method: 'POST' });
      } else {
        await api(`/api/follow/${userId}`, { method: 'POST' });
      }
      refresh();
    } catch (err) {
      alert(err.message);
    }
  }

  if (loading) return <p className="muted">Cargando...</p>;
  if (!profile) return <p className="muted">Usuario no encontrado</p>;

  const avatar = profile.avatar_url
    ? <img src={profile.avatar_url} alt="" className="avatar avatar-lg" />
    : <div className="avatar avatar-lg avatar-placeholder">{(profile.username || '?')[0].toUpperCase()}</div>;

  const isMe = String(me.id) === String(userId);

  return (
    <div>
      <div className="profile-header">
        {avatar}
        <div>
          <h2>@{profile.username}</h2>
          {profile.display_name && <p className="display-name">{profile.display_name}</p>}
          {profile.bio && <p className="bio">{profile.bio}</p>}
          <div className="profile-stats">
            <span>{profile.followers_count ?? 0} seguidores</span>
            <span>{profile.following_count ?? 0} siguiendo</span>
            <span>{profile.posts_count ?? 0} publicaciones</span>
          </div>
          {!isMe && (
            <div className="profile-actions">
              <button className="btn btn-primary" onClick={toggleFollow}>
                {isFollowing ? 'Dejar de seguir' : 'Seguir'}
              </button>
              <a href={`#/messages/${userId}`} className="btn">Mensaje</a>
            </div>
          )}
        </div>
      </div>
      <h3>Publicaciones</h3>
      {posts.length === 0 ? <p className="muted">Sin publicaciones</p> :
        posts.map(p => <PostCard key={p.id} post={p} onRefresh={refresh} />)
      }
    </div>
  );
}
