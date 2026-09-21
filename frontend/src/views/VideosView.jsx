// Moon Watch — Plataforma nativa de videos de Moon respaldada en Telegram
// =========================================================================
// Feed estilo Facebook Watch / Reels, 100% nativo, sobrio y estético.
// Sin lupas ni banners invasivos: los videos son los protagonistas absolutos.
// Con menú de opciones por video, panel de ajustes avanzados y subida a Telegram.

import React, { useState, useEffect, useRef } from 'react';
import { api, uploadMedia, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError, confirmar } from '../ui.js';
import Avatar from '../components/Avatar.jsx';
import {
  IconX, IconHeart, IconComment, IconSend, IconVideo,
  IconSettings, IconMore, IconTrash, IconBookmark, IconCopy, IconCheck,
} from '../components/Icons.jsx';

const PESTANAS = [
  { id: 'para_ti', label: 'Para ti' },
  { id: 'tendencias', label: 'Tendencias' },
  { id: 'siguiendo', label: 'Siguiendo' },
  { id: 'mis_videos', label: 'Mis Videos' },
];

const CONFIG_POR_DEFECTO = {
  autoplay: true,
  sonidoInicial: false, // Silenciado por defecto para no asustar en público
  loopInfinito: true,
  pausarFueraPantalla: true,
  ahorroDatos: false,
  dobleTapPantallaCompleta: true,
  calidad: 'alta', // 'alta' | 'ahorro'
};

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

