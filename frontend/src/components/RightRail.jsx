// Moon — Barra lateral derecha (escritorio)
// ============================================================
// Tres bloques con datos reales del backend:
//   · buscador (lleva a #/explore?q=…)
//   · tendencias (#/explore, hashtags con más publicaciones)
//   · a quién seguir (/api/users/suggestions, con seguir al instante)
// Si una llamada falla, el bloque se oculta sin ruido: la barra nunca
// debe romper la lectura del contenido.

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError } from '../ui.js';
import Avatar, { VerifiedBadge } from './Avatar.jsx';
import {
  IconSearch, IconTrend, IconUsers, IconPlus, IconCheck, IconAt,
} from './Icons.jsx';

function numero(n) {
  const v = Number(n || 0);
  if (v >= 1000000) return (v / 1000000).toFixed(1).replace('.0', '') + ' M';
  if (v >= 1000) return (v / 1000).toFixed(1).replace('.0', '') + ' mil';
  return String(v);
}

function Sugerencia({ usuario, onSeguido }) {
  const [siguiendo, setSiguiendo] = useState(false);
  const [busy, setBusy] = useState(false);

  async function seguir() {
    if (busy || siguiendo) return;
    setBusy(true);
    try {
      await api.post(`/api/users/${usuario.id}/follow`, {});
      setSiguiendo(true);
      toast.ok(`Sigues a @${usuario.username}`);
      if (onSeguido) onSeguido(usuario.id);
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="user-suggest">
      <a href={`#/user/${usuario.username}`} aria-label={`Perfil de ${usuario.username}`}>
        <Avatar user={usuario} size="sm" />
      </a>
      <div className="body">
        <a href={`#/user/${usuario.username}`}>
          <b className="ellipsis">
            {usuario.display_name || usuario.username}
            <VerifiedBadge show={usuario.is_verified} />
          </b>
          <span className="ellipsis">
            <IconAt style={{ width: 11, height: 11, verticalAlign: -1 }} />
            {usuario.username} · {numero(usuario.followers_count)} seguidores
          </span>
        </a>
      </div>
      <button
        className={`btn btn-sm ${siguiendo ? 'btn-outline' : 'btn-primary'}`}
        onClick={seguir}
        disabled={busy || siguiendo}
        aria-label={siguiendo ? `Ya sigues a ${usuario.username}` : `Seguir a ${usuario.username}`}
      >
        {siguiendo ? <IconCheck /> : <IconPlus />}
        <span>{siguiendo ? 'Siguiendo' : 'Seguir'}</span>
      </button>
    </div>
  );
}

export default function RightRail() {
  const { user } = useAuth();
  const [q, setQ] = useState('');
  const [tendencias, setTendencias] = useState(null);
  const [sugerencias, setSugerencias] = useState(null);

  useEffect(() => {
    if (!user) return;
    let vivo = true;
    api.get('/api/hashtags')
      .then((r) => { if (vivo) setTendencias(Array.isArray(r) ? r.slice(0, 6) : []); })
      .catch(() => { if (vivo) setTendencias([]); });
    api.get('/api/users/suggestions')
      .then((r) => { if (vivo) setSugerencias(Array.isArray(r) ? r.slice(0, 3) : []); })
      .catch(() => { if (vivo) setSugerencias([]); });
    return () => { vivo = false; };
  }, [user]);

  function buscar(e) {
    e.preventDefault();
    const texto = q.trim();
    if (!texto) return;
    window.location.hash = `#/explore?q=${encodeURIComponent(texto)}&type=users`;
  }

  if (!user) return null;

  return (
    <aside className="rail" aria-label="Descubrir">
      <form className="rail-search" onSubmit={buscar} role="search">
        <IconSearch />
        <input
          className="input"
          type="search"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Buscar en Moon"
          aria-label="Buscar en Moon"
        />
      </form>

      {tendencias && tendencias.length > 0 ? (
        <section className="rail-card">
          <h3><IconTrend /> Tendencias</h3>
          {tendencias.map((t, i) => (
            <a className="trend" key={t.tag} href={`#/explore?q=${encodeURIComponent(t.tag)}&type=posts`}>
              <span className="rank">{i + 1}</span>
              <span className="body">
                <b className="ellipsis">#{t.tag}</b>
                <small>{numero(t.posts_count)} publicaciones</small>
              </span>
            </a>
          ))}
          <div className="foot"><a href="#/explore">Ver todo en Explorar</a></div>
        </section>
      ) : null}

      {sugerencias && sugerencias.length > 0 ? (
        <section className="rail-card">
          <h3><IconUsers /> A quién seguir</h3>
          {sugerencias.map((u) => (
            <Sugerencia
              key={u.id}
              usuario={u}
              onSeguido={(id) => setSugerencias((lista) => lista.filter((x) => x.id !== id))}
            />
          ))}
          <div className="foot"><a href="#/explore">Descubrir más personas</a></div>
        </section>
      ) : null}

      <div className="legal">
        <span>© {new Date().getFullYear()} Moon</span>
        <a href="#/settings">Privacidad</a>
        <a href="#/settings">Términos</a>
        <a href="#/settings">Ayuda</a>
        <a href="#/settings">Estado</a>
      </div>
    </aside>
  );
}
