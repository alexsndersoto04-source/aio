// Moon — Sugerencias de personas (arranque en frío de la comunidad)
// ============================================================
// Una red social vacía se siente muerta: cuando el feed o el explorador no
// tienen nada que mostrar, proponemos personas reales a las que seguir, con
// su botón de seguir funcionando contra la API. Nada simulado.

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import { plural } from '../utils.js';
import { avisoError } from '../ui.js';
import Avatar, { VerifiedBadge } from './Avatar.jsx';

export function FilaSugerencia({ u, onFollowed }) {
  const [siguiendo, setSiguiendo] = useState(false);
  const [busy, setBusy] = useState(false);

  async function alternar() {
    if (busy) return;
    setBusy(true);
    try {
      if (siguiendo) {
        await api.del(`/api/users/${u.id}/follow`);
        setSiguiendo(false);
      } else {
        await api.post(`/api/users/${u.id}/follow`, {});
        setSiguiendo(true);
      }
      if (onFollowed) onFollowed(u.id, siguiendo);
    } catch (e) {
      avisoError(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="card suggest">
      <a href={`#/user/${u.username}`} aria-label={`Perfil de ${u.display_name || u.username}`}>
        <Avatar user={u} size="md" />
      </a>
      <span className="who">
        <span className="name">
          <a href={`#/user/${u.username}`}>{u.display_name || u.username}</a>
          <VerifiedBadge show={u.is_verified} />
        </span>
        <span className="at">@{u.username} · {plural(u.followers_count || 0, 'seguidor')}</span>
      </span>
      <button
        type="button"
        className={siguiendo ? 'btn btn-outline btn-sm' : 'btn btn-aurora btn-sm'}
        onClick={alternar}
        disabled={busy}
        aria-pressed={siguiendo}
      >
        {siguiendo ? 'Siguiendo' : 'Seguir'}
      </button>
    </div>
  );
}

/** Lista de personas sugeridas; `null` mientras carga, `[]` si no hay nadie. */
export function useSugerencias() {
  const [lista, setLista] = useState(null);
  useEffect(() => {
    let vivo = true;
    api.get('/api/users/suggestions')
      .then((res) => { if (vivo) setLista(Array.isArray(res) ? res : res?.items || []); })
      .catch(() => { if (vivo) setLista([]); });
    return () => { vivo = false; };
  }, []);
  return [lista, setLista];
}

export function SugerenciasPersonas({ titulo = 'Personas que podrías seguir', limite = 3 }) {
  const [lista] = useSugerencias();
  if (lista === null) return <div className="spinner" style={{ margin: '18px auto' }} />;
  if (lista.length === 0) return null;
  return (
    <section aria-label={titulo} className="bloque-sugerencias">
      <h2 className="titulo-bloque">{titulo}</h2>
      {lista.slice(0, limite).map((u) => (
        <FilaSugerencia key={u.id} u={u} />
      ))}
    </section>
  );
}
