// Moon — Publicación individual + comentarios (tiempo real)
// ============================================================
// Cada comentario tiene ahora lo que antes no tenía: reacción propia,
// fijado (si la publicación es tuya), etiqueta de «Autor» cuando comenta
// quien publicó, y la lista se puede ordenar por mejores o recientes.

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import { toast, pedirTexto, avisoError } from '../ui.js';
import { useAuth } from '../auth.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import PostCard, { REACCIONES } from '../components/PostCard.jsx';
import { timeAgo } from '../utils.js';
import { IconMore, IconPin, IconHeart, IconTrash, IconReport, IconCheck } from '../components/Icons.jsx';
import { useFlotante } from '../flotante.jsx';

/** Un comentario con su reacción, su menú y su etiqueta de autor. */
function CommentRow({ creador, onCambio, onEliminar }) {
  const { user } = useAuth();
  const [c, setC] = useState(creador);
  const [reaccionando, setReaccionando] = useState(false);
  // El selector del comentario también cae siempre dentro de la pantalla.
  const { anclaRef, menu: menuReacciones } = useFlotante(reaccionando);
  const [menu, setMenu] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => { setC(creador); }, [creador]);

  const mios = c.reacciones?.mi || null;
  const esMioElComentario = c.is_mine;
  const puedoFijar = c.puedo_fijar;

  async function reaccionar(tipo) {
    setReaccionando(false);
    if (busy) return;
    setBusy(true);
    try {
      const res = await api.post(`/api/comments/${c.id}/react`, { tipo });
      const next = { ...c, reacciones: res.reacciones };
      setC(next);
      if (onCambio) onCambio(next);
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  async function quitarReaccion() {
    setReaccionando(false);
    if (busy) return;
    setBusy(true);
    try {
      const res = await api.del(`/api/comments/${c.id}/react`);
      const next = { ...c, reacciones: res.reacciones };
      setC(next);
      if (onCambio) onCambio(next);
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  async function fijar() {
    setMenu(false);
    try {
      const res = await api.post(`/api/comments/${c.id}/pin`, {});
      toast.ok(res.fijado ? 'Comentario fijado arriba' : 'Comentario suelto');
      if (onCambio) onCambio({ ...c, pinned: res.fijado });
    } catch (e) {
      avisoError(e);
    }
  }

  async function reportar() {
    setMenu(false);
    const motivo = await pedirTexto({
      title: 'Reportar comentario',
      label: '¿Por qué lo reportas?',
      placeholder: 'Spam, acoso, contenido inapropiado…',
      confirmText: 'Enviar reporte',
    });
    if (!motivo) return;
    api.post('/api/reports', { target_type: 'comment', target_id: c.id, reason: motivo, detail: '' })
      .then(() => toast.ok('Reporte enviado. Gracias por cuidar Moon.'))
      .catch(avisoError);
  }

  return (
    <div className={`comentario ${c.pinned ? 'fijado' : ''}`}>
      <a href={`#/user/${c.username}`} aria-label={`Perfil de ${c.username}`}>
        <Avatar user={{ username: c.username, display_name: c.display_name, avatar_url: c.avatar_url }} size="sm" />
      </a>
      <div className="cuerpo">
        <div className="linea">
          <span className="meta">
            <a className="nombre" href={`#/user/${c.username}`}>{c.display_name}</a>
            <VerifiedBadge show={c.is_verified} />
            {c.es_autor ? <span className="etiqueta-autor">Autor</span> : null}
            {c.pinned ? <span className="etiqueta-fijado" title="Comentario fijado" aria-label="Comentario fijado"><IconPin /></span> : null}
            <span className="muted small">· {timeAgo(c.created_at)}</span>
          </span>
          <button
            type="button"
            ref={anclaRef}
            className={`icon-btn reaccionar-comentario ${(mios || (c.reacciones?.total || 0) > 0) ? 'activo' : ''}`}
            onClick={() => (mios ? quitarReaccion() : reaccionar('me_gusta'))}
            onContextMenu={(e) => { e.preventDefault(); setReaccionando(true); }}
            aria-label={mios ? 'Quitar mi reacción' : 'Reaccionar al comentario'}
            title={mios
              ? `${REACCIONES.find((r) => r.tipo === mios)?.nombre} — toca para quitarla`
              : `${(c.reacciones?.total || 0)} reacciones — mantén pulsado para elegir otra`}
          >
            {(() => {
              const emoji = mios
                ? REACCIONES.find((r) => r.tipo === mios)?.emoji
                : (c.reacciones?.orden?.[0]?.emoji || null);
              return emoji ? <span className="moon-emoji">{emoji}</span> : <IconHeart />;
            })()}
            {(c.reacciones?.total || 0) > 0 ? <b className="conteo">{c.reacciones.total}</b> : null}
          </button>
          <span className="menu-caja">
            <button type="button" className="icon-btn" onClick={() => setMenu((v) => !v)} aria-label="Opciones del comentario" aria-expanded={menu}>
              <IconMore />
            </button>
            {menu ? (
              <>
                <span className="hoja-fondo" role="presentation" onClick={() => setMenu(false)} />
                <span className="menu comentario-menu" role="menu">
                  <button role="menuitem" onClick={() => { setMenu(false); setReaccionando(true); }}>
                    <IconHeart /> Otra reacción
                  </button>
                  {puedoFijar ? (
                    <button role="menuitem" onClick={fijar}>
                      <IconPin /> {c.pinned ? 'Soltar el comentario' : 'Fijar arriba'}
                    </button>
                  ) : null}
                  {esMioElComentario ? (
                    <button role="menuitem" className="danger" onClick={() => { setMenu(false); onEliminar(c); }}>
                      <IconTrash /> Eliminar
                    </button>
                  ) : (
                    <button role="menuitem" onClick={reportar}>
                      <IconReport /> Reportar
                    </button>
                  )}
                </span>
              </>
            ) : null}
          </span>
        </div>
        <p className="texto-comentario">{c.content}</p>

        {reaccionando ? (
          <>
            <span className="hoja-fondo" role="presentation" onClick={() => setReaccionando(false)} />
            {menuReacciones(REACCIONES.map((r) => (
              <button
                key={r.tipo}
                role="menuitem"
                className={mios === r.tipo ? 'activa' : ''}
                onClick={() => reaccionar(r.tipo)}
                title={r.nombre}
                aria-label={r.nombre}
              >
                <span className="moon-emoji">{r.emoji}</span>
              </button>
            )))}
          </>
        ) : null}
      </div>
    </div>
  );
}

export default function PostView({ id }) {
  const { user } = useAuth();
  const [post, setPost] = useState(null);
  const [comments, setComments] = useState([]);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [notFound, setNotFound] = useState(false);
  const [orden, setOrden] = useState('recientes');

  async function load(ordenActual = orden) {
    try {
      const p = await api.get(`/api/posts/${id}`);
      setPost(p);
      const res = await api.get(`/api/posts/${id}/comments?orden=${ordenActual}`);
      setComments(res || []);
    } catch (e) {
      if (e.status === 404) setNotFound(true);
      else avisoError(e);
    }
  }

  useEffect(() => { load(orden); /* eslint-disable-next-line */ }, [id, orden]);

  async function sendComment(e) {
    e.preventDefault();
    if (!draft.trim()) return;
    setBusy(true);
    try {
      const created = await api.post(`/api/posts/${id}/comments`, { content: draft.trim() });
      setComments((prev) => [...prev, created]);
      setDraft('');
      setPost((p) => ({ ...p, comments_count: (p.comments_count || 0) + 1 }));
    } catch (err) {
      avisoError(err);
    } finally {
      setBusy(false);
    }
  }

  async function eliminarComentario(c) {
    try {
      await api.del(`/api/comments/${c.id}`);
      setComments((prev) => prev.filter((x) => x.id !== c.id));
      setPost((p) => ({ ...p, comments_count: Math.max(0, (p.comments_count || 0) - 1) }));
      toast.ok('Comentario eliminado');
    } catch (e) {
      avisoError(e);
    }
  }

  function cambiarComentario(next) {
    setComments((prev) => {
      const lista = prev.map((x) => (x.id === next.id ? { ...x, ...next } : x));
      // Si se fijó o se soltó, se reordena como lo haría el servidor.
      if ('pinned' in next) {
        return [...lista].sort((a, b) => {
          if (!!b.pinned !== !!a.pinned) return b.pinned ? 1 : -1;
          return orden === 'mejores'
            ? (b.reacciones?.total || 0) - (a.reacciones?.total || 0)
            : String(b.created_at).localeCompare(String(a.created_at)) * -1;
        });
      }
      return lista;
    });
  }

  if (notFound) return <div className="card empty"><h3>Publicación no encontrada</h3></div>;
  if (!post) return <div className="spinner" />;

  // El servidor marca cuáles puedo fijar; se recalcula con lo que sé del post.
  const soyAutor = post.is_mine || (user && post.author_username === user.username);
  const lista = comments.map((c) => ({ ...c, puedo_fijar: !!soyAutor }));

  return (
    <>
      <a href="#/feed" className="muted" style={{ display: 'inline-block', marginBottom: 10 }}>← Volver</a>
      <PostCard post={post} onChanged={setPost} />

      <div className="card" style={{ padding: '8px 20px 16px' }}>
        <div className="row comentarios-cabecera">
          <h3 style={{ margin: '10px 0 4px', fontSize: 16, fontWeight: 800 }}>
            Comentarios <span className="muted" style={{ fontWeight: 500 }}>({comments.length})</span>
          </h3>
          <span className="spacer" />
          <div className="orden-comentarios" role="group" aria-label="Ordenar comentarios">
            <button type="button" className={orden === 'mejores' ? 'activo' : ''} onClick={() => setOrden('mejores')}>Mejores</button>
            <button type="button" className={orden === 'recientes' ? 'activo' : ''} onClick={() => setOrden('recientes')}>Recientes</button>
          </div>
        </div>

        <form className="row" style={{ padding: '10px 0', borderBottom: '1px solid var(--line-2)', marginBottom: 6 }} onSubmit={sendComment}>
          <Avatar user={user} size="sm" />
          <input
            className="input"
            style={{ border: 'none', boxShadow: 'none', background: 'var(--surface-2)', borderRadius: 999, padding: '9px 16px' }}
            placeholder="Escribe un comentario… escribe @ para nombrar a alguien"
            value={draft}
            maxLength={600}
            onChange={(e) => setDraft(e.target.value)}
          />
          <button className="btn btn-sm" disabled={busy || !draft.trim()}>
            {busy ? <IconCheck /> : 'Enviar'}
          </button>
        </form>

        {comments.length === 0 ? (
          <p className="muted" style={{ padding: '14px 0' }}>Sé el primero en comentar.</p>
        ) : (
          lista.map((c) => (
            <CommentRow
              key={c.id}
              creador={c}
              onCambio={cambiarComentario}
              onEliminar={eliminarComentario}
            />
          ))
        )}
      </div>
    </>
  );
}
