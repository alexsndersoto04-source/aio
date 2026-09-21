// Moon — Feed de Videos y Entretenimiento
// ============================================================
// Diseño integrado nativo en Moon (claro, elegante y limpio):
// - Sin modales emergentes raros ni franjas negras desproporcionadas.
// - Formato de feed de publicaciones de video profesionales.
// - Reproducción directa integrada con proporciones exactas.

import React, { useState, useEffect } from 'react';
import { IconSearch, IconX } from '../components/Icons.jsx';

const CATEGORIAS = [
  { id: 'trending', label: 'Tendencias' },
  { id: 'music', label: 'Música', canal: 'music' },
  { id: 'gaming', label: 'Videojuegos', canal: 'videogames' },
  { id: 'fun', label: 'Humor y Comedia', canal: 'fun' },
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
  const [error, setError] = useState('');
  // Cuál video está reproduciéndose activamente en el feed
  const [videoReproduciendo, setVideoReproduciendo] = useState(null);

  useEffect(() => {
    let cancelado = false;
    setCargando(true);
    setError('');
    setVideoReproduciendo(null);

    async function cargarVideos() {
      try {
        let url = 'https://api.dailymotion.com/videos?fields=id,title,thumbnail_720_url,thumbnail_360_url,duration,owner.screenname,owner.username,views_total,created_time&limit=20';

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
        if (!res.ok) throw new Error('Error al conectar con la red de videos');
        const data = await res.json();

        if (!cancelado) {
          setVideos(data.list || []);
          setCargando(false);
        }
      } catch (err) {
        if (!cancelado) {
          console.error('[videos] Error:', err);
          setError('No pudimos cargar los videos. Revisa tu conexión.');
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

  function limpiar() {
    setBusqueda('');
    setTermino('');
  }

  return (
    <div className="videos-shell">
      {/* Cabecera integrada */}
      <div className="videos-cabecera">
        <h1 className="videos-titulo">Videos</h1>
        <p className="videos-sub">Tendencias, música y entretenimiento en Moon.</p>

        <form className="videos-buscador" onSubmit={buscar}>
          <IconSearch />
          <input
            type="search"
            value={busqueda}
            onChange={(e) => setBusqueda(e.target.value)}
            placeholder="Buscar videos, artistas, temas..."
          />
          {busqueda ? (
            <button type="button" className="limpiar-btn" onClick={limpiar}>
              <IconX />
            </button>
          ) : null}
        </form>
      </div>

      {/* Categorías */}
      {!termino && (
        <div className="videos-categorias">
          {CATEGORIAS.map((cat) => (
            <button
              key={cat.id}
              type="button"
              className={`cat-btn ${categoria === cat.id ? 'activa' : ''}`}
              onClick={() => setCategoria(cat.id)}
            >
              {cat.label}
            </button>
          ))}
        </div>
      )}

      {termino && (
        <div className="videos-aviso-busqueda">
          <span>Resultados para: <b>"{termino}"</b></span>
          <button type="button" onClick={limpiar}>Volver a categorías</button>
        </div>
      )}

      {/* Cargando */}
      {cargando && (
        <div className="videos-feed">
          {Array.from({ length: 4 }).map((_, i) => (
            <div className="video-post-card-skeleton" key={i} />
          ))}
        </div>
      )}

      {/* Error */}
      {error && !cargando && (
        <div className="videos-aviso-busqueda">
          <span>{error}</span>
          <button type="button" onClick={() => setCategoria(categoria)}>Reintentar</button>
        </div>
      )}

      {/* Feed nativo de videos */}
      {!cargando && !error && (
        <div className="videos-feed">
          {videos.map((vid) => {
            const estaReproduciendo = videoReproduciendo === vid.id;
            const canal = vid['owner.screenname'] || vid['owner.username'] || 'Creador';
            const inicial = canal.charAt(0).toUpperCase();

            return (
              <article key={vid.id} className="video-post-card">
                {/* Cabecera del video */}
                <div className="video-post-head">
                  <div className="video-post-avatar">{inicial}</div>
                  <div className="video-post-meta">
                    <span className="video-post-autor">{canal}</span>
                    <span className="video-post-tag">En Moon Video</span>
                  </div>
                </div>

                {/* Reproductor o Portada interactiva */}
                <div className="video-frame-container">
                  {estaReproduciendo ? (
                    <iframe
                      title={vid.title}
                      src={`https://www.dailymotion.com/embed/video/${vid.id}?autoplay=1&ui-logo=0&ui-start-screen-info=0`}
                      allowFullScreen
                      allow="autoplay; fullscreen"
                    />
                  ) : (
                    <div
                      className="video-placeholder-wrap"
                      onClick={() => setVideoReproduciendo(vid.id)}
                      role="button"
                      tabIndex={0}
                    >
                      <img
                        src={vid.thumbnail_720_url || vid.thumbnail_360_url}
                        alt={vid.title}
                        loading="lazy"
                      />
                      <span className="video-duracion-badge">{formatearSegundos(vid.duration)}</span>
                      <div className="video-play-btn-pulse">
                        <div className="play-circle">▶</div>
                      </div>
                    </div>
                  )}
                </div>

                {/* Título y detalles al pie */}
                <div className="video-post-foot">
                  <h2 className="video-post-titulo">{vid.title}</h2>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}
