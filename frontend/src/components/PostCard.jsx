// Moon — Tarjeta de publicación (feed, perfil, búsqueda, hilo)
// ============================================================
// Like / guardar / comentar / menú (editar, eliminar, reportar) contra el
// backend real, con actualización optimista de los contadores. Todo el
// diálogo pasa por los avisos y diálogos de la aplicación (nada de ventanas
// del navegador).

import React, { useEffect, useRef, useState } from 'react';
import { api, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { timeAgo, linkify } from '../utils.js';
import { toast, confirmar, pedirTexto, avisoError } from '../ui.js';
import Avatar, { VerifiedBadge } from './Avatar.jsx';
import {
  IconHeart, IconBookmark, IconComment, IconMore, IconTrash, IconEdit, IconReport,
  IconLink, IconX, IconGlobe, IconLock, IconSend,
} from './Icons.jsx';

// Caché en memoria de quién reaccionó: evita repetir la misma petición
// cada vez que la publicación vuelve a pintarse (feed, perfil, hilo).
const cacheReacciones = new Map();

function useReacciones(post) {
  const [datos, setDatos] = useState(() => cacheReacciones.get(post.id) || null);
  const cuenta = post.likes_count || 0;
  useEffect(() => {
    if (cuenta <= 0) return undefined;
    const guardado = cacheReacciones.get(post.id);
    if (guardado && guardado.total === cuenta) { setDatos(guardado); return undefined; }
    let vivo = true;
    api.get(`/api/posts/${post.id}/likes`)
      .then((res) => {
        if (!vivo) return;
        cacheReacciones.set(post.id, res);
        setDatos(res);
      })
      .catch(() => {});
    return () => { vivo = false; };
  }, [post.id, cuenta]);
  return datos;
}

/** «A María y 23 más les gusta» — con nombres de verdad, no inventados. */
function PruebaSocial({ post }) {
  const datos = useReacciones(post);
  const total = post.likes_count || 0;
  if (total <= 0) return null;
  const gente = (datos?.items || []).slice(0, 3);
  const primero = gente[0];
  const resto = Math.max(0, total - 1);

  return (
    <div className="prueba-social">
      {gente.length > 0 ? (
        <span className="pila-avatares" aria-hidden="true">
          {gente.map((u) => (
            <Avatar key={u.id} user={u} className="mini" />
          ))}
        </span>
      ) : null}
      <span>
        {primero ? (
          <>
            A <b>{primero.display_name || primero.username}</b>
            {resto > 0 ? <> y <b>{resto}</b> {resto === 1 ? 'persona más' : 'personas más'}</> : null} les gusta
          </>
        ) : (
          <><b>{total}</b> {total === 1 ? 'me gusta' : 'me gusta'}</>
        )}
      </span>
      <span className="corazon" aria-hidden="true"><IconHeart filled /></span>
    </div>
  );
}

function PostMenu({ post, onDelete, onEdit, onReport }) {
  const { user } = useAuth();
  const [abierto, setAbierto] = useState(false);
  const caja = useRef(null);
  const esMio = user && post.is_mine;

  useEffect(() => {
    if (!abierto) return;
    function fuera(e) {
      if (caja.current && !caja.current.contains(e.target)) setAbierto(false);
    }
    function escape(e) { if (e.key === 'Escape') setAbierto(false); }
    document.addEventListener('mousedown', fuera);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', fuera);
      document.removeEventListener('keydown', escape);
    };
  }, [abierto]);

  async function copiarEnlace() {
    setAbierto(false);
    const url = `${window.location.origin}${window.location.pathname}#/post/${post.id}`;
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(url);
        toast.ok('Enlace copiado al portapapeles');
      } else {
        toast.info(url);
      }
    } catch {
      toast.info(url);
    }
  }

  return (
    <div ref={caja} style={{ position: 'relative' }}>
      <button
        className="icon-btn"
        onClick={() => setAbierto((v) => !v)}
        aria-label="Más opciones de la publicación"
        aria-expanded={abierto}
        aria-haspopup="menu"
      >
        <IconMore />
      </button>

      {abierto ? (
        <div className="menu" role="menu">
          {esMio ? (
            <>
              <button role="menuitem" onClick={() => { setAbierto(false); onEdit(); }}>
                <IconEdit /> Editar
              </button>
              <button role="menuitem" className="danger" onClick={() => { setAbierto(false); onDelete(); }}>
                <IconTrash /> Eliminar
              </button>
              <hr />
            </>
          ) : (
            <button role="menuitem" onClick={() => { setAbierto(false); onReport(); }}>
              <IconReport /> Reportar
            </button>
          )}
          <button role="menuitem" onClick={copiarEnlace}>
            <IconLink /> Copiar enlace
          </button>
        </div>
      ) : null}
    </div>
  );
}

