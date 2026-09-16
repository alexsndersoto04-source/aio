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
  IconLink, IconX, IconGlobe, IconLock, IconSend, IconPin, IconEyeOff, IconCheck,
} from './Icons.jsx';

// Las reacciones que se pueden poner: mismas claves que entiende el servidor.
export const REACCIONES = [
  { tipo: 'me_gusta', emoji: '👍', nombre: 'Me gusta' },
  { tipo: 'me_encanta', emoji: '❤️', nombre: 'Me encanta' },
  { tipo: 'risa', emoji: '😂', nombre: 'Me da risa' },
  { tipo: 'sorpresa', emoji: '😮', nombre: 'Me sorprende' },
  { tipo: 'triste', emoji: '😢', nombre: 'Me entristece' },
  { tipo: 'enojo', emoji: '😠', nombre: 'Me molesta' },
];

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

/** Cuerpo del post: los textos largos se recortan con «Ver más» en vez de
 *  quedar amputados en mitad de la tarjeta. */
function CuerpoPost({ contenido }) {
  const [expandido, setExpandido] = useState(false);
  const texto = contenido || '';
  const necesita = texto.length > 300 || texto.split('\n').length > 4;
  return (
    <div className="post-body-caja">
      <p
        className={`post-body${necesita && !expandido ? ' recortado' : ''}`}
        dangerouslySetInnerHTML={{ __html: linkify(texto) }}
      />
      {necesita ? (
        <button
          type="button"
          className="ver-mas"
          onClick={() => setExpandido((v) => !v)}
          aria-expanded={expandido}
        >
          {expandido ? 'Mostrar menos' : 'Ver más'}
        </button>
      ) : null}
    </div>
  );
}

