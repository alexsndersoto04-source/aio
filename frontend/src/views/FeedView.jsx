// Moon — Feed principal (Inicio)

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api.js';
import { avisoError } from '../ui.js';
import PostCard from '../components/PostCard.jsx';
import Composer from '../components/Composer.jsx';
import Historias from '../components/Historias.jsx';
import { SugerenciasPersonas } from '../components/Sugerencias.jsx';
import { useAuth } from '../auth.jsx';
import Avatar from '../components/Avatar.jsx';
import { realtime } from '../realtime.js';
import { toast } from '../ui.js';
import {
  IconRefresh, IconBookmark, IconLayers, IconUsers, IconSettings, IconSearch,
  IconTrend, IconImage, IconMore, IconPlus, IconMail,
} from '../components/Icons.jsx';

const TABS = [
  { id: 'feed', label: 'Para ti' },
  { id: 'trending', label: 'Tendencias' },
  { id: 'latest', label: 'Recientes' },
];

// Con qué se puede filtrar el inicio.
const FILTROS = [
  { id: '', label: 'Todo', icono: <IconLayers /> },
  { id: 'fotos', label: 'Fotos', icono: <IconImage /> },
  { id: 'encuestas', label: 'Encuestas', icono: <IconTrend /> },
  { id: 'texto', label: 'Solo texto', icono: <IconBookmark /> },
];

// Atajos del menú de opciones del inicio.
const ATAJOS = [
  { href: '#/explore', label: 'Buscar en Moon', desc: 'Personas, etiquetas y grupos', icono: <IconSearch /> },
  { href: '#/amigos', label: 'Contactos', desc: 'Quién está en línea ahora', icono: <IconUsers /> },
  { href: '#/grupos', label: 'Mis grupos', desc: 'Lo que se publica en tus grupos', icono: <IconLayers /> },
  { href: '#/profile/saved', label: 'Guardados', desc: 'Lo que guardaste para después', icono: <IconBookmark /> },
  { href: '#/notifications', label: 'Avisos', desc: 'Me gusta, comentarios y menciones', icono: <IconMail /> },
  { href: '#/settings', label: 'Ajustes', desc: 'Perfil, privacidad y apariencia', icono: <IconSettings /> },
];

/** Franja de gente conectada ahora mismo, con presencia real del servidor. */
function EnLinea() {
  const { user } = useAuth();
  const [datos, setDatos] = useState(null);

  useEffect(() => {
    if (!user) return undefined;
    let vivo = true;
    const cargar = () => api.get('/api/users/presence')
      .then((res) => { if (vivo) setDatos(res); })
      .catch(() => {});
    cargar();
    const off = realtime.on((ev) => { if (ev.type === 'presence') cargar(); });
    return () => { vivo = false; off(); };
  }, [user]);

  const gente = (datos?.en_linea || []).slice(0, 6);
  if (gente.length === 0) return null;

  return (
    <a className="franja-en-linea" href="#/amigos">
      <span className="pila-avatares" aria-hidden="true">
        {gente.map((u) => (
          <span className="avatar-con-estado" key={u.id}>
            <Avatar user={u} className="mini" />
            <span className="punto-online" />
          </span>
        ))}
      </span>
      <span className="etiqueta-viva"><span className="luz" />{datos.total_en_linea} en línea ahora</span>
      <span className="ver">Ver contactos</span>
    </a>
  );
}