export default function PostCard({ post, onChanged, compact = false }) {
  const { refreshMe } = useAuth();
  const [p, setP] = useState(post);
  const [busy, setBusy] = useState(false);
  const [editando, setEditando] = useState(false);
  const [borrador, setBorrador] = useState(p.content || '');
  const [animarLike, setAnimarLike] = useState(false);

  useEffect(() => { setP(post); }, [post]);

  const aplicar = (next) => { setP(next); if (onChanged) onChanged(next); };

  async function alternar(kind) {
    if (busy) return;
    setBusy(true);
    try {
      if (kind === 'like') {
        await api.post(`/api/posts/${p.id}/like`, {});
        aplicar({ ...p, is_liked: true, likes_count: (p.likes_count || 0) + 1 });
        setAnimarLike(true);
        setTimeout(() => setAnimarLike(false), 420);
      } else if (kind === 'unlike') {
        await api.del(`/api/posts/${p.id}/like`);
        aplicar({ ...p, is_liked: false, likes_count: Math.max(0, (p.likes_count || 0) - 1) });
      } else if (kind === 'save') {
        const res = await api.post(`/api/posts/${p.id}/save`, {});
        aplicar({
          ...p,
          is_saved: res.saved,
          saves_count: res.saved ? (p.saves_count || 0) + 1 : Math.max(0, (p.saves_count || 0) - 1),
        });
        toast.ok(res.saved ? 'Guardado en tu colección' : 'Quitado de guardados');
      }
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  async function eliminar() {
    const ok = await confirmar({
      title: '¿Eliminar esta publicación?',
      message: 'Se borrará junto con sus comentarios y reacciones.',
      confirmText: 'Eliminar',
      danger: true,
    });
    if (!ok) return;
    try {
      await api.del(`/api/posts/${p.id}`);
      aplicar({ ...p, deleted: true });
      toast.ok('Publicación eliminada');
      refreshMe();
    } catch (e) {
      avisoError(e);
    }
  }

  async function guardarEdicion() {
    const texto = borrador.trim();
    if (!texto) { toast.err('La publicación no puede quedar vacía'); return; }
    try {
      await api.patch(`/api/posts/${p.id}`, { content: texto });
      aplicar({ ...p, content: texto });
      setEditando(false);
      toast.ok('Publicación actualizada');
    } catch (e) {
      avisoError(e);
    }
  }

  // Compartir de verdad: el menú del sistema si existe, si no el enlace.
  async function compartir() {
    const url = `${window.location.origin}${window.location.pathname}#/post/${p.id}`;
    const texto = (p.content || '').slice(0, 120);
    try {
      if (navigator.share) {
        await navigator.share({ title: 'Publicación de Moon', text: texto, url });
        return;
      }
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(url);
        toast.ok('Enlace copiado. Ya puedes pegarlo donde quieras.');
        return;
      }
      toast.info(url);
    } catch { /* la persona canceló: no pasa nada */ }
  }

  async function reportar() {
    const motivo = await pedirTexto({
      title: 'Reportar publicación',
      label: '¿Por qué la reportas?',
      placeholder: 'Spam, acoso, contenido inapropiado…',
      confirmText: 'Enviar reporte',
    });
    if (!motivo) return;
    try {
      await api.post('/api/reports', { target_type: 'post', target_id: p.id, reason: motivo, detail: '' });
      toast.ok('Reporte enviado. Gracias por cuidar Moon.');
    } catch (e) {
      avisoError(e);
    }
  }

  if (p.deleted) return null;

  const imagenes = p.images || [];

  return (
    <article className="post">
      <header className="post-head">
        <a href={`#/user/${p.author_username}`} aria-label={`Perfil de ${p.author_username}`}>
          <Avatar
            user={{ username: p.author_username, display_name: p.author_display_name, avatar_url: p.author_avatar_url }}
          />
        </a>
        <div className="who">
          <div className="name">
            <a href={`#/user/${p.author_username}`}>{p.author_display_name || p.author_username}</a>
            <VerifiedBadge show={p.author_is_verified} />
            <span className="at">@{p.author_username}</span>
            <span className="at" aria-hidden="true">·</span>
            <a className="at" href={`#/post/${p.id}`} title={p.created_at}>
              {timeAgo(p.created_at)}
            </a>
            {p.edited_at ? <span className="pill">editado</span> : null}
            <span className="quien-meta" title={p.author_is_private ? 'Cuenta privada' : 'Publicación pública'}>
              {p.author_is_private ? <IconLock /> : <IconGlobe />}
            </span>
          </div>
        </div>
        {compact ? null : (
          <PostMenu
            post={p}
            onDelete={eliminar}
            onEdit={() => { setBorrador(p.content || ''); setEditando(true); }}
            onReport={reportar}
          />
        )}
      </header>

      {editando ? (
        <div className="mb" style={{ marginTop: 10 }}>
          <textarea
            className="textarea"
            value={borrador}
            maxLength={2000}
            onChange={(e) => setBorrador(e.target.value)}
            rows={3}
            aria-label="Editar publicación"
          />
          <div className="row mt" style={{ justifyContent: 'flex-end' }}>
            <button className="btn btn-ghost btn-sm" onClick={() => { setEditando(false); setBorrador(p.content); }}>
              Cancelar
            </button>
            <button className="btn btn-primary btn-sm" onClick={guardarEdicion} disabled={!borrador.trim()}>
              Guardar cambios
            </button>
          </div>
        </div>
      ) : (
        <p className="post-body" dangerouslySetInnerHTML={{ __html: linkify(p.content) }} />
      )}

      {imagenes.length > 0 ? (
        <div className={`post-images count-${Math.min(imagenes.length, 4)}`}>
          {imagenes.length > 1 ? (
            <span className="contador-fotos">1 / {imagenes.length}</span>
          ) : null}
          {imagenes.map((img, i) => (
            <img
              key={i}
              src={imgUrl(img.original_url || img.url)}
              alt={`Imagen ${i + 1} de la publicación`}
              loading="lazy"
            />
          ))}
        </div>
      ) : null}

      <PruebaSocial post={p} />

      <footer className="post-actions">
        <button
          className={p.is_liked ? 'liked' : ''}
          onClick={() => alternar(p.is_liked ? 'unlike' : 'like')}
          disabled={busy}
          aria-pressed={!!p.is_liked}
          aria-label={p.is_liked ? 'Quitar me gusta' : 'Me gusta'}
        >
          <IconHeart filled={p.is_liked} className={animarLike ? 'latido' : ''} />
          <span className="etiqueta">{p.is_liked ? 'Te gusta' : 'Me gusta'}</span>
          {p.likes_count > 0 ? <span className="count">{p.likes_count}</span> : null}
        </button>

        <a href={`#/post/${p.id}`} aria-label={`Ver comentarios (${p.comments_count || 0})`}>
          <IconComment />
          <span className="etiqueta">Comentar</span>
          {p.comments_count > 0 ? <span className="count">{p.comments_count}</span> : null}
        </a>

        <button className="compartir" onClick={compartir} aria-label="Compartir publicación">
          <IconSend />
          <span className="etiqueta">Compartir</span>
        </button>

        <button
          className={p.is_saved ? 'saved' : ''}
          onClick={() => alternar('save')}
          disabled={busy}
          aria-pressed={!!p.is_saved}
          aria-label={p.is_saved ? 'Quitar de guardados' : 'Guardar publicación'}
          title={p.is_saved ? 'Quitar de guardados' : 'Guardar'}
        >
          <IconBookmark filled={p.is_saved} />
          <span className="etiqueta">{p.is_saved ? 'Guardado' : 'Guardar'}</span>
        </button>
      </footer>
    </article>
  );
}

export function PostCardSkeleton() {
  return <div className="skeleton-post" aria-hidden="true" />;
}

export function EmptyPosts({ title = 'Aún no hay publicaciones', texto = 'Cuando alguien publique, lo verás aquí.' }) {
  return (
    <div className="empty empty-state">
      <IconX style={{ display: 'none' }} />
      <h3>{title}</h3>
      <p>{texto}</p>
    </div>
  );
}
