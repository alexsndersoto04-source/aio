// Moon — Sección de Videos y Entretenimiento
// ============================================================
// Diseño claro, limpio y profesional al estilo de Moon:
// - Fondo claro descansado con tarjetas blancas y acentos morados.
// - Categorías de contenido (Tendencias, Música, Videojuegos, Noticias, Deportes).
// - Buscador de videos instantáneo.
// - Reproductor integrado en modal elegante sin salir de Moon.
// - Consume el catálogo público mundial de Dailymotion sin costos.

import React, { useState, useEffect, useRef } from 'react';
import {
  IconSearch, IconX, IconRefresh, IconSpark, IconPlay,
} from '../components/Icons.jsx';

const CATEGORIAS = [
  { id: 'trending', label: 'Tendencias' },
  { id: 'music', label: 'Música', canal: 'music' },
  { id: 'gaming', label: 'Videojuegos', canal: 'videogames' },
  { id: 'news', label: 'Noticias', canal: 'news' },
  { id: 'sport', label: 'Deportes', canal: 'sport' },
  { id: 'fun', label: 'Comedia', canal: 'fun' },
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
  const [videoActivo, setVideoActivo] = useState(null);

  useEffect(() => {
    let cancelado = false;
    setCargando(true);
    setError('');

    async function cargarVideos() {
      try {
        let url = 'https://api.dailymotion.com/videos?fields=id,title,thumbnail_360_url,duration,owner.screenname,views_total,created_time&limit=24';

        if (termino.trim()) {
          url += `&search=${encodeURIComponent(termino.trim())}`;
        } else {
          const catActual = CATEGORIAS.find((c) => c.id === categoria);
          if (catActual && catActual.canal) {
            url += `&channel=${catActual.canal}`;
          } else {
            url += '&sort=trending';
          }
        }

        const res = await fetch(url);
        if (!res.ok) throw new Error('No se pudieron cargar los videos');
        const data = await res.json();

        if (!cancelado) {
          setVideos(data.list || []);
          setCargando(false);
        }
      } catch (err) {
        if (!cancelado) {
          console.error('[videos] Error cargando:', err);
          setError('No pudimos conectar con el catálogo de videos. Comprueba tu conexión.');
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

  function limpiarBusqueda() {
    setBusqueda('');
    setTermino('');
  }

  return (
    <div className="videos-shell">
      {/* Cabecera limpia y clara */}
      <div className="videos-cabecera">
        <div className="videos-titulo-area">
          <h1 className="videos-titulo">Videos</h1>
          <p className="videos-sub">Descubre lo más nuevo en música, tendencias y entretenimiento.</p>
        </div>

        {/* Buscador de videos */}
        <form className="videos-buscador" onSubmit={buscar}>
          <IconSearch />
          <input
            type="search"
            value={busqueda}
            onChange={(e) => setBusqueda(e.target.value)}
            placeholder="Buscar videos, canciones, creadores..."
          />
          {busqueda ? (
            <button type="button" className="limpiar-btn" onClick={limpiarBusqueda}>
              <IconX />
            </button>
          ) : null}
        </form>
      </div>

      {/* Pestañas de categorías */}
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
          <button type="button" onClick={limpiarBusqueda}>Ver categorías</button>
        </div>
      )}

      {/* Estado de carga */}
      {cargando && (
        <div className="videos-grid-esqueleto">
          {Array.from({ length: 8 }).map((_, i) => (
            <div className="video-card-skeleton" key={i} />
          ))}
        </div>
      )}

      {/* Error */}
      {error && !cargando && (
        <div className="videos-error">
          <p>{error}</p>
          <button type="button" onClick={() => setCategoria(categoria)}>Reintentar</button>
        </div>
      )}

      {/* Cuadrícula de videos */}
      {!cargando && !error && (
        <div className="videos-grid">
          {videos.map((vid) => (
            <div
              key={vid.id}
              className="video-card"
              onClick={() => setVideoActivo(vid)}
              role="button"
              tabIndex={0}
            >
              <div className="video-thumb-wrap">
                <img
                  src={vid.thumbnail_360_url || 'https://via.placeholder.com/360x200?text=Video'}
                  alt={vid.title}
                  loading="lazy"
                />
                <span className="video-duracion">{formatearSegundos(vid.duration)}</span>
                <div className="video-play-overlay">
                  <span className="play-icon">▶</span>
                </div>
              </div>
              <div className="video-info">
                <h3 className="video-title" title={vid.title}>{vid.title}</h3>
                <div className="video-meta">
                  <span className="video-canal">{vid['owner.screenname'] || 'Canal'}</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Modal Reproductor de Video */}
      {videoActivo && (
        <div className="video-modal-velo" onClick={() => setVideoActivo(null)}>
          <div className="video-modal-caja" onClick={(e) => e.stopPropagation()}>
            <button
              type="button"
              className="video-cerrar-btn"
              onClick={() => setVideoActivo(null)}
              aria-label="Cerrar video"
            >
              ✕
            </button>
            <div className="video-reproductor-iframe-wrap">
              <iframe
                title={videoActivo.title}
                src={`https://www.dailymotion.com/embed/video/${videoActivo.id}?autoplay=1`}
                width="100%"
                height="100%"
                allowFullScreen
                allow="autoplay; fullscreen"
                frameBorder="0"
              />
            </div>
            <div className="video-modal-detalles">
              <h2>{videoActivo.title}</h2>
              <p>Por: <b>{videoActivo['owner.screenname'] || 'Creador'}</b></p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
