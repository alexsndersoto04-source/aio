// Moon Watch — Feed nativo idéntico al diseño de Facebook Watch
// ============================================================
// - Estructura limpia y plana (sin marcos desbordados ni modales).
// - Video borde a borde de 16:9 que encaja exacto en móvil y PC.
// - Barra de creador con avatar, nombre, botón Seguir.
// - Barra inferior de Me gusta, Comentar, Compartir.

import React, { useState, useEffect } from 'react';
import {
  IconSearch, IconX, IconHeart, IconComment, IconSend,
} from '../components/Icons.jsx';

const TOPICS = [
  { id: 'trending', label: 'Para ti' },
  { id: 'music', label: 'Música', canal: 'music' },
  { id: 'fun', label: 'Humor', canal: 'fun' },
  { id: 'gaming', label: 'Videojuegos', canal: 'videogames' },
  { id: 'sport', label: 'Deportes', canal: 'sport' },
  { id: 'news', label: 'Noticias', canal: 'news' },
  { id: 'tech', label: 'Tecnología', canal: 'tech' },
];

function formatTime(s) {
  if (!s) return '0:00';
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${sec < 10 ? '0' : ''}${sec}`;
}

function formatViews(n) {
  const num = Number(n || 0);
  if (num > 1000000) return `${(num / 1000000).toFixed(1)} M`;
  if (num > 1000) return `${(num / 1000).toFixed(0)} mil`;
  return String(num || 120);
}

export default function VideosView() {
  const [topic, setTopic] = useState('trending');
  const [query, setQuery] = useState('');
  const [searchTerm, setSearchTerm] = useState('');
  const [videos, setVideos] = useState([]);
  const [loading, setLoading] = useState(true);
  const [playingId, setPlayingId] = useState(null);
  const [liked, setLiked] = useState({});
  const [following, setFollowing] = useState({});

  useEffect(() => {
    let active = true;
    setLoading(true);
    setPlayingId(null);

    async function fetchVideos() {
      try {
        let url = 'https://api.dailymotion.com/videos?fields=id,title,thumbnail_720_url,thumbnail_360_url,duration,owner.screenname,owner.username,views_total,created_time&limit=15';

        if (searchTerm.trim()) {
          url += `&search=${encodeURIComponent(searchTerm.trim())}`;
        } else {
          const currentTopic = TOPICS.find((t) => t.id === topic);
          if (currentTopic && currentTopic.canal) {
            url += `&channel=${currentTopic.canal}`;
          } else {
            url += '&sort=trending';
          }
        }

        const res = await fetch(url);
        if (!res.ok) throw new Error('Error de red');
        const data = await res.json();

        if (active) {
          setVideos(data.list || []);
          setLoading(false);
        }
      } catch (err) {
        if (active) {
          console.error('[watch] error:', err);
          setLoading(false);
        }
      }
    }

    fetchVideos();
    return () => { active = false; };
  }, [topic, searchTerm]);

  function handleSearch(e) {
    e.preventDefault();
    setSearchTerm(query.trim());
  }

  return (
    <div className="videos-shell">
      {/* 1. Cabecera Facebook Watch */}
      <div className="watch-fb-header">
        <div className="watch-fb-title-bar">
          <h1 className="watch-fb-title">Videos</h1>
        </div>

        <form className="watch-fb-searchbox" onSubmit={handleSearch}>
          <IconSearch />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar videos en Watch..."
          />
          {query ? (
            <button
              type="button"
              className="clear-btn"
              onClick={() => { setQuery(''); setSearchTerm(''); }}
            >
              <IconX />
            </button>
          ) : null}
        </form>
      </div>

      {/* 2. Barra de temas horizontales (Pestañas estilo Watch) */}
      <div className="watch-fb-tabs">
        {TOPICS.map((t) => (
          <button
            key={t.id}
            type="button"
            className={`watch-fb-pill ${topic === t.id && !searchTerm ? 'active' : ''}`}
            onClick={() => { setSearchTerm(''); setTopic(t.id); }}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* 3. Feed de videos */}
      <div className="watch-fb-feed">
        {loading && Array.from({ length: 3 }).map((_, i) => (
          <div className="watch-skeleton-card" key={i} />
        ))}

        {!loading && videos.map((vid) => {
          const isPlaying = playingId === vid.id;
          const author = vid['owner.screenname'] || vid['owner.username'] || 'Creador de Video';
          const isLiked = Boolean(liked[vid.id]);
          const isFollowed = Boolean(following[author]);

          return (
            <article key={vid.id} className="watch-fb-card">
              {/* Cabecera del video */}
              <div className="watch-card-header">
                <div className="watch-creator-info">
                  <div className="watch-avatar-circle">
                    {author.charAt(0).toUpperCase()}
                  </div>
                  <div className="watch-creator-texts">
                    <span className="watch-creator-name">{author}</span>
                    <span className="watch-post-date">Sugerido para ti · 🌐</span>
                  </div>
                </div>

                <button
                  type="button"
                  className="watch-follow-btn"
                  onClick={() => setFollowing((prev) => ({ ...prev, [author]: !prev[author] }))}
                >
                  {isFollowed ? 'Siguiendo' : 'Seguir'}
                </button>
              </div>

              {/* Título de la publicación */}
              <p className="watch-card-text">{vid.title}</p>

              {/* Video o Portada (ancho 100% borde a borde sin desbordes) */}
              <div className="watch-media-box">
                {isPlaying ? (
                  <iframe
                    title={vid.title}
                    src={`https://www.dailymotion.com/embed/video/${vid.id}?autoplay=1&ui-logo=0&ui-start-screen-info=0&sharing-enable=0`}
                    allowFullScreen
                    allow="autoplay; fullscreen"
                  />
                ) : (
                  <div
                    className="watch-thumbnail-wrapper"
                    onClick={() => setPlayingId(vid.id)}
                    role="button"
                    tabIndex={0}
                  >
                    <img
                      src={vid.thumbnail_720_url || vid.thumbnail_360_url}
                      alt={vid.title}
                      loading="lazy"
                    />
                    <span className="watch-duration-tag">{formatTime(vid.duration)}</span>
                    <div className="watch-play-button-overlay">
                      <div className="watch-play-circle-icon">▶</div>
                    </div>
                  </div>
                )}
              </div>

              {/* Estadísticas de reproducciones estilo Facebook */}
              <div className="watch-stats-row">
                <span>{formatViews(vid.views_total)} reproducciones</span>
                <span>{isLiked ? 'Tú y otros' : 'Reacciones'}</span>
              </div>

              {/* Barra de 3 botones inferiores: Me gusta, Comentar, Compartir */}
              <div className="watch-fb-actions-bar">
                <button
                  type="button"
                  className={`watch-fb-action-button ${isLiked ? 'liked' : ''}`}
                  onClick={() => setLiked((prev) => ({ ...prev, [vid.id]: !prev[vid.id] }))}
                >
                  <IconHeart filled={isLiked} /> {isLiked ? 'Me gusta' : 'Me gusta'}
                </button>
                <button
                  type="button"
                  className="watch-fb-action-button"
                  onClick={() => setPlayingId(vid.id)}
                >
                  <IconComment /> Comentar
                </button>
                <button
                  type="button"
                  className="watch-fb-action-button"
                  onClick={() => {
                    const shareUrl = `https://www.dailymotion.com/video/${vid.id}`;
                    if (navigator.share) {
                      navigator.share({ title: vid.title, url: shareUrl }).catch(() => {});
                    } else {
                      navigator.clipboard?.writeText(shareUrl);
                      alert('Enlace copiado al portapapeles');
                    }
                  }}
                >
                  <IconSend /> Compartir
                </button>
              </div>
            </article>
          );
        })}
      </div>
    </div>
  );
}
