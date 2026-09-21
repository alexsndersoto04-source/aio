// Moon Watch — Feed de videos con YouTube y Comunidad Moon
// ============================================================
// Entretenimiento real en español sin bots ni contenido falso de IA.
// Diseño plano idéntico a Facebook Watch, con miniaturas HD y
// reproductor fluido de YouTube sin necesidad de tarjeta ni Google Cloud.

import React, { useState, useEffect } from 'react';
import { api, imgUrl } from '../api.js';
import {
  IconSearch, IconX, IconHeart, IconComment, IconSend,
} from '../components/Icons.jsx';

const CATEGORIAS = [
  { id: 'tendencias', label: 'Tendencias' },
  { id: 'comedia', label: 'Comedia & Creadores' },
  { id: 'musica', label: 'Música' },
  { id: 'deportes', label: 'Deportes' },
  { id: 'gaming', label: 'Gaming' },
  { id: 'moon', label: 'Comunidad Moon' },
];

// Catálogo curado de videos en español de primer nivel (YouTube)
const CATALOGO_YOUTUBE = {
  tendencias: [
    {
      id: '0e3GPea1Tyg',
      title: '¡Sobreviví 7 Días En Una Ciudad Abandonada!',
      author: 'MrBeast en Español',
      verified: true,
      duration: '18:42',
      views: '42.8 M',
      avatar: 'M',
    },
    {
      id: 'b3_l344U2Gw',
      title: '¡Construí 100 Casas Y Las Regalé a Familias Necesitadas!',
      author: 'MrBeast en Español',
      verified: true,
      duration: '16:15',
      views: '54.2 M',
      avatar: 'M',
    },
    {
      id: 'yD8sNnJ_v-k',
      title: 'LA VELADA DEL AÑO — Los Momentos Más Épicos e Históricos',
      author: 'Ibai',
      verified: true,
      duration: '22:10',
      views: '18.9 M',
      avatar: 'I',
    },
    {
      id: 'V1bFr2SWP1I',
      title: 'REACCIONANDO A LOS PEORES TIKTOKS DE LA HISTORIA',
      author: 'Auron',
      verified: true,
      duration: '14:35',
      views: '11.5 M',
      avatar: 'A',
    },
    {
      id: 'dQ7Ym0b943M',
      title: 'Te Lo Dijo El Chombo — Lo Que Nadie Sabe De La Música Urbana',
      author: 'El Chombo',
      verified: true,
      duration: '15:20',
      views: '9.3 M',
      avatar: 'C',
    },
  ],
  comedia: [
    {
      id: '2Q_ZzIat97Y',
      title: 'RPS: Cosas de Parejas y Situaciones de la Vida Real',
      author: 'Franco Escamilla',
      verified: true,
      duration: '24:50',
      views: '28.1 M',
      avatar: 'F',
    },
    {
      id: '7R1N4hsFD9g',
      title: 'Viendo el Teléfono de tu Pareja (Sketch de Humor)',
      author: 'enchufetv',
      verified: true,
      duration: '4:15',
      views: '35.4 M',
      avatar: 'E',
    },
    {
      id: 'p9H1C5_rR3E',
      title: 'Harina — El Teniente Harina (Video Completo Oficial)',
      author: 'Backdoor - Humor por donde no pasa la luz',
      verified: true,
      duration: '3:50',
      views: '68.0 M',
      avatar: 'B',
    },
    {
      id: 'gM8p1c_h0xE',
      title: 'Probando la Comida Callejera Más Peligrosa y Picante',
      author: 'Luisito Comunica',
      verified: true,
      duration: '17:30',
      views: '19.7 M',
      avatar: 'L',
    },
    {
      id: '60ItHLz5WEA',
      title: 'Los 7 Misterios y Secretos Más Aterradores del Mundo',
      author: 'DrossRotzank',
      verified: true,
      duration: '16:04',
      views: '14.2 M',
      avatar: 'D',
    },
  ],
  musica: [
    {
      id: 'CocEMWmpcfs',
      title: 'SHAKIRA || BZRP Music Sessions #53 (Official Video)',
      author: 'Bizarrap',
      verified: true,
      duration: '3:38',
      views: '730 M',
      avatar: 'B',
    },
    {
      id: '_X3qMs8U_9c',
      title: 'Bad Bunny - MONACO (Official Video con Al Pacino)',
      author: 'Bad Bunny',
      verified: true,
      duration: '7:12',
      views: '185 M',
      avatar: 'B',
    },
    {
      id: 'A_g3lMcWVy0',
      title: 'QUEVEDO || BZRP Music Sessions #52 (Quédate)',
      author: 'Bizarrap',
      verified: true,
      duration: '3:19',
      views: '670 M',
      avatar: 'B',
    },
    {
      id: 'zD_ZqEw7F8g',
      title: 'KAROL G, Peso Pluma - QLONA (Official Video)',
      author: 'Karol G',
      verified: true,
      duration: '2:53',
      views: '410 M',
      avatar: 'K',
    },
    {
      id: '5p_UkyH1nF8',
      title: 'Feid, Young Miko - CLASSY 101 (Official Video)',
      author: 'Feid',
      verified: true,
      duration: '3:20',
      views: '390 M',
      avatar: 'F',
    },
  ],
  deportes: [
    {
      id: '19rZ5f0wVbQ',
      title: 'Los 10 Mejores Golazos Históricos de Lionel Messi en LaLiga',
      author: 'LaLiga EA Sports Oficial',
      verified: true,
      duration: '12:05',
      views: '32.1 M',
      avatar: 'L',
    },
    {
      id: 'E4qBvjQ2_3c',
      title: 'Real Madrid — Las Remontadas Épicas de Champions League',
      author: 'UEFA Champions League',
      verified: true,
      duration: '14:40',
      views: '21.4 M',
      avatar: 'R',
    },
    {
      id: 'oF5eF5v0Lw8',
      title: 'Las 50 Mejores Clavadas y Mates en la Historia de la NBA',
      author: 'NBA Latam Oficial',
      verified: true,
      duration: '15:18',
      views: '16.7 M',
      avatar: 'N',
    },
    {
      id: 'p6_pG2mQ7jE',
      title: 'Las Mejores Rimas y Batallas Épicas de Freestyle en Español',
      author: 'Red Bull Batalla',
      verified: true,
      duration: '18:55',
      views: '14.0 M',
      avatar: 'R',
    },
  ],
  gaming: [
    {
      id: 'xQ3t8L4g1rA',
      title: 'Risas y Momentos Inolvidables en Minecraft con Amigos',
      author: 'VEGETTA777',
      verified: true,
      duration: '21:10',
      views: '15.6 M',
      avatar: 'V',
    },
    {
      id: 'L_LUpnjgPso',
      title: 'Troleando en GTA V Online Modo Caos Total con Streamers',
      author: 'elrubiusOMG',
      verified: true,
      duration: '19:45',
      views: '22.3 M',
      avatar: 'R',
    },
    {
      id: 'fB8UUm_nF5E',
      title: 'Sobreviviendo 100 Días Extremos con Todo en Contra',
      author: 'Spreen',
      verified: true,
      duration: '25:30',
      views: '12.8 M',
      avatar: 'S',
    },
    {
      id: 'vjB3x7qQz8I',
      title: 'El Evento Histórico que Rompió el Récord Mundial de Directos',
      author: 'TheGrefg',
      verified: true,
      duration: '18:22',
      views: '19.1 M',
      avatar: 'G',
    },
  ],
};