/** Componente de reproducción y tarjeta de video individual con menú de opciones */
function TarjetaVideoWatch({ post, config, onEliminar, onActualizar }) {
  const { user: yo } = useAuth();
  const videoRef = useRef(null);
  const contenedorRef = useRef(null);
  const menuRef = useRef(null);

  const [reproduciendo, setReproduciendo] = useState(false);
  const [silenciado, setSilenciado] = useState(!config?.sonidoInicial);
  const [progreso, setProgreso] = useState(0);
  const [duracion, setDuracion] = useState(0);
  const [tiempoActual, setTiempoActual] = useState(0);

  // Menú de opciones de la tarjeta
  const [mostrarMenu, setMostrarMenu] = useState(false);

  // Estados de interacción
  const [isLiked, setIsLiked] = useState(Boolean(post.is_liked));
  const [likesCount, setLikesCount] = useState(Number(post.likes_count || 0));
  const [isSaved, setIsSaved] = useState(Boolean(post.is_saved));
  const autorId = post.user?.id || post.user_id || post.author_id;
  const autorUsername = post.author_username || post.user?.username || 'usuario';
  const autorNombre = post.author_display_name || post.user?.display_name || autorUsername;
  const autorAvatar = post.author_avatar_url || post.user?.avatar_url || '';
  const autorVerificado = Boolean(post.author_is_verified || post.user?.is_verified || post.user?.verified);
  const esMio = Boolean(yo?.id && autorId && Number(yo.id) === Number(autorId));
  const [siguiendo, setSiguiendo] = useState(Boolean(post.user?.is_following));

  // Comentarios
  const [verComentarios, setVerComentarios] = useState(false);
  const [comentarios, setComentarios] = useState([]);
  const [cargandoComentarios, setCargandoComentarios] = useState(false);
  const [nuevoComentario, setNuevoComentario] = useState('');
  const [enviandoComentario, setEnviandoComentario] = useState(false);

  // Cerrar menú al hacer clic fuera
  useEffect(() => {
    function handleClickFuera(e) {
      if (menuRef.current && !menuRef.current.contains(e.target)) {
        setMostrarMenu(false);
      }
    }
    if (mostrarMenu) {
      document.addEventListener('mousedown', handleClickFuera);
      document.addEventListener('touchstart', handleClickFuera);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickFuera);
      document.removeEventListener('touchstart', handleClickFuera);
    };
  }, [mostrarMenu]);

  // Autoplay inteligente y pausa cuando sale de pantalla (IntersectionObserver)
  useEffect(() => {
    if (!config?.autoplay || !contenedorRef.current) return;
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (!videoRef.current) return;
          if (entry.isIntersecting && entry.intersectionRatio >= 0.6) {
            // Video en centro de pantalla -> reproducir si autoplay activo
            videoRef.current.play().then(() => setReproduciendo(true)).catch(() => {
              if (videoRef.current) {
                videoRef.current.muted = true;
                setSilenciado(true);
                videoRef.current.play().then(() => setReproduciendo(true)).catch(() => {});
              }
            });
          } else if (config.pausarFueraPantalla && !entry.isIntersecting) {
            // Fuera de pantalla -> pausar
            videoRef.current.pause();
            setReproduciendo(false);
          }
        });
      },
      { threshold: [0, 0.6] }
    );

    observer.observe(contenedorRef.current);
    return () => observer.disconnect();
  }, [config?.autoplay, config?.pausarFueraPantalla]);

  // Extraer el archivo de video de la publicación
  const archivoVideo = (post.images || []).find((im) => {
    const u = (im.original_url || im.url || im.thumb_url || '').toLowerCase();
    return im.kind === 'video' || /\.(mp4|webm|mov|mkv|3gp|ogv)(\?.*)?$/i.test(u) || u.includes('video');
  });
  const urlVideo = archivoVideo ? (archivoVideo.original_url || archivoVideo.url || archivoVideo.thumb_url) : '';

  // Control de play / pause
  function alternarPlay() {
    if (!videoRef.current) return;
    if (videoRef.current.paused) {
      videoRef.current.play()
        .then(() => setReproduciendo(true))
        .catch(() => {
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

  // Doble toque para pantalla completa
  const ultimoTap = useRef(0);
  function manejarTapVideo() {
    const ahora = Date.now();
    if (config?.dobleTapPantallaCompleta && ahora - ultimoTap.current < 300) {
      activarPantallaCompleta();
    } else {
      alternarPlay();
    }
    ultimoTap.current = ahora;
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
      setIsLiked(!nuevoEstado);
      setLikesCount((prev) => Math.max(0, prev + (nuevoEstado ? -1 : 1)));
    }
  }

  // Alternar Guardar en colección
  async function alternarGuardar() {
    const nuevo = !isSaved;
    setIsSaved(nuevo);
    setMostrarMenu(false);
    try {
      if (nuevo) {
        await api.post(`/api/posts/${post.id}/save`, {});
        toast.ok('Video guardado en tu colección');
      } else {
        await api.del(`/api/posts/${post.id}/save`);
        toast.info('Video removido de guardados');
      }
    } catch {
      setIsSaved(!nuevo);
    }
  }

  // Copiar enlace directo del video
  function copiarEnlace() {
    setMostrarMenu(false);
    const shareUrl = `${window.location.origin}/#/post/${post.id}`;
    if (navigator.clipboard) {
      navigator.clipboard.writeText(shareUrl)
        .then(() => toast.ok('Enlace del video copiado al portapapeles'))
        .catch(() => toast.ok(shareUrl));
    } else {
      toast.ok('Enlace: ' + shareUrl);
    }
  }

  // Eliminar video (si es del propio usuario)
  async function borrarVideo() {
    setMostrarMenu(false);
    const seguro = await confirmar({
      title: '¿Eliminar este video?',
      message: 'Esta publicación y su video se eliminarán definitivamente de Moon Watch.',
      confirmText: 'Eliminar video',
      danger: true,
    });
    if (!seguro) return;

    try {
      await api.del(`/api/posts/${post.id}`);
      toast.ok('Video eliminado correctamente');
      if (onEliminar) onEliminar(post.id);
    } catch (e) {
      avisoError(e);
    }
  }

  // Alternar Seguir al creador
  async function alternarSeguir() {
    if (!autorId || autorId === yo?.id) return;
    const nuevo = !siguiendo;
    setSiguiendo(nuevo);
    try {
      if (nuevo) {
        await api.post(`/api/users/${autorId}/follow`, {});
      } else {
        await api.del(`/api/users/${autorId}/follow`);
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
      {/* 1. Cabecera del creador con menú contextual */}
      <div className="watch-card-header">
        <a href={`#/user/${autorUsername}`} className="watch-creator-info" style={{ textDecoration: 'none', color: 'inherit' }}>
          <Avatar
            user={{
              username: autorUsername,
              display_name: autorNombre,
              avatar_url: autorAvatar,
              is_verified: autorVerificado,
            }}
            size="sm"
          />
          <div className="watch-creator-texts">
            <span className="watch-creator-name">
              {autorNombre}
              {autorVerificado ? ' ✓' : ''}
            </span>
            <span className="watch-post-date">
              @{autorUsername} · {tiempoRelativo(post.created_at)}
            </span>
          </div>
        </a>

        <div className="watch-card-header-actions" ref={menuRef}>
          {yo?.id && autorId && yo.id !== autorId ? (
            <button
              type="button"
              className={`watch-follow-btn ${siguiendo ? 'siguiendo' : ''}`}
              onClick={alternarSeguir}
            >
              {siguiendo ? 'Siguiendo' : '+ Seguir'}
            </button>
          ) : null}

          <button
            type="button"
            className="watch-card-menu-btn"
            onClick={() => setMostrarMenu((v) => !v)}
            title="Opciones de este video"
            aria-label="Opciones de este video"
          >
            <IconMore />
          </button>

          {/* Menú de opciones */}
          {mostrarMenu ? (
            <div className="watch-card-dropdown" role="menu">
              <button type="button" className="watch-dropdown-item" onClick={copiarEnlace}>
                <IconCopy /> Copiar enlace
              </button>
              <button type="button" className="watch-dropdown-item" onClick={alternarGuardar}>
                <IconBookmark filled={isSaved} /> {isSaved ? 'Quitar de guardados' : 'Guardar video'}
              </button>
              {esMio ? (
                <button type="button" className="watch-dropdown-item danger" onClick={borrarVideo}>
                  <IconTrash /> Eliminar video
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>

      {/* 2. Título o descripción del video */}
      {post.content ? (
        <p className="watch-card-text">{post.content}</p>
      ) : null}

      {/* 3. Área de reproducción cinemática sin elementos invasivos */}
      <div className="watch-media-box">
        <video
          ref={videoRef}
          src={imgUrl(urlVideo)}
          playsInline
          loop={Boolean(config?.loopInfinito)}
          preload={config?.ahorroDatos ? 'none' : 'metadata'}
          muted={silenciado}
          onClick={manejarTapVideo}
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

        {/* Botón play central si está pausado */}
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

      {/* 5. Barra de interacciones */}
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
          className={`watch-fb-action-button ${isSaved ? 'saved' : ''}`}
          onClick={alternarGuardar}
        >
          <IconBookmark filled={isSaved} /> {isSaved ? 'Guardado' : 'Guardar'}
        </button>

        <button
          type="button"
          className="watch-fb-action-button"
          onClick={() => {
            const shareUrl = `${window.location.origin}/#/post/${post.id}`;
            if (navigator.share) {
              navigator.share({ title: post.content || 'Video en Moon Watch', url: shareUrl }).catch(() => {});
            } else {
              copiarEnlace();
            }
          }}
        >
          <IconSend /> Compartir
        </button>
      </div>

      {/* 6. Sección de comentarios */}
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

/** Modal de Ajustes y Opciones Avanzadas de Moon Watch */
function AjustesAvanzadosVideosModal({ config, onGuardar, onCerrar }) {
  const [autoplay, setAutoplay] = useState(Boolean(config.autoplay));
  const [sonidoInicial, setSonidoInicial] = useState(Boolean(config.sonidoInicial));
  const [loopInfinito, setLoopInfinito] = useState(Boolean(config.loopInfinito));
  const [pausarFueraPantalla, setPausarFueraPantalla] = useState(Boolean(config.pausarFueraPantalla));
  const [ahorroDatos, setAhorroDatos] = useState(Boolean(config.ahorroDatos));
  const [dobleTapPantallaCompleta, setDobleTapPantallaCompleta] = useState(Boolean(config.dobleTapPantallaCompleta));
  const [calidad, setCalidad] = useState(config.calidad || 'alta');

  function guardar() {
    onGuardar({
      autoplay,
      sonidoInicial,
      loopInfinito,
      pausarFueraPantalla,
      ahorroDatos,
      dobleTapPantallaCompleta,
      calidad,
    });
    toast.ok('Ajustes de videos guardados');
    onCerrar();
  }

  function restaurar() {
    setAutoplay(CONFIG_POR_DEFECTO.autoplay);
    setSonidoInicial(CONFIG_POR_DEFECTO.sonidoInicial);
    setLoopInfinito(CONFIG_POR_DEFECTO.loopInfinito);
    setPausarFueraPantalla(CONFIG_POR_DEFECTO.pausarFueraPantalla);
    setAhorroDatos(CONFIG_POR_DEFECTO.ahorroDatos);
    setDobleTapPantallaCompleta(CONFIG_POR_DEFECTO.dobleTapPantallaCompleta);
    setCalidad(CONFIG_POR_DEFECTO.calidad);
    toast.info('Valores restablecidos por defecto');
  }

  return (
    <div className="watch-modal-backdrop" onClick={onCerrar}>
      <div className="watch-modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="watch-modal-header">
          <h2>Opciones y Ajustes de Videos</h2>
          <button type="button" className="close-btn" onClick={onCerrar} title="Cerrar">
            <IconX />
          </button>
        </div>

        <div className="watch-settings-scroll">
          {/* SECCIÓN 1: REPRODUCCIÓN */}
          <div className="watch-settings-group-header">Reproducción inteligente</div>

          <div className="watch-setting-row">
            <div className="watch-setting-info">
              <span className="watch-setting-title">Reproducción automática</span>
              <span className="watch-setting-desc">
                Reproducir automáticamente los videos al hacer scroll.
              </span>
            </div>
            <label className="watch-toggle-switch">
              <input
                type="checkbox"
                checked={autoplay}
                onChange={(e) => setAutoplay(e.target.checked)}
              />
              <span className="watch-toggle-slider" />
            </label>
          </div>

          <div className="watch-setting-row">
            <div className="watch-setting-info">
              <span className="watch-setting-title">Pausar al salir de pantalla</span>
              <span className="watch-setting-desc">
                Detener el video cuando no esté visible en tu vista.
              </span>
            </div>
            <label className="watch-toggle-switch">
              <input
                type="checkbox"
                checked={pausarFueraPantalla}
                onChange={(e) => setPausarFueraPantalla(e.target.checked)}
              />
              <span className="watch-toggle-slider" />
            </label>
          </div>

          <div className="watch-setting-row">
            <div className="watch-setting-info">
              <span className="watch-setting-title">Repetición continua (Loop)</span>
              <span className="watch-setting-desc">
                Volver a reproducir el video automáticamente al terminar.
              </span>
            </div>
            <label className="watch-toggle-switch">
              <input
                type="checkbox"
                checked={loopInfinito}
                onChange={(e) => setLoopInfinito(e.target.checked)}
              />
              <span className="watch-toggle-slider" />
            </label>
          </div>

          {/* SECCIÓN 2: SONIDO */}
          <div className="watch-settings-group-header">Audio y Sonido</div>

          <div className="watch-setting-row">
            <div className="watch-setting-info">
              <span className="watch-setting-title">Iniciar con sonido</span>
              <span className="watch-setting-desc">
                Si está desactivado, los videos arrancan en silencio hasta que toques el botón de audio.
              </span>
            </div>
            <label className="watch-toggle-switch">
              <input
                type="checkbox"
                checked={sonidoInicial}
                onChange={(e) => setSonidoInicial(e.target.checked)}
              />
              <span className="watch-toggle-slider" />
            </label>
          </div>

          {/* SECCIÓN 3: GESTOS Y PANTALLA */}
          <div className="watch-settings-group-header">Gestos y visualización</div>

          <div className="watch-setting-row">
            <div className="watch-setting-info">
              <span className="watch-setting-title">Doble toque: pantalla completa</span>
              <span className="watch-setting-desc">
                Tocar dos veces sobre el video para entrar en pantalla completa nativa.
              </span>
            </div>
            <label className="watch-toggle-switch">
              <input
                type="checkbox"
                checked={dobleTapPantallaCompleta}
                onChange={(e) => setDobleTapPantallaCompleta(e.target.checked)}
              />
              <span className="watch-toggle-slider" />
            </label>
          </div>

          {/* SECCIÓN 4: DATOS MÓVILES */}
          <div className="watch-settings-group-header">Consumo de datos móviles</div>

          <div className="watch-setting-row">
            <div className="watch-setting-info">
              <span className="watch-setting-title">Modo ahorro de datos</span>
              <span className="watch-setting-desc">
                Descarga datos solo cuando presionas Play para ahorrar megas en tu plan móvil.
              </span>
            </div>
            <label className="watch-toggle-switch">
              <input
                type="checkbox"
                checked={ahorroDatos}
                onChange={(e) => setAhorroDatos(e.target.checked)}
              />
              <span className="watch-toggle-slider" />
            </label>
          </div>

          <div className="watch-setting-row">
            <div className="watch-setting-info">
              <span className="watch-setting-title">Calidad de transmisión</span>
              <span className="watch-setting-desc">
                Selecciona la calidad preferida de reproducción.
              </span>
            </div>
            <select
              className="watch-select-input"
              value={calidad}
              onChange={(e) => setCalidad(e.target.value)}
            >
              <option value="alta">Alta definición (HD)</option>
              <option value="estandar">Estándar equilibrada</option>
              <option value="ahorro">Bajo consumo</option>
            </select>
          </div>
        </div>

        <div className="watch-modal-actions">
          <button type="button" className="btn btn-ghost" onClick={restaurar}>
            Restablecer
          </button>
          <button type="button" className="btn btn-aurora" onClick={guardar}>
            Guardar cambios
          </button>
        </div>
      </div>
    </div>
  );
}

/** Pantalla principal de Videos (Moon Watch) */
export default function VideosView() {
  const [categoria, setCategoria] = useState('para_ti');
  const [videos, setVideos] = useState([]);
  const [cargando, setCargando] = useState(true);

  // Configuración avanzada de usuario guardada localmente
  const [config, setConfig] = useState(() => {
    try {
      const guardada = localStorage.getItem('moon_watch_config_v2');
      if (guardada) return { ...CONFIG_POR_DEFECTO, ...JSON.parse(guardada) };
    } catch {}
    return CONFIG_POR_DEFECTO;
  });

  // Modal de Ajustes Avanzados
  const [mostrarAjustes, setMostrarAjustes] = useState(false);

  // Modal de subida de video
  const [mostrarModal, setMostrarModal] = useState(false);
  const [archivoSeleccionado, setArchivoSeleccionado] = useState(null);
  const [previewUrl, setPreviewUrl] = useState('');
  const [descripcionVideo, setDescripcionVideo] = useState('');
  const [subiendoPct, setSubiendoPct] = useState(0);
  const [publicando, setPublicando] = useState(false);
  const inputArchivoRef = useRef(null);

  // Guardar configuración
  function onGuardarConfig(nuevaConfig) {
    setConfig(nuevaConfig);
    try {
      localStorage.setItem('moon_watch_config_v2', JSON.stringify(nuevaConfig));
    } catch {}
  }

  // Cargar videos de Moon
  useEffect(() => {
    let activo = true;
    setCargando(true);

    const params = new URLSearchParams();
    params.set('cat', categoria);
    params.set('limit', '30');

    api.get(`/api/videos?${params.toString()}`)
      .then((res) => {
        if (!activo) return;
        const lista = Array.isArray(res) ? res : (res?.items || []);
        setVideos(lista);
      })
      .catch((err) => {
        console.error('[watch] error cargando videos:', err);
      })
      .finally(() => {
        if (activo) setCargando(false);
      });

    return () => { activo = false; };
  }, [categoria]);

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

      const postListo = {
        ...postCreado,
        images: (postCreado?.images && postCreado.images.length > 0)
          ? postCreado.images.map((im) => ({ ...im, kind: 'video' }))
          : [{ id: resMedia.id, url: resMedia.url, original_url: resMedia.url, thumb_url: resMedia.url, kind: 'video' }],
      };

      setVideos((prev) => [postListo, ...prev.filter((p) => p.id !== postListo.id)]);

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
      {/* 1. Cabecera Estilizada de Videos (Limpia, sin lupas ni desbordes) */}
      <div className="watch-fb-header">
        <div className="watch-fb-title-bar">
          <div className="watch-brand-zone">
            <h1 className="watch-fb-title">Videos</h1>
            <span className="watch-badge-live">Watch</span>
          </div>

          <div className="watch-header-actions">
            <button
              type="button"
              className="watch-settings-btn"
              onClick={() => setMostrarAjustes(true)}
              title="Ajustes y opciones avanzadas de video"
              aria-label="Ajustes y opciones avanzadas de video"
            >
              <IconSettings />
              <span>Ajustes</span>
            </button>

            <button
              type="button"
              className="watch-subir-btn-hero"
              onClick={() => setMostrarModal(true)}
            >
              <IconVideo />
              <span>Subir video</span>
            </button>
          </div>
        </div>
      </div>

      {/* 2. Pestañas de categorías */}
      <div className="watch-fb-tabs">
        {PESTANAS.map((pestana) => (
          <button
            key={pestana.id}
            type="button"
            className={`watch-fb-pill ${categoria === pestana.id ? 'active' : ''}`}
            onClick={() => setCategoria(pestana.id)}
          >
            {pestana.label}
          </button>
        ))}
      </div>

      {/* 3. Feed de videos: el primer video aparece inmediatamente limpio */}
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
            config={config}
            onEliminar={(idEliminado) => {
              setVideos((prev) => prev.filter((p) => p.id !== idEliminado));
            }}
            onActualizar={() => {}}
          />
        ))}
      </div>

      {/* 4. Modal de Opciones y Ajustes Avanzados de Videos */}
      {mostrarAjustes ? (
        <AjustesAvanzadosVideosModal
          config={config}
          onGuardar={onGuardarConfig}
          onCerrar={() => setMostrarAjustes(false)}
        />
      ) : null}

      {/* 5. Modal de Subida de Video */}
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
