// Moon — Mi perfil (portada, números, fotos, grupos y guardados)
// ============================================================
// Todo lo que se ve sale de la base de datos: el perfil, los números, las
// publicaciones, la rejilla de fotos, mis grupos y lo que guardé.

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { api, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { miembroDesde } from '../utils.js';
import { toast, avisoError } from '../ui.js';
import { timeAgo } from '../utils.js';
import ListaGente from '../components/ListaGente.jsx';
import {
  IconBookmark, IconGrid, IconLayers, IconSettings, IconShield, IconLink,
  IconMapPin, IconCalendar, IconHeart, IconUsers, IconComment, IconTrend,
  IconLock, IconCheck, IconX,
} from '../components/Icons.jsx';

const SECCIONES = [
  { id: 'posts', label: 'Publicaciones', icono: <IconGrid /> },
  { id: 'fotos', label: 'Fotos', icono: <IconGrid /> },
  { id: 'me-gusta', label: 'Mis me gusta', icono: <IconHeart /> },
  { id: 'comentarios', label: 'Mis comentarios', icono: <IconComment /> },
  { id: 'etiquetas', label: 'Etiquetas', icono: <IconTrend /> },
  { id: 'grupos', label: 'Grupos', icono: <IconLayers /> },
  { id: 'saved', label: 'Guardados', icono: <IconBookmark /> },
];

/** Personas que quieren seguirme (cuenta privada) o esperan mi permiso. */
function Solicitudes({ pendientes, enviadas, onResolver, onRetirar }) {
  if (!pendientes.length && !enviadas.length) return null;
  return (
    <div className="solicitudes">
      <div className="cabecera"><IconLock /> {pendientes.length
        ? `${pendientes.length === 1 ? 'Una persona quiere' : `${pendientes.length} personas quieren`} seguirte`
        : 'Esperando permiso'}</div>
      {pendientes.map((s) => (
        <div className="solicitud" key={s.solicitud_id}>
          <div className="quien">
            <Avatar user={s} size="sm" />
            <span className="texto">
              <b>{s.display_name || s.username}</b>
              <small>@{s.username} · {timeAgo(s.created_at)}</small>
            </span>
          </div>
          <div className="acciones">
            <button className="btn btn-sm" onClick={() => onResolver(s, true)}><IconCheck /> Aceptar</button>
            <button className="btn btn-outline btn-sm" onClick={() => onResolver(s, false)}><IconX /> Rechazar</button>
          </div>
        </div>
      ))}
      {enviadas.map((u) => (
        <div className="solicitud" key={`env-${u.id}`}>
          <div className="quien">
            <Avatar user={u} size="sm" />
            <span className="texto">
              <b>{u.display_name || u.username}</b>
              <small>Le pediste seguir su cuenta privada · {timeAgo(u.created_at)}</small>
            </span>
          </div>
          <div className="acciones">
            <button className="btn btn-outline btn-sm" onClick={() => onRetirar(u)}>Retirar</button>
          </div>
        </div>
      ))}
    </div>
  );
}

/** Mis comentarios: cada uno lleva a la publicación donde está. */
function MisComentarios({ items }) {
  if (items.length === 0) {
    return (
      <div className="card empty">
        <div className="moon-emoji">💬</div>
        <h3>Todavía no comentaste nada</h3>
        <p>Cuando comentes una publicación aparecerá aquí, con su enlace.</p>
      </div>
    );
  }
  return (
    <div className="lista-mis-cosas">
      {items.map((c) => (
        <a className="fila-mia" key={c.id} href={`#/post/${c.post_id}`}>
          <Avatar user={{ username: c.post_autor, display_name: c.post_display_name, avatar_url: c.post_avatar_url }} size="sm" />
          <span className="texto">
            <span className="mia">{c.content}</span>
            <small>En la publicación de @{c.post_autor || 'alguien'} · {timeAgo(c.created_at)}</small>
          </span>
        </a>
      ))}
    </div>
  );
}

/** Etiquetas que sigo, con la opción de dejarlas. */
function MisEtiquetas({ etiquetas, posts, onDejar }) {
  if (etiquetas.length === 0) {
    return (
      <div className="card empty">
        <div className="moon-emoji">#️⃣</div>
        <h3>No sigues ninguna etiqueta</h3>
        <p>En Explorar puedes seguir #temas y verlos juntos aquí.</p>
        <a className="btn" href="#/explore">Explorar etiquetas</a>
      </div>
    );
  }
  return (
    <>
      <div className="chips-filtro chips-etiquetas" role="group" aria-label="Etiquetas que sigo">
        {etiquetas.map((h) => (
          <button key={h.tag} type="button" className="activo" title="Dejar de seguir" onClick={() => onDejar(h.tag)}>
            #{h.tag} <IconX />
          </button>
        ))}
      </div>
      {posts.length === 0 ? (
        <p className="muted small" style={{ margin: '4px 2px 10px' }}>
          Ninguna publicación usa todavía tus etiquetas.
        </p>
      ) : null}
      {posts.map((post) => <PostCard key={post.id} post={post} />)}
    </>
  );
}

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
  const [section, setSection] = useState(SECCIONES.some((x) => x.id === tab) ? tab : 'posts');
  const [gente, setGente] = useState(null); // 'followers' | 'following' | null
  const [likes, setLikes] = useState([]);
  const [mios, setMios] = useState([]);
  const [etiquetas, setEtiquetas] = useState([]);
  const [postsEtiquetas, setPostsEtiquetas] = useState([]);
  const [solicitudes, setSolicitudes] = useState([]);
  const [enviadas, setEnviadas] = useState([]);

  useEffect(() => {
    api.get('/api/auth/me').then(setMe).catch(() => {});
    refreshMe();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Lo que espera mi permiso y lo que espera el de otros: siempre a la vista.
  const cargarSolicitudes = useCallback(() => {
    api.get('/api/me/solicitudes').then((r) => setSolicitudes(r?.solicitudes || [])).catch(() => setSolicitudes([]));
    api.get('/api/me/enviadas').then((r) => setEnviadas(r?.enviadas || [])).catch(() => setEnviadas([]));
  }, []);

  useEffect(() => { cargarSolicitudes(); }, [cargarSolicitudes]);

  async function resolver(s, aceptar) {
    setSolicitudes((l) => l.filter((x) => x.solicitud_id !== s.solicitud_id));
    try {
      await api.post(`/api/me/solicitudes/${s.solicitud_id}`, { aceptar });
      toast.ok(aceptar ? `${s.display_name || s.username} ya te sigue` : 'Solicitud rechazada');
      api.get('/api/auth/me').then(setMe).catch(() => {});
    } catch (e) { avisoError(e); cargarSolicitudes(); }
  }

  async function retirar(u) {
    setEnviadas((l) => l.filter((x) => x.id !== u.id));
    try {
      await api.del(`/api/users/${u.id}/follow`);
      toast.ok(`Retiraste la solicitud a ${u.display_name || u.username}`);
    } catch (e) { avisoError(e); cargarSolicitudes(); }
  }

  async function dejarEtiqueta(tag) {
    setEtiquetas((l) => l.filter((h) => h.tag !== tag));
    try { await api.del(`/api/hashtags/${tag}/follow`); toast.info(`Dejaste de seguir #${tag}`); }
    catch (e) { avisoError(e); }
  }

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
    } else if (section === 'me-gusta') {
      api.get('/api/me/likes?page=1&limit=20')
        .then((res) => setLikes(res?.items || []))
        .catch(() => {})
        .finally(() => setLoading(false));
    } else if (section === 'comentarios') {
      api.get('/api/me/comments?page=1&limit=30')
        .then((res) => setMios(res?.items || []))
        .catch(() => {})
        .finally(() => setLoading(false));
    } else if (section === 'etiquetas') {
      Promise.all([
        api.get('/api/me/hashtags').catch(() => ({ hashtags: [] })),
        api.get('/api/feed/etiquetas').catch(() => ({ items: [] })),
      ]).then(([h, p]) => {
        setEtiquetas(h?.hashtags || []);
        setPostsEtiquetas(p?.items || []);
      }).finally(() => setLoading(false));
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
            {me.created_at ? <span className="dato"><IconCalendar /> Se unió en {miembroDesde(me.created_at)}</span> : null}
          </div>

          <div className="profile-stats">
            <span><b>{numero(me.posts_count)}</b><span>{me.posts_count === 1 ? 'publicación' : 'publicaciones'}</span></span>
            <button
              type="button"
              className={`stat-pulsable ${gente === 'followers' ? 'activa' : ''}`}
              aria-expanded={gente === 'followers'}
              onClick={() => setGente(gente === 'followers' ? null : 'followers')}
            >
              <b>{numero(me.followers_count)}</b>
              <span>{me.followers_count === 1 ? 'seguidor' : 'seguidores'}</span>
            </button>
            <button
              type="button"
              className={`stat-pulsable ${gente === 'following' ? 'activa' : ''}`}
              aria-expanded={gente === 'following'}
              onClick={() => setGente(gente === 'following' ? null : 'following')}
            >
              <b>{numero(me.following_count)}</b><span>siguiendo</span>
            </button>
          </div>
        </div>

        <div className="row" style={{ gap: 8, padding: '12px 14px 2px', flexWrap: 'wrap' }}>
          <a className="btn btn-outline btn-sm" href="#/settings"><IconSettings /> Editar perfil</a>
          <button className="btn btn-outline btn-sm" onClick={compartirPerfil}><IconLink /> Compartir</button>
          <button
            className="btn btn-outline btn-sm"
            onClick={async () => {
              const enlace = `${window.location.origin}/#/user/${me?.username || me?.id || ''}`;
              try {
                await navigator.clipboard.writeText(enlace);
                toast.ok('Enlace copiado');
              } catch {
                toast.info(enlace);
              }
            }}
          >
            <IconLink /> Copiar enlace
          </button>
          <a className="btn btn-outline btn-sm" href="#/settings/apariencia">Apariencia</a>
          {isAdmin ? <a className="btn btn-outline btn-sm" href="#/admin"><IconShield /> Administración</a> : null}
        </div>

        <div className="acciones-perfil-extra">
          {me.is_private ? (
            <span className="muted small"><IconLock /> Tu cuenta es privada: quien quiera seguirte te lo pide.</span>
          ) : (
            <a className="enlace-suave" href="#/settings/privacidad">Haz tu cuenta privada</a>
          )}
        </div>
      </div>

      <Solicitudes pendientes={solicitudes} enviadas={enviadas} onResolver={resolver} onRetirar={retirar} />

      {gente ? (
        <ListaGente id={me.id} inicial={gente} onCerrar={() => setGente(null)} />
      ) : null}

      <div className="tabs tabs-perfil">
        {SECCIONES.map((s) => (
          <button
            key={s.id}
            className={section === s.id ? 'active' : ''}
            onClick={() => { setSection(s.id); setGente(null); }}
          >
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

      {loading && (section === 'me-gusta' || section === 'comentarios' || section === 'etiquetas') ? (
        <div className="spinner" />
      ) : null}

      {!loading && section === 'me-gusta' ? (
        likes.length === 0 ? (
          <div className="card empty">
            <div className="moon-emoji">❤️</div>
            <h3>Sin me gusta todavía</h3>
            <p>Lo que te guste aparece aquí para volver a encontrarlo.</p>
          </div>
        ) : (
          <>
            <p className="titulo-pequeno"><IconHeart /> {likes.length} publicaciones que te gustaron</p>
            {likes.map((post) => <PostCard key={post.id} post={post} />)}
          </>
        )
      ) : null}

      {!loading && section === 'comentarios' ? <MisComentarios items={mios} /> : null}

      {!loading && section === 'etiquetas' ? (
        <MisEtiquetas etiquetas={etiquetas} posts={postsEtiquetas} onDejar={dejarEtiqueta} />
      ) : null}

      {(section === 'posts' ? posts : section === 'saved' ? saved : []).map((post) => (
        <PostCard key={post.id} post={post} />
      ))}
    </>
  );
}
