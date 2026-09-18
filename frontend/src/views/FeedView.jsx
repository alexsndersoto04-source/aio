// Moon — Feed principal (Inicio)

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { IlustraInicio } from '../components/Ilustraciones.jsx';
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
import { IconRefresh, IconCamera } from '../components/Icons.jsx';
import { palabrasSilenciadas } from '../prefs.js';

const TABS = [
  { id: 'feed', label: 'Para ti' },
  { id: 'trending', label: 'Tendencias' },
  { id: 'latest', label: 'Recientes' },
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
  const [refrescando, setRefrescando] = useState(false);
  const [ocultas, setOcultas] = useState(0);
  // El redactor se abre a pantalla completa: arriba solo queda la fila fina.
  const [redactor, setRedactor] = useState(false);
  const [posts, setPosts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [more, setMore] = useState(false);
  const loadRef = useRef(null);

  const load = useCallback(async (t, p, append) => {
    try {
      const path = t === 'feed' ? '/api/feed' : `/api/feed/${t}`;
      const res = await api.get(`${path}?page=${p}&limit=10`);
      const silenciadas = palabrasSilenciadas();
      // Lo que traiga una palabra silenciada no se muestra (Ajustes → Contenido).
      const items = (res.items || []).filter((post) => {
        if (silenciadas.length === 0) return true;
        const t = String(post.content || '').toLowerCase();
        return !silenciadas.some((palabra) => t.includes(palabra));
      });
      setPosts((prev) => (append ? [...prev, ...items] : items));
      setTotal(res.total || items.length);
      const quitadas = (res.items || []).length - items.length;
      if (quitadas > 0) setOcultas((n) => n + quitadas);
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
    load(tab, 1, false);
  }, [tab, load]);

  /** Volver a pedir lo primero sin perder el sitio. */
  async function actualizar() {
    setRefrescando(true);
    await load(tab, 1, false);
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
          load(tab, next, true);
          return next;
        });
      }
    }, { rootMargin: '300px' });
    obs.observe(el);
    return () => obs.disconnect();
  }, [loading, more, tab, load]);

  function onCreated(post) {
    setPosts((prev) => [post, ...prev]);
  }

  return (
    <>
      <div className="topbar barra-inicio">
        <h1>{tab === 'feed' ? 'Inicio' : tab === 'trending' ? 'Tendencias' : 'Recientes'}</h1>
        <span className="spacer" />
        <button
          type="button"
          className={`icon-btn${refrescando ? ' girando' : ''}`}
          onClick={actualizar}
          aria-label="Ver lo más nuevo"
          title="Ver lo más nuevo"
        >
          <IconRefresh />
        </button>
      </div>

      <EnLinea />

      {/* Crear: una sola línea. Al tocarla se abre el redactor a pantalla completa. */}
      <button type="button" className="crear-rapido" onClick={() => setRedactor(true)}>
        <Avatar user={user} className="mini" />
        <span className="texto">¿Qué está pasando en tu órbita?</span>
        <span className="icono"><IconCamera /></span>
      </button>

      {tab === 'feed' ? <Historias /> : null}

      <div className="tabs tabs-inicio">
        {TABS.map((t) => (
          <button key={t.id} className={tab === t.id ? 'active' : ''} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>

      {loading ? <div className="spinner" /> : null}

      {ocultas > 0 ? (
        <div className="aviso-silenciadas">
          <span>
            {ocultas === 1
              ? 'Se ocultó 1 publicación por las palabras que silenciaste.'
              : `Se ocultaron ${ocultas} publicaciones por las palabras que silenciaste.`}
          </span>
          <a href="#/settings/contenido">Ver la lista</a>
        </div>
      ) : null}

      {!loading && posts.length === 0 ? (
        <>
          <div className="card empty">
            <IlustraInicio />
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

      {redactor ? (
        <div className="pantalla-redactor" role="dialog" aria-modal="true" aria-label="Escribir una publicación">
          <header className="cabecera-redactor">
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => setRedactor(false)}>
              Cancelar
            </button>
            <b>Nueva publicación</b>
            <span className="hueco" />
          </header>
          <div className="cuerpo-redactor">
            <Composer
              onCreated={(post) => { onCreated(post); setRedactor(false); }}
              placeholder="¿Qué está pasando en tu órbita?"
            />
          </div>
        </div>
      ) : null}
    </>
  );
}
