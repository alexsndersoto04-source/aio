// Moon Watch — Plataforma nativa de videos de Moon respaldada en Telegram
// =========================================================================
// Feed estilo Facebook Watch / Reels, 100% nativo y profesional.
// Sin terceros, sin límites de bloqueo, con reproductor en pantalla completa,
// comentarios en vivo, reacciones y subida directa de videos a tu biblioteca.

import React, { useState, useEffect, useRef } from 'react';
import { api, uploadMedia, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError } from '../ui.js';
import Avatar from '../components/Avatar.jsx';
import {
  IconSearch, IconX, IconHeart, IconComment, IconSend, IconVideo,
} from '../components/Icons.jsx';

const PESTANAS = [
  { id: 'para_ti', label: 'Para ti' },
  { id: 'tendencias', label: 'Tendencias' },
  { id: 'siguiendo', label: 'Siguiendo' },
  { id: 'mis_videos', label: 'Mis Videos' },
];

function tiempoRelativo(fechaIso) {
  if (!fechaIso) return '';
  const seg = Math.max(0, Math.floor((Date.now() - new Date(fechaIso).getTime()) / 1000));
  if (seg < 60) return 'Hace un momento';
  const min = Math.floor(seg / 60);
  if (min < 60) return `Hace ${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return `Hace ${h} h`;
  const d = Math.floor(h / 24);
  return `Hace ${d} d`;
}

function formatearSegundos(s) {
  if (isNaN(s) || s === Infinity) return '0:00';
  const min = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${min}:${sec < 10 ? '0' : ''}${sec}`;
}