export default function FeedView() {
  const { user } = useAuth();
  const [tab, setTab] = useState('feed');
  const [filtro, setFiltro] = useState('');
  const [menu, setMenu] = useState(false);
  const [refrescando, setRefrescando] = useState(false);
  const [posts, setPosts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [more, setMore] = useState(false);
  const loadRef = useRef(null);

  const load = useCallback(async (t, p, append, tipo = '') => {
    try {
      const path = t === 'feed' ? '/api/feed' : `/api/feed/${t}`;
      const res = await api.get(`${path}?page=${p}&limit=10${tipo ? `&tipo=${tipo}` : ''}`);
      const items = res.items || [];
      setPosts((prev) => (append ? [...prev, ...items] : items));
      setTotal(res.total || items.length);
      setMore(items.length === 10 && (p * 10) < (res.total || 0));
    } catch (e) {
      avisoError(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    setLoading(true);
    setPage(1);
    load(tab, 1, false, filtro);
  }, [tab, filtro, load]);

  /** Volver a pedir lo primero sin perder el sitio. */
  async function actualizar() {
    setRefrescando(true);
    await load(tab, 1, false, filtro);
    setRefrescando(false);
    toast.ok('Al día');
  }

  // Scroll infinito
  useEffect(() => {
    const el = loadRef.current;
    if (!el) return;
    // Navegadores sin IntersectionObserver: queda el botón «Ver más».
    if (typeof IntersectionObserver === 'undefined') return;
    const obs = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting && !loading && more) {
        setPage((p) => {
          const next = p + 1;
          load(tab, next, true, filtro);
          return next;
        });
      }
    }, { rootMargin: '300px' });
    obs.observe(el);
    return () => obs.disconnect();
  }, [loading, more, tab, filtro, load]);

  function onCreated(post) {
    setPosts((prev) => [post, ...prev]);
  }

  return (
    <>
      <div className="topbar barra-inicio">
        <h1>{tab === 'feed' ? 'Inicio' : tab === 'trending' ? 'Tendencias' : 'Recientes'}</h1>
        <button
          type="button"
          className={`icon-btn${refrescando ? ' girando' : ''}`}
          onClick={actualizar}
          aria-label="Ver lo más nuevo"
          title="Ver lo más nuevo"
        >
          <IconRefresh />
        </button>
        <button
          type="button"
          className="icon-btn"
          onClick={() => setMenu((v) => !v)}
          aria-expanded={menu}
          aria-label="Más opciones del inicio"
          title="Más opciones"
        >
          <IconMore />
        </button>
      </div>

      {menu ? (
        <div className="menu-inicio">
          {ATAJOS.map((a) => (
            <a key={a.href} href={a.href} className="atajo" onClick={() => setMenu(false)}>
              <span className="icono">{a.icono}</span>
              <span className="texto"><b>{a.label}</b><small>{a.desc}</small></span>
            </a>
          ))}
          <button
            type="button"
            className="atajo como-boton"
            onClick={() => {
              setMenu(false);
              window.dispatchEvent(new CustomEvent('moon:componer'));
            }}
          >
            <span className="icono"><IconPlus /></span>
            <span className="texto"><b>Escribir una publicación</b><small>Texto, fotos o una encuesta</small></span>
          </button>
        </div>
      ) : null}

      <EnLinea />
      {tab === 'feed' ? <Historias /> : null}
      {tab === 'feed' ? <Composer onCreated={onCreated} /> : null}

      <div className="tabs">
        {TABS.map((t) => (
          <button key={t.id} className={tab === t.id ? 'active' : ''} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>

      {/* Con esto decides qué quieres ver, sin que cambie de pantalla. */}
      <div className="chips-filtro" role="group" aria-label="Filtrar publicaciones">
        {FILTROS.map((f) => (
          <button
            key={f.id || 'todo'}
            type="button"
            className={filtro === f.id ? 'activo' : ''}
            aria-pressed={filtro === f.id}
            onClick={() => setFiltro(f.id)}
          >
            {f.icono}
            {f.label}
          </button>
        ))}
      </div>

      {loading ? <div className="spinner" /> : null}

      {!loading && posts.length === 0 && filtro ? (
        <div className="card empty">
          <h3>Nada por aquí con ese filtro</h3>
          <p>No hay publicaciones de ese tipo todavía.</p>
          <button type="button" className="btn btn-outline" onClick={() => setFiltro('')}>Ver todo</button>
        </div>
      ) : null}

      {!loading && posts.length === 0 && !filtro ? (
        <>
          <div className="card empty">
            <div className="moon-emoji">🌙</div>
            <h3>Sin publicaciones todavía</h3>
            <p>Sigue a personas para llenar tu feed, o publica algo tú mismo.</p>
            <a className="btn btn-aurora" href="#/explore">Explorar</a>
          </div>
          <SugerenciasPersonas />
        </>
      ) : null}

      {posts.map((post) => (
        <PostCard key={post.id} post={post}
          onChanged={(next) => setPosts((prev) => prev.map((x) => (x.id === next.id ? next : x)))} />
      ))}

      <div ref={loadRef} />
      {more ? <div className="spinner" style={{ margin: '12px auto' }} /> : null}
    </>
  );
}
