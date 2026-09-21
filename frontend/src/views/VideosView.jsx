// Moon Watch — Feed nativo de videos estilo Facebook Watch
// ============================================================
// Diseño limpio, claro y profesional:
// - Sin modales flotantes raros.
// - Tarjetas limpias de ancho completo con avatar, título e interacción.
// - Reproductor integrado directamente en el feed.

import React, { useState, useEffect } from 'react';
import {
  IconSearch, IconX, IconHeart, IconComment, IconSend,
} from '../components/Icons.jsx';

const CATEGORIAS = [
  { id: 'trending', label: 'Para ti' },
  { id: 'music', label: 'Música', canal: 'music' },
  { id: 'gaming', label: 'Videojuegos', canal: 'videogames' },
  { id: 'fun', label: 'Humor', canal: 'fun' },
  { id: 'sport', label: 'Deportes', canal: 'sport' },
  { id: 'news', label: 'Noticias', canal: 'news' },
  { id: 'tech', label: 'Tecnología', canal: 'tech' },
];

function formatearSegundos(seg) {
  if (!seg) return '0:00';
  const m = Math.floor(seg / 60);
  const s = Math.floor(seg % 60);
  return `${m}:${s < 10 ? '0' : ''}${s}`;
}

export default function VideosView() {
  const [categoria, setCategoria] = useState('trending');
  const [busqueda, setBusqueda] = useState('');
  const [termino, setTermino] = useState('');
  const [videos, setVideos] = useState([]);
  const [cargando, setCargando] = useState(true);
  const [videoActivo, setVideoActivo] = useState(null);
  const [likes, setLikes] = useState({});

  useEffect(() => {
    let cancelado = false;
    setCargando(true);
    setVideoActivo(null);

    async function cargarVideos() {
      try {
        let url = 'https://api.dailymotion.com/videos?fields=id,title,thumbnail_720_url,thumbnail_360_url,duration,owner.screenname,owner.username,created_time&limit=20';

        if (termino.trim()) {
          url += `&search=${encodeURIComponent(termino.trim())}`;
        } else {
          const cat = CATEGORIAS.find((c) => c.id === categoria);
          if (cat && cat.canal) {
            url += `&channel=${cat.canal}`;
          } else {
            url += '&sort=trending';
          }
        }

        const res = await fetch(url);
        if (!res.ok) throw new Error('Error al conectar con el servidor de videos');
        const data = await res.json();

        if (!cancelado) {
          setVideos(data.list || []);
          setCargando(false);
        }
      } catch (err) {
        if (!cancelado) {
          console.error('[videos] Error:', err);
          setCargando(false);
        }
      }
    }

    cargarVideos();
    return () => { cancelado = true; };
  }, [categoria, termino]);

  function buscar(e) {
    e.preventDefault();
    setTermino(busqueda.trim());
  }

  function toggleLike(id) {
    setLikes((prev) => ({ ...prev, [id]: !prev[id] }));
  }

  return (
    <div className="videos-shell">
      {/* Cabecera estilo Facebook Watch */}
      <div className="videos-cabecera">
        <div className="videos-top-row">
          <h1 className="videos-titulo">Videos</h1>
        </div>

        <form className="videos-buscador" onSubmit={buscar}>
          <IconSearch />
          <input
            type="search"
            value={busqueda}
            onChange={(e) => setBusqueda(e.target.value)}
            placeholder="Buscar en Videos..."
          />
          {busqueda ? (
            <button type="button" className="limpiar-btn" onClick={() => { setBusqueda(''); setTermino(''); }}>
              <IconX />
            </button>
          ) : null}
        </form>
      </div>

      {/* Píldoras de filtros estilo Watch */}
      <div className="videos-categorias">
        {CATEGORIAS.map((cat) => (
          <button
            key={cat.id}
            type="button"
            className={`cat-btn ${categoria === cat.id && !termino ? 'activa' : ''}`}
            onClick={() => { setTermino(''); setCategoria(cat.id); }}
          >
            {cat.label}
          </button>
        ))}
      </div>

      {/* Feed continuo de publicaciones de video */}
      <div className="videos-feed">
        {cargando && Array.from({ length: 3 }).map((_, i) => (
          <div className="watch-card-skeleton" key={i} />
        ))}

        {!cargando && videos.map((vid) => {
          const reproduciendo = videoActivo === vid.id;
          const canal = vid['owner.screenname'] || vid['owner.username'] || 'Creador';
          const inicial = canal.charAt(0).toUpperCase();
          const meGusta = Boolean(likes[vid.id]);

          return (
            <article key={vid.id} className="watch-card">
              {/* Creador / Canal */}
              <div className="watch-head">
                <div className="watch-avatar">{inicial}</div>
                <div className="watch-meta">
                  <span className="watch-canal">{canal}</span>
                  <span className="watch-fecha">Sugerido para ti</span>
                </div>
              </div>

              {/* Título de la publicación */}
              <h2 className="watch-titulo">{vid.title}</h2>

              {/* Reproductor / Portada */}
              <div className="watch-video-box">
                {reproduciendo ? (
                  <iframe
                    title={vid.title}
                    src={`https://www.dailymotion.com/embed/video/${vid.id}?autoplay=1&ui-logo=0&ui-start-screen-info=0&sharing-enable=0`}
                    allowFullScreen
                    allow="autoplay; fullscreen"
                  />
                ) : (
                  <div
                    className="watch-portada"
                    onClick={() => setVideoActivo(vid.id)}
                    role="button"
                    tabIndex={0}
                  >
                    <img
                      src={vid.thumbnail_720_url || vid.thumbnail_360_url}
                      alt={vid.title}
                      loading="lazy"
                    />
                    <span className="watch-duracion">{formatearSegundos(vid.duration)}</span>
                    <div className="watch-play-overlay">
                      <div className="watch-play-boton">▶</div>
                    </div>
                  </div>
                )}
              </div>

              {/* Botones de acción estilo red social */}
              <div className="watch-acciones">
                <button
                  type="button"
                  className={`watch-accion-btn ${meGusta ? 'activo' : ''}`}
                  onClick={() => toggleLike(vid.id)}
                  style={{ color: meGusta ? 'var(--accent)' : undefined }}
                >
                  <IconHeart /> Me gusta
                </button>
                <button
                  type="button"
                  className="watch-accion-btn"
                  onClick={() => {
                    const el = document.querySelector(`[data-vid="${vid.id}"]`);
                    setVideoActivo(vid.id);
                  }}
                >
                  <IconComment /> Comentar
                </button>
                <button
                  type="button"
                  className="watch-accion-btn"
                  onClick={() => {
                    if (navigator.share) {
                      navigator.share({ title: vid.title, url: `https://www.dailymotion.com/video/${vid.id}` }).catch(() => {});
                    } else {
                      navigator.clipboard?.writeText(`https://www.dailymotion.com/video/${vid.id}`);
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
