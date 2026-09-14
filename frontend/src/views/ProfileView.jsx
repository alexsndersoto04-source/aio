// Moon — Mi perfil (portada, números, fotos, grupos y guardados)
// ============================================================
// Todo lo que se ve sale de la base de datos: el perfil, los números, las
// publicaciones, la rejilla de fotos, mis grupos y lo que guardé.

import React, { useEffect, useMemo, useState } from 'react';
import { api, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { timeAgo } from '../utils.js';
import { toast, avisoError } from '../ui.js';
import {
  IconBookmark, IconGrid, IconLayers, IconSettings, IconShield, IconLink,
  IconMapPin, IconCalendar, IconHeart, IconUsers,
} from '../components/Icons.jsx';

const SECCIONES = [
  { id: 'posts', label: 'Publicaciones', icono: <IconGrid /> },
  { id: 'fotos', label: 'Fotos', icono: <IconGrid /> },
  { id: 'grupos', label: 'Grupos', icono: <IconLayers /> },
  { id: 'saved', label: 'Guardados', icono: <IconBookmark /> },
];

/** Rejilla de fotos: todas las imágenes de mis publicaciones, con su me gusta. */
function RejillaFotos({ posts, comentar }) {
  const fotos = useMemo(
    () => posts.flatMap((p) => (p.images || []).map((img) => ({ ...img, post: p }))),
    [posts]
  );
  if (fotos.length === 0) {
    return (
      <div className="card empty">
        <div className="moon-emoji">🌙</div>
        <h3>Sin fotos todavía</h3>
        <p>Cuando publiques imágenes aparecerán aquí, en rejilla.</p>
      </div>
    );
  }
  return (
    <div className="rejilla-perfil">
      {fotos.map((f, i) => (
        <a key={f.id || i} href={`#/post/${f.post.id}`} aria-label="Abrir la publicación">
          <img src={imgUrl(f.original_url || f.url)} alt="" loading="lazy" />
          <span className="me-gusta"><IconHeart filled /> {f.post.likes_count || 0}</span>
        </a>
      ))}
    </div>
  );
}

function MisGrupos() {
  const [grupos, setGrupos] = useState(null);
  useEffect(() => {
    api.get('/api/me/groups')
      .then((res) => setGrupos(res.mios || res || []))
      .catch(() => setGrupos([]));
  }, []);
  if (grupos === null) return <div className="spinner" />;
  if (grupos.length === 0) {
    return (
      <div className="card empty">
        <div className="moon-emoji">👥</div>
        <h3>Todavía no estás en grupos</h3>
        <p>Los grupos reúnen a la gente por tema. Puedes crear el tuyo.</p>
        <a className="btn" href="#/grupos">Ver grupos</a>
      </div>
    );
  }
  return (
    <div className="lista-simple">
      {grupos.map((g) => (
        <a className="fila-ajuste" key={g.id} href={`#/grupo/${g.id}`}>
          <span className="icono"><IconUsers /></span>
          <span className="texto">
            <b>{g.name}</b>
            <small>{g.members_count || 1} miembros · {g.privacy === 'private' ? 'Privado' : 'Público'}</small>
          </span>
        </a>
      ))}
    </div>
  );
}

export default function ProfileView({ tab }) {
  const { user, refreshMe, isAdmin } = useAuth();
  const [me, setMe] = useState(user);
  const [posts, setPosts] = useState([]);
  const [saved, setSaved] = useState([]);
  const [loading, setLoading] = useState(true);
  const [section, setSection] = useState(tab === 'saved' ? 'saved' : 'posts');

  useEffect(() => {
    api.get('/api/auth/me').then(setMe).catch(() => {});
    refreshMe();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    setLoading(true);
    if (section === 'posts' || section === 'fotos') {
      api.get(`/api/users/${me?.id}/posts?page=1&limit=20`)
        .then((res) => setPosts(res.items || []))
        .catch(() => {})
        .finally(() => setLoading(false));
    } else if (section === 'saved') {
      api.get('/api/me/saved?page=1&limit=20')
        .then((res) => setSaved(res.items || []))
        .catch(() => {})
        .finally(() => setLoading(false));
    } else {
      setLoading(false);
    }
  }, [section, me?.id]);

  async function compartirPerfil() {
    const url = `${window.location.origin}${window.location.pathname}#/user/${me?.id}`;
    try {
      if (navigator.share) { await navigator.share({ title: 'Mi perfil en Moon', url }); return; }
      await navigator.clipboard.writeText(url);
      toast.ok('Enlace de tu perfil copiado');
    } catch { toast.info(url); }
  }

  if (!me) return <div className="spinner" />;

  const numero = (n) => Number(n || 0).toLocaleString('es-VE');

  return (
    <>
      <div className="card profile-head perfil-cabecera">
        {me.cover_url ? (
          <div className="profile-cover"><img src={imgUrl(me.cover_url)} alt="" /></div>
        ) : <div className="profile-cover" />}

        <div className="profile-row">
          <Avatar user={me} size="xl" className="ring" />
          <div style={{ paddingBottom: 6, minWidth: 0 }}>
            <h2 style={{ margin: 0, fontSize: 21, display: 'flex', alignItems: 'center', gap: 6 }}>
              {me.display_name || me.username} <VerifiedBadge show={me.is_verified} />
            </h2>
            <div className="muted" style={{ fontSize: 13 }}>
              @{me.username}{me.location ? ` · ${me.location}` : ''}{me.is_private ? ' · privada' : ''}
            </div>
            <span className="insignia-rol" style={{ marginTop: 6 }}>
              {isAdmin ? <><IconShield /> Administrador</> : <><IconUsers /> Miembro</>}
            </span>
          </div>
        </div>

        <div className="profile-info">
          {me.bio ? <p className="profile-bio">{me.bio}</p> : null}

          <div className="profile-meta">
            {me.location ? <span className="dato"><IconMapPin /> {me.location}</span> : null}
            {me.created_at ? <span className="dato"><IconCalendar /> Se unió {timeAgo(me.created_at)}</span> : null}
            {me.link ? (
              <span className="dato"><IconLink />
                <a href={me.link.startsWith('http') ? me.link : `https://${me.link}`} target="_blank" rel="noopener noreferrer">
                  {me.link.replace(/^https?:\/\//, '')}
                </a>
              </span>
            ) : null}
          </div>

          <div className="profile-stats">
            <span><b>{numero(me.posts_count)}</b><span>publicaciones</span></span>
            <span><b>{numero(me.followers_count)}</b><span>seguidores</span></span>
            <span><b>{numero(me.following_count)}</b><span>siguiendo</span></span>
          </div>
        </div>

        <div className="row" style={{ gap: 8, padding: '12px 14px 2px', flexWrap: 'wrap' }}>
          <a className="btn btn-outline btn-sm" href="#/settings"><IconSettings /> Editar perfil</a>
          <button className="btn btn-outline btn-sm" onClick={compartirPerfil}><IconLink /> Compartir</button>
          <a className="btn btn-outline btn-sm" href="#/settings/apariencia">Apariencia</a>
          {isAdmin ? <a className="btn btn-outline btn-sm" href="#/admin"><IconShield /> Administración</a> : null}
        </div>
      </div>

      <div className="tabs">
        {SECCIONES.map((s) => (
          <button key={s.id} className={section === s.id ? 'active' : ''} onClick={() => setSection(s.id)}>
            {s.label}
          </button>
        ))}
      </div>

      {section === 'grupos' ? <MisGrupos /> : null}

      {loading && section !== 'grupos' ? <div className="spinner" /> : null}

      {!loading && section === 'posts' && posts.length === 0 ? (
        <div className="card empty">
          <div className="moon-emoji">🌙</div>
          <h3>Aún no publicaste nada</h3>
          <p>Tu primera publicación aparecerá aquí.</p>
        </div>
      ) : null}

      {!loading && section === 'saved' && saved.length === 0 ? (
        <div className="card empty"><h3>Sin guardados</h3><p>Toca el icono de guardar en cualquier publicación.</p></div>
      ) : null}

      {!loading && section === 'fotos' ? (
        <RejillaFotos posts={posts} comentar={() => {}} />
      ) : null}

      {(section === 'posts' ? posts : section === 'saved' ? saved : []).map((post) => (
        <PostCard key={post.id} post={post} />
      ))}
    </>
  );
}