function extraerIdYoutube(cadena) {
  if (!cadena) return null;
  const c = cadena.trim();
  const m = c.match(/(?:youtu\.be\/|youtube\.com\/(?:embed\/|v\/|watch\?v=|watch\?.+&v=))([\w-]{11})/);
  if (m) return m[1];
  if (/^[\w-]{11}$/.test(c)) return c;
  return null;
}

export default function VideosView() {
  const [categoria, setCategoria] = useState('tendencias');
  const [query, setQuery] = useState('');
  const [busqueda, setBusqueda] = useState('');
  const [videosMoon, setVideosMoon] = useState([]);
  const [cargandoMoon, setCargandoMoon] = useState(false);
  const [playingId, setPlayingId] = useState(null);
  const [liked, setLiked] = useState({});
  const [following, setFollowing] = useState({});

  // Cargar videos de la comunidad Moon si se elige esa pestaña
  useEffect(() => {
    if (categoria === 'moon') {
      let vivo = true;
      setCargandoMoon(true);
      api.get('/api/feed?tipo=videos&limit=20')
        .then((res) => {
          if (!vivo) return;
          const items = (res?.items || res || []).map((p) => {
            const vidObj = (p.images || []).find((im) => {
              const u = im.original_url || im.url || '';
              return im.kind === 'video' || /\.(mp4|webm|mov|mkv|3gp)(\?.*)?$/i.test(u);
            });
            return {
              id: `moon-${p.id}`,
              esMoon: true,
              videoUrl: vidObj ? (vidObj.original_url || vidObj.url) : '',
              title: p.content || 'Video de la comunidad Moon',
              author: p.user?.display_name || p.user?.username || 'Usuario Moon',
              avatar: (p.user?.username || 'M').charAt(0).toUpperCase(),
              duration: 'Moon',
              views: `${p.likes_count || 1} me gusta`,
            };
          }).filter((x) => Boolean(x.videoUrl));
          setVideosMoon(items);
        })
        .catch(() => {})
        .finally(() => { if (vivo) setCargandoMoon(false); });
      return () => { vivo = false; };
    }
    return undefined;
  }, [categoria]);

  function handleSearch(e) {
    e.preventDefault();
    setBusqueda(query.trim());
    setPlayingId(null);
  }

  // Si el usuario pega un enlace de YouTube en el buscador:
  const ytIdDirecto = extraerIdYoutube(busqueda);

  // Lista de videos a mostrar
  let listaMostrar = [];

  if (categoria === 'moon') {
    listaMostrar = videosMoon;
  } else {
    const listaBase = CATALOGO_YOUTUBE[categoria] || CATALOGO_YOUTUBE.tendencias;
    if (ytIdDirecto) {
      listaMostrar = [
        {
          id: ytIdDirecto,
          title: `Video de YouTube (${ytIdDirecto})`,
          author: 'YouTube',
          verified: true,
          duration: 'En vivo',
          views: 'En reproducción',
          avatar: 'Y',
        },
        ...listaBase,
      ];
    } else if (busqueda) {
      const q = busqueda.toLowerCase();
      listaMostrar = Object.values(CATALOGO_YOUTUBE).flat().filter((v) => (
        v.title.toLowerCase().includes(q) || v.author.toLowerCase().includes(q)
      ));
    } else {
      listaMostrar = listaBase;
    }
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
            placeholder="Buscar videos o pega un enlace de YouTube..."
          />
          {query ? (
            <button
              type="button"
              className="clear-btn"
              onClick={() => { setQuery(''); setBusqueda(''); }}
            >
              <IconX />
            </button>
          ) : null}
        </form>
      </div>

      {/* 2. Barra de categorías */}
      <div className="watch-fb-tabs">
        {CATEGORIAS.map((cat) => (
          <button
            key={cat.id}
            type="button"
            className={`watch-fb-pill ${categoria === cat.id && !busqueda ? 'active' : ''}`}
            onClick={() => {
              setBusqueda('');
              setQuery('');
              setCategoria(cat.id);
              setPlayingId(null);
            }}
          >
            {cat.label}
          </button>
        ))}
      </div>

      {/* 3. Feed de videos */}
      <div className="watch-fb-feed">
        {cargandoMoon && (
          <div style={{ padding: '30px 16px', textAlign: 'center', color: 'var(--muted)' }}>
            Cargando videos de la comunidad…
          </div>
        )}

        {categoria === 'moon' && !cargandoMoon && videosMoon.length === 0 && (
          <div style={{ padding: '40px 16px', textAlign: 'center', color: 'var(--muted)' }}>
            <p style={{ fontSize: 16, fontWeight: 600, color: 'var(--text)', marginBottom: 6 }}>
              Aún no hay videos en la comunidad Moon
            </p>
            <p style={{ fontSize: 14 }}>
              ¡Sé el primero en subir un video con el botón de publicar de Moon!
            </p>
          </div>
        )}

        {listaMostrar.map((vid) => {
          const isPlaying = playingId === vid.id;
          const isLiked = Boolean(liked[vid.id]);
          const isFollowed = Boolean(following[vid.author]);
          const thumbUrl = vid.esMoon
            ? imgUrl(vid.videoUrl)
            : `https://i.ytimg.com/vi/${vid.id}/hqdefault.jpg`;

          return (
            <article key={vid.id} className="watch-fb-card">
              {/* Cabecera del video */}
              <div className="watch-card-header">
                <div className="watch-creator-info">
                  <div className="watch-avatar-circle">
                    {vid.avatar || vid.author.charAt(0).toUpperCase()}
                  </div>
                  <div className="watch-creator-texts">
                    <span className="watch-creator-name">{vid.author}</span>
                    <span className="watch-post-date">
                      {vid.esMoon ? 'Moon Video' : 'Canal verificado · YouTube'}
                    </span>
                  </div>
                </div>

                <button
                  type="button"
                  className="watch-follow-btn"
                  onClick={() => setFollowing((prev) => ({ ...prev, [vid.author]: !prev[vid.author] }))}
                >
                  {isFollowed ? 'Siguiendo' : 'Seguir'}
                </button>
              </div>

              {/* Título de la publicación */}
              <p className="watch-card-text">{vid.title}</p>

              {/* Video o Portada 16:9 borde a borde */}
              <div className="watch-media-box">
                {isPlaying ? (
                  vid.esMoon ? (
                    <video
                      src={imgUrl(vid.videoUrl)}
                      controls
                      autoPlay
                      playsInline
                      className="post-video-player"
                    />
                  ) : (
                    <iframe
                      title={vid.title}
                      src={`https://www.youtube-nocookie.com/embed/${vid.id}?autoplay=1&rel=0&modestbranding=1&playsinline=1`}
                      allowFullScreen
                      allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
                    />
                  )
                ) : (
                  <div
                    className="watch-thumbnail-wrapper"
                    onClick={() => setPlayingId(vid.id)}
                    role="button"
                    tabIndex={0}
                  >
                    <img
                      src={thumbUrl}
                      alt={vid.title}
                      loading="lazy"
                    />
                    <span className="watch-duration-tag">{vid.duration}</span>
                    <div className="watch-play-button-overlay">
                      <div className="watch-play-circle-icon">▶</div>
                    </div>
                  </div>
                )}
              </div>

              {/* Estadísticas compactas */}
              <div className="watch-stats-row">
                <span>{vid.views}</span>
                <span>{isLiked ? 'Te gusta este video' : 'Reacciones'}</span>
              </div>

              {/* Barra inferior compacta de reacciones */}
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
                    const shareUrl = vid.esMoon
                      ? window.location.href
                      : `https://www.youtube.com/watch?v=/${vid.id}`;
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