/** Encuesta dentro de una publicación: se vota y se ven los resultados. */
function Encuesta({ post, onCambio }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [votantes, setVotantes] = useState(null);
  const p = post.poll;
  if (!p || p === true) return null;

  const votado = (p.mi_voto || []).length > 0;
  const mostrarResultados = votado || p.cerrada;
  const misVotos = p.mi_voto || [];

  /** Quién votó: solo lo ve quien creó la encuesta (el servidor lo verifica). */
  async function verQuienVoto() {
    if (votantes) { setVotantes(null); return; }
    try {
      setVotantes(await api.get(`/api/posts/${post.id}/poll/votos`));
    } catch (e) {
      avisoError(e);
    }
  }

  async function votar(i) {
    if (busy || p.cerrada) return;
    setBusy(true);
    setError('');
    try {
      const res = await api.post(`/api/posts/${post.id}/vote`, { opcion: i });
      if (onCambio) onCambio(res);
    } catch (e) {
      setError(e.message || 'No se pudo votar');
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="encuesta" role="group" aria-label="Encuesta">
      {p.pregunta ? <p className="pregunta">{p.pregunta}</p> : null}
      <div className="opciones">
        {p.opciones.map((o, i) => {
          const marcada = misVotos.includes(i);
          return (
            <button
              key={i}
              type="button"
              className={`opcion${marcada ? ' marcada' : ''}`}
              onClick={() => votar(i)}
              disabled={busy || p.cerrada}
              aria-pressed={marcada}
            >
              {mostrarResultados ? <span className="barra" style={{ width: `${o.porcentaje}%` }} aria-hidden="true" /> : null}
              <span className="texto">{o.texto}</span>
              {mostrarResultados ? <span className="porcentaje">{o.porcentaje}%</span> : null}
              {marcada ? <span className="hecho">✓</span> : null}
            </button>
          );
        })}
      </div>
      <div className="pie-encuesta">
        <span>{p.total} {p.total === 1 ? 'voto' : 'votos'}</span>
        <span>·</span>
        <span>{p.cerrada ? 'Encuesta cerrada' : votado ? 'Ya votaste' : 'Toca para votar'}</span>
        {p.multiple ? <span className="pastilla">varias respuestas</span> : null}
        {p.cierra ? <span className="pastilla">{p.cierra}</span> : null}
        {post.is_mine ? (
          <button type="button" className="btn-ghost btn-sm ver-votantes" onClick={verQuienVoto} aria-expanded={!!votantes}>
            {votantes ? 'Ocultar los votos' : 'Ver quién votó'}
          </button>
        ) : null}
      </div>
      {votantes ? (
        <div className="quien-voto">
          {votantes.opciones.map((o) => (
            <div key={o.indice} className="linea-voto">
              <b>{o.texto}</b>
              <span className="muted small">{o.votos} {o.votos === 1 ? 'voto' : 'votos'}</span>
              {o.personas.length > 0 ? (
                <span className="personas">
                  {o.personas.slice(0, 8).map((u) => (
                    <a key={u.id} href={`#/user/${u.username}`} title={u.display_name}>
                      <Avatar user={u} className="mini" size="sm" />
                    </a>
                  ))}
                  {o.personas.length > 8 ? <span className="muted small">+{o.personas.length - 8}</span> : null}
                </span>
              ) : <span className="muted small">nadie todavía</span>}
            </div>
          ))}
        </div>
      ) : null}
      {error ? <p className="error-encuesta">{error}</p> : null}
    </div>
  );
}

function PostMenu({ post, onDelete, onEdit, onReport, onPin, onNoInteresa, onReaccionar, onEnviarA }) {
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
              <button role="menuitem" onClick={() => { setAbierto(false); onReaccionar(); }}>
                <IconHeart /> Elegir reacción
              </button>
              <button role="menuitem" onClick={() => { setAbierto(false); onPin(); }}>
                <IconPin /> {post.pinned ? 'Quitar de fijadas' : 'Fijar en mi perfil'}
              </button>
              <button role="menuitem" onClick={() => { setAbierto(false); onEdit(); }}>
                <IconEdit /> Editar
              </button>
              <button role="menuitem" className="danger" onClick={() => { setAbierto(false); onDelete(); }}>
                <IconTrash /> Eliminar
              </button>
              <hr />
            </>
          ) : (
            <>
              <button role="menuitem" onClick={() => { setAbierto(false); onReaccionar(); }}>
                <IconHeart /> Elegir reacción
              </button>
              <button role="menuitem" onClick={() => { setAbierto(false); onNoInteresa(); }}>
                <IconEyeOff /> No me interesa
              </button>
              <button role="menuitem" onClick={() => { setAbierto(false); onReport(); }}>
                <IconReport /> Reportar
              </button>
              <hr />
            </>
          )}
          <button role="menuitem" onClick={() => { setAbierto(false); onEnviarA(); }}>
            <IconSend /> Enviar a un chat
          </button>
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
  // Selector de reacciones variadas (se abre al mantener pulsado o con el botón).
  const [reaccionando, setReaccionando] = useState(false);
  const [oculto, setOculto] = useState(false);
  // Compartir al chat: se elige la conversación en una hoja.
  const [enviandoA, setEnviandoA] = useState(false);
  const [convs, setConvs] = useState(null);
  const reacciones = p.reacciones || null;
  const cajaReaccion = useRef(null);

  // Si el selector se abre desde el menú ⋯ (que está arriba), la barra de
  // acciones queda fuera de la pantalla: se baja a la vista para que el
  // selector aparezca donde la persona está mirando.
  useEffect(() => {
    if (!reaccionando) return;
    cajaReaccion.current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [reaccionando]);

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

  /** Poner (o cambiar) la reacción: una sola por persona. */
  async function reaccionar(tipo) {
    setReaccionando(false);
    if (busy) return;
    setBusy(true);
    try {
      const res = await api.post(`/api/posts/${p.id}/react`, { tipo });
      aplicar(res);
      if (tipo === 'me_gusta') {
        setAnimarLike(true);
        setTimeout(() => setAnimarLike(false), 420);
      }
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  /** Quitar mi reacción. */
  async function quitarReaccion() {
    setReaccionando(false);
    if (busy) return;
    setBusy(true);
    try {
      aplicar(await api.del(`/api/posts/${p.id}/react`));
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  /** Fijar (o soltar) la publicación: aparece arriba en tu perfil. */
  async function fijar() {
    try {
      const res = await api.post(`/api/posts/${p.id}/pin`, {});
      aplicar(res);
      toast.ok(res.pinned ? 'Fijada: se verá arriba en tu perfil' : 'Ya no está fijada');
    } catch (e) {
      avisoError(e);
    }
  }

  /** «No me interesa»: desaparece de tu inicio sin bloquear a nadie. */
  async function noInteresa() {
    try {
      await api.post(`/api/posts/${p.id}/interesa`, { no: true });
      setOculto(true);
      toast.ok('Listo: te mostraremos menos de esto');
    } catch (e) {
      avisoError(e);
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

  /** Abre la lista de conversaciones para mandarle esta publicación. */
  async function abrirEnviarA() {
    setEnviandoA(true);
    if (convs) return;
    try {
      setConvs(await api.get('/api/messages/conversations') || []);
    } catch (e) {
      avisoError(e);
      setConvs([]);
    }
  }

  /** Manda la publicación como tarjeta al chat elegido. */
  async function enviarA(conv) {
    try {
      await api.post(`/api/messages/conversations/${conv.id}/messages`, { post_id: p.id });
      setEnviandoA(false);
      toast.ok(`Enviado a ${conv.display_name || conv.username}`);
    } catch (e) {
      avisoError(e);
    }
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

  if (p.deleted || oculto) return null;

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
            {p.pinned ? <span className="pill pill-fijada"><IconPin /> Fijada</span> : null}
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
            onPin={fijar}
            onNoInteresa={noInteresa}
            onReaccionar={() => setReaccionando(true)}
            onEnviarA={abrirEnviarA}
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
        <CuerpoPost contenido={p.content} />
      )}

      <Encuesta post={p} onCambio={aplicar} />

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

      {reacciones && reacciones.total > 0 ? (
        <div className="resumen-reacciones">
          <span className="emojis" aria-hidden="true">
            {reacciones.orden.slice(0, 3).map((r) => (
              <span key={r.tipo} className="moon-emoji">{r.emoji}</span>
            ))}
          </span>
          <span className="cuenta">
            {reacciones.total === 1 ? '1 reacción' : `${reacciones.total} reacciones`}
            {reacciones.total > 3 ? ` · ${reacciones.orden.slice(0, 3).map((r) => r.emoji).join(' ')}` : ''}
          </span>
        </div>
      ) : null}

      <PruebaSocial post={p} />

      <footer className="post-actions">
        <span className="reaccion-caja" ref={cajaReaccion}>
          <button
            className={p.is_liked ? 'liked' : ''}
            onClick={() => (p.is_liked ? quitarReaccion() : reaccionar('me_gusta'))}
            onContextMenu={(e) => { e.preventDefault(); setReaccionando(true); }}
            disabled={busy}
            aria-pressed={!!p.is_liked}
            aria-label={p.is_liked ? 'Quitar mi reacción' : 'Reaccionar'}
          >
            {reacciones?.mi ? (
              <span className="moon-emoji reaccion-mia" aria-hidden="true">{REACCIONES.find((r) => r.tipo === reacciones.mi)?.emoji || '👍'}</span>
            ) : (
              <IconHeart filled={p.is_liked} className={animarLike ? 'latido' : ''} />
            )}
            <span className="etiqueta">
              {reacciones?.mi ? REACCIONES.find((r) => r.tipo === reacciones.mi)?.nombre : 'Reaccionar'}
            </span>
            {p.likes_count > 0 ? <span className="count">{p.likes_count}</span> : null}
          </button>
          {reaccionando ? (
            <>
              <span className="hoja-fondo" role="presentation" onClick={() => setReaccionando(false)} />
              <span className="reacciones-menu" role="menu" aria-label="Reacciones">
                {REACCIONES.map((r) => (
                  <button
                    key={r.tipo}
                    role="menuitem"
                    className={reacciones?.mi === r.tipo ? 'activa' : ''}
                    onClick={() => reaccionar(r.tipo)}
                    title={r.nombre}
                    aria-label={r.nombre}
                  >
                    <span className="moon-emoji">{r.emoji}</span>
                  </button>
                ))}
              </span>
            </>
          ) : null}
        </span>

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

      {enviandoA ? (
        <>
          <span className="hoja-fondo" role="presentation" onClick={() => setEnviandoA(false)} />
          <div className="hoja-reenviar" role="dialog" aria-label="Enviar la publicación a un chat">
            <div className="cabecera">
              <b>Enviar a un chat</b>
              <button type="button" className="icon-btn" onClick={() => setEnviandoA(false)} aria-label="Cerrar">
                <IconX />
              </button>
            </div>
            <div className="vista-previa">
              <span className="ellipsis">{p.content || 'Publicación con fotos'}</span>
            </div>
            <div className="lista">
              {convs === null ? <p className="muted small" style={{ padding: '14px 4px' }}>Cargando tus chats…</p> : null}
              {convs && convs.map((c) => (
                <button type="button" key={c.id} onClick={() => enviarA(c)}>
                  <Avatar user={c} size="sm" />
                  <span className="quien">
                    <b>{c.display_name || c.username}</b>
                    <span className="muted small ellipsis">{c.last_message || `@${c.username}`}</span>
                  </span>
                  <IconSend />
                </button>
              ))}
              {convs && convs.length === 0 ? (
                <p className="muted small" style={{ padding: '14px 4px' }}>
                  Todavía no tienes conversaciones. Abre una desde el perfil de alguien.
                </p>
              ) : null}
            </div>
          </div>
        </>
      ) : null}
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
