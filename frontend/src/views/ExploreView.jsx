// Moon — Explorar: búsqueda real (usuarios/posts/hashtags)

import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../api.js';
import { avisoError } from '../ui.js';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { debounce, plural } from '../utils.js';
import { IconSearch, IconUsers, IconLayers, IconImage, IconTrend } from '../components/Icons.jsx';
import { SugerenciasPersonas } from '../components/Sugerencias.jsx';
import { palabrasSilenciadas } from '../prefs.js';

const TIPOS = [
  { id: 'users', label: 'Personas', icono: <IconUsers /> },
  { id: 'posts', label: 'Publicaciones', icono: <IconImage /> },
  { id: 'groups', label: 'Grupos', icono: <IconLayers /> },
  { id: 'tags', label: 'Etiquetas', icono: <IconTrend /> },
];

const ORDENES = [
  { id: 'recientes', label: 'Recientes' },
  { id: 'populares', label: 'Con más me gusta' },
];

const TIPO_VALIDO = (t) => (TIPOS.some((x) => x.id === t) ? t : 'users');

export default function ExploreView({ initialQ = '', initialType = 'users' }) {
  const [q, setQ] = useState(initialQ);
  const [type, setType] = useState(TIPO_VALIDO(initialType));
  const [orden, setOrden] = useState('recientes');
  const [users, setUsers] = useState([]);
  const [posts, setPosts] = useState([]);
  const [grupos, setGrupos] = useState([]);
  const [etiquetas, setEtiquetas] = useState([]);
  const [trendingTags, setTrendingTags] = useState([]);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);

  const runSearch = useCallback(debounce(async (query, t, ord = 'recientes') => {
    const limpio = query.trim();
    if (!limpio) {
      setUsers([]);
      setPosts([]);
      setGrupos([]);
      setEtiquetas([]);
      setSearched(false);
      return;
    }
    setLoading(true);
    try {
      const url = `/api/search?q=${encodeURIComponent(limpio)}&type=${t}${t === 'posts' ? `&orden=${ord}` : ''}`;
      const res = await api.get(url);
      let lista = Array.isArray(res) ? res : [];
      if (t === 'posts') {
        const malas = palabrasSilenciadas();
        if (malas.length) {
          lista = lista.filter((post) => {
            const texto = String(post.content || '').toLowerCase();
            return !malas.some((palabra) => texto.includes(palabra));
          });
        }
      }
      if (t === 'users') { setUsers(lista); setPosts([]); setGrupos([]); setEtiquetas([]); }
      else if (t === 'posts') { setPosts(lista); setUsers([]); setGrupos([]); setEtiquetas([]); }
      else if (t === 'groups') { setGrupos(lista); setUsers([]); setPosts([]); setEtiquetas([]); }
      else { setEtiquetas(lista); setUsers([]); setPosts([]); setGrupos([]); }
      setSearched(true);
    } catch (e) {
      avisoError(e);
    } finally {
      setLoading(false);
    }
  }, 400), []);

  useEffect(() => {
    api.get('/api/hashtags').then(setTrendingTags).catch(() => {});
  }, []);

  // Sincroniza cuando la URL cambia (p. ej. clic en #hashtag de un post).
  useEffect(() => {
    if (!initialQ) return;
    const t = TIPO_VALIDO(initialType);
    setQ(initialQ);
    setType(t);
    runSearch(initialQ, t, orden);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialQ, initialType]);

  function onType(t) {
    setType(t);
    if (q.trim()) runSearch(q, t, orden);
  }

  function onOrden(o) {
    setOrden(o);
    if (q.trim()) runSearch(q, type, o);
  }

  return (
    <>
      <div className="topbar"><h1>Explorar</h1></div>

      <div className="card" style={{ padding: 14, marginBottom: 16 }}>
        <div className="row" style={{ gap: 8 }}>
          <IconSearch style={{ color: 'var(--ink-3)', width: 20, height: 20 }} />
          <input
            className="input"
            style={{ border: 'none', boxShadow: 'none', padding: '8px 4px' }}
            placeholder="Busca personas, publicaciones…"
            value={q}
            onChange={(e) => { setQ(e.target.value); runSearch(e.target.value, type); }}
          />
        </div>
        <div className="tabs tabs-buscar" style={{ borderBottom: 'none', marginBottom: 0, marginTop: 6 }}>
          {TIPOS.map((t) => (
            <button key={t.id} className={type === t.id ? 'active' : ''} onClick={() => onType(t.id)}>
              {t.icono}
              {t.label}
            </button>
          ))}
        </div>

        {/* Solo las publicaciones se pueden ordenar */}
        {type === 'posts' ? (
          <div className="chips-filtro chips-buscar" role="group" aria-label="Ordenar resultados">
            {ORDENES.map((o) => (
              <button
                key={o.id}
                type="button"
                className={orden === o.id ? 'activo' : ''}
                aria-pressed={orden === o.id}
                onClick={() => onOrden(o.id)}
              >
                {o.label}
              </button>
            ))}
          </div>
        ) : null}
      </div>

      {loading ? <div className="spinner" /> : null}

      {!loading && !q.trim() ? (
        <div className="card" style={{ padding: 16 }}>
          <h3 style={{ margin: '0 0 8px', fontSize: 15, fontWeight: 800 }}>Tendencias</h3>
          {trendingTags.map((t) => (
            <a key={t.tag} href={`#/explore?q=${encodeURIComponent(t.tag)}&type=posts`}
              className="row" style={{ padding: '9px 4px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span className="hash">#{t.tag}</span>
              <span className="muted">{plural(t.posts_count, 'publicación')}</span>
            </a>
          ))}
          {trendingTags.length === 0 ? (
            <p className="muted" style={{ margin: 0 }}>
              Aún no hay hashtags: usa #algo al publicar y crea el primero.
            </p>
          ) : null}
        </div>
      ) : null}

      {!loading && !q.trim() ? <SugerenciasPersonas titulo="Personas destacadas" limite={4} /> : null}

      {!loading && searched && type === 'users' ? (
        users.length === 0 ? (
          <div className="card empty"><p>Sin resultados para «{q}».</p></div>
        ) : (
          users.map((u) => (
            <a key={u.id} href={`#/user/${u.username}`} className="card suggest" style={{ marginBottom: 10, textDecoration: 'none' }}>
              <Avatar user={u} />
              <span className="who">
                <span className="name" style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                  {u.display_name || u.username} <VerifiedBadge show={u.is_verified} />
                </span>
                <span className="at">@{u.username}</span>
              </span>
              <span className="muted">{plural(u.followers_count, 'seguidor')}</span>
            </a>
          ))
        )
      ) : null}

      {!loading && searched && type === 'posts' ? (
        posts.length === 0 ? (
          <div className="card empty"><p>Sin publicaciones para «{q}».</p></div>
        ) : (
          posts.map((post) => <PostCard key={post.id} post={post} />)
        )
      ) : null}

      {!loading && searched && type === 'groups' ? (
        grupos.length === 0 ? (
          <div className="card empty"><p>Ningún grupo se llama «{q}».</p></div>
        ) : (
          grupos.map((g) => (
            <a key={g.id} href={`#/grupo/${g.id}`} className="card tarjeta-resultado">
              <span className="marca"><IconLayers /></span>
              <span className="datos">
                <b>{g.name}</b>
                <small>{plural(g.miembros || 0, 'miembro')} · {g.privacy === 'private' ? 'privado' : 'abierto'}</small>
                {g.about ? <span className="muted small linea-2">{g.about}</span> : null}
              </span>
              <span className="accion">{g.soy_miembro ? 'Estás dentro' : 'Ver grupo'}</span>
            </a>
          ))
        )
      ) : null}

      {!loading && searched && type === 'tags' ? (
        etiquetas.length === 0 ? (
          <div className="card empty"><p>Ninguna etiqueta con «{q}».</p></div>
        ) : (
          <div className="card">
            {etiquetas.map((t) => (
              <a key={t.tag} href={`#/explore?q=${encodeURIComponent(t.tag)}&type=posts`} className="fila-etiqueta">
                <span className="hash">#{t.tag}</span>
                <span className="muted">{plural(t.posts_count, 'publicación')}</span>
              </a>
            ))}
          </div>
        )
      ) : null}
    </>
  );
}