/** Componente de reproducción y tarjeta de video individual */
function TarjetaVideoWatch({ post, onActualizar }) {
  const { user: yo } = useAuth();
  const videoRef = useRef(null);
  const contenedorRef = useRef(null);

  const [reproduciendo, setReproduciendo] = useState(false);
  const [silenciado, setSilenciado] = useState(false);
  const [progreso, setProgreso] = useState(0);
  const [duracion, setDuracion] = useState(0);
  const [tiempoActual, setTiempoActual] = useState(0);

  // Estados de interacción
  const [isLiked, setIsLiked] = useState(Boolean(post.is_liked));
  const [likesCount, setLikesCount] = useState(Number(post.likes_count || 0));
  const [siguiendo, setSiguiendo] = useState(Boolean(post.user?.is_following));

  // Comentarios
  const [verComentarios, setVerComentarios] = useState(false);
  const [comentarios, setComentarios] = useState([]);
  const [cargandoComentarios, setCargandoComentarios] = useState(false);
  const [nuevoComentario, setNuevoComentario] = useState('');
  const [enviandoComentario, setEnviandoComentario] = useState(false);

  // Extraer el archivo de video de la publicación
  const archivoVideo = (post.images || []).find((im) => {
    const u = im.original_url || im.url || '';
    return im.kind === 'video' || /\.(mp4|webm|mov|mkv|3gp)(\?.*)?$/i.test(u);
  });
  const urlVideo = archivoVideo ? (archivoVideo.original_url || archivoVideo.url) : '';

  // Control de play / pause
  function alternarPlay() {
    if (!videoRef.current) return;
    if (videoRef.current.paused) {
      videoRef.current.play()
        .then(() => setReproduciendo(true))
        .catch((err) => {
          console.warn('Reproduccion bloqueada por navegador, intentando silenciado:', err?.message || err);
          if (videoRef.current) {
            videoRef.current.muted = true;
            setSilenciado(true);
            videoRef.current.play().then(() => setReproduciendo(true)).catch(() => {});
          }
        });
    } else {
      videoRef.current.pause();
      setReproduciendo(false);
    }
  }

  // Pantalla completa nativa
  function activarPantallaCompleta() {
    const el = videoRef.current || contenedorRef.current;
    if (!el) return;
    if (el.requestFullscreen) {
      el.requestFullscreen().catch(() => {});
    } else if (el.webkitRequestFullscreen) {
      el.webkitRequestFullscreen();
    } else if (el.webkitEnterFullscreen) {
      // iOS Safari nativo para <video>
      el.webkitEnterFullscreen();
    }
  }

  // Alternar Me Gusta
  async function alternarLike() {
    const nuevoEstado = !isLiked;
    setIsLiked(nuevoEstado);
    setLikesCount((prev) => Math.max(0, prev + (nuevoEstado ? 1 : -1)));

    try {
      if (nuevoEstado) {
        await api.post(`/api/posts/${post.id}/like`, {});
      } else {
        await api.del(`/api/posts/${post.id}/like`);
      }
    } catch {
      // Revertir en caso de fallo
      setIsLiked(!nuevoEstado);
      setLikesCount((prev) => Math.max(0, prev + (nuevoEstado ? -1 : 1)));
    }
  }

  // Alternar Seguir al creador
  async function alternarSeguir() {
    if (!post.user?.id || post.user.id === yo?.id) return;
    const nuevo = !siguiendo;
    setSiguiendo(nuevo);
    try {
      if (nuevo) {
        await api.post(`/api/users/${post.user.id}/follow`, {});
      } else {
        await api.del(`/api/users/${post.user.id}/follow`);
      }
    } catch {
      setSiguiendo(!nuevo);
    }
  }

  // Cargar comentarios
  async function abrirComentarios() {
    const siguiente = !verComentarios;
    setVerComentarios(siguiente);
    if (siguiente && comentarios.length === 0) {
      setCargandoComentarios(true);
      try {
        const res = await api.get(`/api/posts/${post.id}/comments`);
        setComentarios(res || []);
      } catch (e) {
        avisoError(e);
      } finally {
        setCargandoComentarios(false);
      }
    }
  }

  // Enviar comentario
  async function enviarComentario(e) {
    e.preventDefault();
    const txt = nuevoComentario.trim();
    if (!txt || enviandoComentario) return;
    setEnviandoComentario(true);
    try {
      const creado = await api.post(`/api/posts/${post.id}/comments`, { content: txt });
      setComentarios((prev) => [...prev, creado]);
      setNuevoComentario('');
      if (onActualizar) onActualizar();
    } catch (err) {
      avisoError(err);
    } finally {
      setEnviandoComentario(false);
    }
  }

  if (!urlVideo) return null;

  return (
    <article className="watch-fb-card" ref={contenedorRef}>
      {/* 1. Cabecera del creador */}
      <div className="watch-card-header">
        <div className="watch-creator-info">
          <Avatar user={post.user} size="sm" />
          <div className="watch-creator-texts">
            <span className="watch-creator-name">
              {post.user?.display_name || post.user?.username || 'Creador Moon'}
              {post.user?.verified ? ' ✓' : ''}
            </span>
            <span className="watch-post-date">
              @{post.user?.username || 'usuario'} · {tiempoRelativo(post.created_at)}
            </span>
          </div>
        </div>

        {yo?.id !== post.user?.id ? (
          <button
            type="button"
            className={`watch-follow-btn ${siguiendo ? 'siguiendo' : ''}`}
            onClick={alternarSeguir}
          >
            {siguiendo ? 'Siguiendo' : '+ Seguir'}
          </button>
        ) : null}
      </div>

      {/* 2. Título o descripción del video */}
      {post.content ? (
        <p className="watch-card-text">{post.content}</p>
      ) : null}

      {/* 3. Área de reproducción de video profesional */}
      <div className="watch-media-box">
        <video
          ref={videoRef}
          src={imgUrl(urlVideo)}
          playsInline
          crossOrigin="anonymous"
          preload="metadata"
          muted={silenciado}
          onClick={alternarPlay}
          onTimeUpdate={() => {
            if (videoRef.current) {
              const cur = videoRef.current.currentTime;
              const dur = videoRef.current.duration || 1;
              setTiempoActual(cur);
              setProgreso((cur / dur) * 100);
            }
          }}
          onLoadedMetadata={() => {
            if (videoRef.current) setDuracion(videoRef.current.duration || 0);
          }}
          onPlay={() => setReproduciendo(true)}
          onPause={() => setReproduciendo(false)}
          onEnded={() => setReproduciendo(false)}
        />

        {/* Botón flotante central de Play cuando está pausado */}
        {!reproduciendo ? (
          <div className="watch-play-button-overlay" onClick={alternarPlay}>
            <div className="watch-play-circle-icon">▶</div>
          </div>
        ) : null}

        {/* Barra de control inferior nativa */}
        <div className="watch-video-controls-bar">
          <button type="button" className="watch-ctrl-btn" onClick={alternarPlay} title="Play/Pausa">
            {reproduciendo ? '❚❚' : '▶'}
          </button>

          <span className="watch-time-label">
            {formatearSegundos(tiempoActual)} / {formatearSegundos(duracion)}
          </span>

          {/* Barra de avance deslizante */}
          <input
            type="range"
            min="0"
            max="100"
            value={progreso}
            className="watch-scrubber"
            onChange={(e) => {
              const pct = Number(e.target.value);
              setProgreso(pct);
              if (videoRef.current && duracion > 0) {
                videoRef.current.currentTime = (pct / 100) * duracion;
              }
            }}
          />

          <button
            type="button"
            className="watch-ctrl-btn"
            onClick={() => setSilenciado(!silenciado)}
            title={silenciado ? 'Activar sonido' : 'Silenciar'}
          >
            {silenciado ? '🔇' : '🔊'}
          </button>

          <button
            type="button"
            className="watch-ctrl-btn"
            onClick={activarPantallaCompleta}
            title="Pantalla completa"
          >
            ⛶
          </button>
        </div>
      </div>

      {/* 4. Estadísticas del video */}
      <div className="watch-stats-row">
        <span>{likesCount} {likesCount === 1 ? 'me gusta' : 'me gusta'}</span>
        <span>{post.comments_count || comentarios.length || 0} comentarios</span>
      </div>

      {/* 5. Barra de interacciones estilo Facebook Watch */}
      <div className="watch-fb-actions-bar">
        <button
          type="button"
          className={`watch-fb-action-button ${isLiked ? 'liked' : ''}`}
          onClick={alternarLike}
        >
          <IconHeart filled={isLiked} /> {isLiked ? 'Me gusta' : 'Me gusta'}
        </button>

        <button
          type="button"
          className="watch-fb-action-button"
          onClick={abrirComentarios}
        >
          <IconComment /> Comentar
        </button>

        <button
          type="button"
          className="watch-fb-action-button"
          onClick={() => {
            const shareUrl = window.location.origin + `/p/${post.id}`;
            if (navigator.share) {
              navigator.share({ title: post.content || 'Video en Moon Watch', url: shareUrl }).catch(() => {});
            } else {
              navigator.clipboard?.writeText(shareUrl);
              toast.ok('Enlace del video copiado al portapapeles');
            }
          }}
        >
          <IconSend /> Compartir
        </button>
      </div>

      {/* 6. Sección de comentarios desplegable */}
      {verComentarios ? (
        <div className="watch-comments-section">
          <form className="watch-comment-input-box" onSubmit={enviarComentario}>
            <input
              type="text"
              placeholder="Escribe un comentario..."
              value={nuevoComentario}
              onChange={(e) => setNuevoComentario(e.target.value)}
            />
            <button type="submit" disabled={!nuevoComentario.trim() || enviandoComentario}>
              Enviar
            </button>
          </form>

          {cargandoComentarios ? (
            <div style={{ padding: '12px', textAlign: 'center', color: '#65676b', fontSize: 13 }}>
              Cargando comentarios…
            </div>
          ) : null}

          {comentarios.map((c) => (
            <div className="watch-comment-item" key={c.id}>
              <div className="watch-comment-bubble">
                <b>{c.user?.display_name || c.user?.username || 'Usuario'}</b>
                <p>{c.content}</p>
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </article>
  );
}

/** Pantalla principal de Moon Watch */
export default function VideosView() {
  const [categoria, setCategoria] = useState('para_ti');
  const [busqueda, setBusqueda] = useState('');
  const [queryInput, setQueryInput] = useState('');
  const [videos, setVideos] = useState([]);
  const [cargando, setCargando] = useState(true);

  // Modal de subida de video
  const [mostrarModal, setMostrarModal] = useState(false);
  const [archivoSeleccionado, setArchivoSeleccionado] = useState(null);
  const [previewUrl, setPreviewUrl] = useState('');
  const [descripcionVideo, setDescripcionVideo] = useState('');
  const [subiendoPct, setSubiendoPct] = useState(0);
  const [publicando, setPublicando] = useState(false);
  const inputArchivoRef = useRef(null);

  // Cargar videos de Moon
  useEffect(() => {
    let activo = true;
    setCargando(true);

    const params = new URLSearchParams();
    params.set('cat', categoria);
    if (busqueda) params.set('q', busqueda);
    params.set('limit', '25');

    api.get(`/api/videos?${params.toString()}`)
      .then((res) => {
        if (!activo) return;
        setVideos(res?.items || res || []);
      })
      .catch((err) => {
        console.error('[watch] error cargando videos:', err);
      })
      .finally(() => {
        if (activo) setCargando(false);
      });

    return () => { activo = false; };
  }, [categoria, busqueda]);

  function buscar(e) {
    e.preventDefault();
    setBusqueda(queryInput.trim());
  }

  // Manejar selección de video
  function onSeleccionarArchivo(e) {
    const f = e.target.files && e.target.files[0];
    if (!f) return;
    if (!f.type.startsWith('video/') && !/\.(mp4|webm|mov|mkv|3gp)$/i.test(f.name)) {
      toast.err('Selecciona un archivo de video válido (.mp4, .webm, .mov)');
      return;
    }
    if (f.size > 120 * 1024 * 1024) {
      toast.err('El video no puede superar los 120 MB');
      return;
    }

    setArchivoSeleccionado(f);
    setPreviewUrl(URL.createObjectURL(f));
  }

  // Publicar video directo a Moon Watch
  async function publicarVideo(e) {
    e.preventDefault();
    if (!archivoSeleccionado || publicando) return;

    setPublicando(true);
    setSubiendoPct(1);

    try {
      // 1. Subir a Moon (respaldado en Telegram)
      const resMedia = await uploadMedia('video', archivoSeleccionado, (pct) => {
        setSubiendoPct(pct);
      });

      // 2. Crear la publicación
      const postCreado = await api.post('/api/posts', {
        content: descripcionVideo.trim(),
        images: [{ id: resMedia.id, url: resMedia.url }],
      });

      toast.ok('¡Video publicado exitosamente en Moon Watch!');
      setVideos((prev) => [postCreado, ...prev]);

      // Limpiar y cerrar modal
      setMostrarModal(false);
      setArchivoSeleccionado(null);
      setPreviewUrl('');
      setDescripcionVideo('');
    } catch (err) {
      avisoError(err);
    } finally {
      setPublicando(false);
      setSubiendoPct(0);
    }
  }

  return (
    <div className="videos-shell">
      {/* 1. Cabecera principal estilo Facebook Watch */}
      <div className="watch-fb-header">
        <div className="watch-fb-title-bar">
          <div className="watch-title-brand">
            <h1 className="watch-fb-title">Moon Watch</h1>
          </div>

          <button
            type="button"
            className="watch-subir-btn-hero"
            onClick={() => setMostrarModal(true)}
          >
            <IconVideo /> Subir video
          </button>
        </div>

        {/* Buscador plano */}
        <form className="watch-fb-searchbox" onSubmit={buscar}>
          <IconSearch />
          <input
            type="search"
            value={queryInput}
            onChange={(e) => setQueryInput(e.target.value)}
            placeholder="Buscar videos en Moon Watch..."
          />
          {queryInput ? (
            <button
              type="button"
              className="clear-btn"
              onClick={() => { setQueryInput(''); setBusqueda(''); }}
            >
              <IconX />
            </button>
          ) : null}
        </form>
      </div>

      {/* 2. Pestañas de categorías */}
      <div className="watch-fb-tabs">
        {PESTANAS.map((pestana) => (
          <button
            key={pestana.id}
            type="button"
            className={`watch-fb-pill ${categoria === pestana.id && !busqueda ? 'active' : ''}`}
            onClick={() => {
              setBusqueda('');
              setQueryInput('');
              setCategoria(pestana.id);
            }}
          >
            {pestana.label}
          </button>
        ))}
      </div>

      {/* 3. Feed de videos */}
      <div className="watch-fb-feed">
        {cargando && Array.from({ length: 2 }).map((_, i) => (
          <div className="watch-skeleton-card" key={i} />
        ))}

        {!cargando && videos.length === 0 && (
          <div className="watch-empty-box">
            <div className="watch-empty-icon">📹</div>
            <h3>Tu biblioteca de videos en Moon Watch</h3>
            <p>
              Todos los videos que tú y la comunidad suban aquí quedan guardados de por vida
              en tu canal de Telegram, con reproducción fluida, sin límites ni cortes.
            </p>
            <button
              type="button"
              className="btn btn-aurora"
              onClick={() => setMostrarModal(true)}
            >
              + Subir el primer video
            </button>
          </div>
        )}

        {!cargando && videos.map((v) => (
          <TarjetaVideoWatch
            key={v.id}
            post={v}
            onActualizar={() => {}}
          />
        ))}
      </div>

      {/* 4. Modal de Subida de Video */}
      {mostrarModal ? (
        <div className="watch-modal-backdrop" onClick={() => !publicando && setMostrarModal(false)}>
          <div className="watch-modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="watch-modal-header">
              <h2>Subir video a Moon Watch</h2>
              <button
                type="button"
                className="close-btn"
                disabled={publicando}
                onClick={() => setMostrarModal(false)}
              >
                <IconX />
              </button>
            </div>

            <form onSubmit={publicarVideo}>
              {/* Selector o Vista previa del video */}
              {!previewUrl ? (
                <div
                  className="watch-dropzone"
                  onClick={() => inputArchivoRef.current && inputArchivoRef.current.click()}
                >
                  <input
                    ref={inputArchivoRef}
                    type="file"
                    accept="video/mp4,video/webm,video/quicktime,video/3gpp"
                    hidden
                    onChange={onSeleccionarArchivo}
                  />
                  <div className="dropzone-icon">📁</div>
                  <p><b>Toca para elegir un video desde tu galería</b></p>
                  <span>Formatos: MP4, WebM, MOV (hasta 120 MB)</span>
                </div>
              ) : (
                <div className="watch-preview-container">
                  <video src={previewUrl} controls playsInline className="preview-vid" />
                  <button
                    type="button"
                    className="btn-cambiar-video"
                    onClick={() => { setArchivoSeleccionado(null); setPreviewUrl(''); }}
                  >
                    Cambiar video
                  </button>
                </div>
              )}

              {/* Descripción */}
              <div className="watch-form-group">
                <textarea
                  className="watch-textarea"
                  rows={3}
                  placeholder="Escribe una descripción o título para tu video..."
                  value={descripcionVideo}
                  onChange={(e) => setDescripcionVideo(e.target.value)}
                  disabled={publicando}
                />
              </div>

              {/* Barra de progreso si está subiendo */}
              {publicando ? (
                <div className="watch-upload-progress">
                  <div className="progress-info">
                    <span>Subiendo a tu almacén de Telegram…</span>
                    <span>{subiendoPct}%</span>
                  </div>
                  <div className="progress-track">
                    <div className="progress-bar" style={{ width: `${subiendoPct}%` }} />
                  </div>
                </div>
              ) : null}

              {/* Botón de publicar */}
              <div className="watch-modal-actions">
                <button
                  type="button"
                  className="btn btn-ghost"
                  disabled={publicando}
                  onClick={() => setMostrarModal(false)}
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  className="btn btn-aurora"
                  disabled={!archivoSeleccionado || publicando}
                >
                  {publicando ? `Publicando (${subiendoPct}%)…` : 'Publicar Video'}
                </button>
              </div>
            </form>
          </div>
        </div>
      ) : null}
    </div>
  );
}
