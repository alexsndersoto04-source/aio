import React, { useEffect, useState } from 'react';
import { useAuth } from '../auth.jsx';
import { api } from '../api.js';
import Avatar from './Avatar.jsx';

export default function RightRail() {
  const { user } = useAuth();
  const [trending, setTrending] = useState([]);

  useEffect(() => {
    api('/api/feed/trending').then(setTrending).catch(() => {});
  }, []);

  return (
    <aside className="rightrail">
      <a className="card rail-card rail-me" href={`#/profile/${user.id}`}>
        <Avatar src={user.avatar_url} name={user.display_name || user.username} size="md" />
        <div className="rail-me-text">
          <strong>{user.display_name || user.username}</strong>
          <span>@{user.username}</span>
        </div>
      </a>
      <div className="rail-stats">
        <div>
          <strong>{user.posts_count ?? 0}</strong>
          <span>Posts</span>
        </div>
        <div>
          <strong>{user.following_count ?? 0}</strong>
          <span>Siguiendo</span>
        </div>
        <div>
          <strong>{user.followers_count ?? 0}</strong>
          <span>Seguidores</span>
        </div>
      </div>
      <div className="card rail-card">
        <h3 className="rail-title">Tendencias</h3>
        {trending.length === 0 ? (
          <p className="muted sm">Aún no hay actividad en la comunidad.</p>
        ) : (
          trending.slice(0, 5).map((p, i) => (
            <div key={p.id} className="trend-item">
              <span className="trend-rank">{i + 1}</span>
              <div className="trend-body">
                <p className="trend-text">{p.content}</p>
                <span className="muted sm">
                  @{p.username} · {p.likes_count ?? 0} me gusta
                </span>
              </div>
            </div>
          ))
        )}
      </div>
      <p className="rail-foot">© 2026 Moon · Construido con Titan</p>
    </aside>
  );
}
