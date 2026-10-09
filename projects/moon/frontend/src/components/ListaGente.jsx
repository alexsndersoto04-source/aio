// Moon — Gente en una lista (seguidores y seguidos)
// ============================================================
// Se abre desde los números del perfil: tocas «seguidores» o
// «siguiendo» y ves a las personas una por una, con el botón de seguir
// al lado, sin salir de donde estabas.
//
// La misma pieza sirve para mi perfil y para el de cualquier otro.

import React, { useEffect, useState } from 'react';
import { api } from '../api.js';
import Avatar, { VerifiedBadge } from './Avatar.jsx';
import { avisoError, toast } from '../ui.js';
import { plural } from '../utils.js';

export default function ListaGente({ id, inicial = 'followers', onCerrar }) {
  const [vista, setVista] = useState(inicial);
  const [datos, setDatos] = useState(null);
  const [ocupado, setOcupado] = useState(0);

  useEffect(() => { setVista(inicial); }, [inicial]);

  useEffect(() => {
    let vivo = true;
    setDatos(null);
    api.get(`/api/users/${id}/${vista}`)
      .then((res) => { if (vivo) setDatos(res); })
      .catch((e) => { if (vivo) { avisoError(e); setDatos({ items: [], total: 0 }); } });
    return () => { vivo = false; };
  }, [id, vista]);

  async function seguir(u) {
    setOcupado(u.id);
    try {
      if (u.le_sigo) await api.del(`/api/users/${u.id}/follow`);
      else await api.post(`/api/users/${u.id}/follow`, {});
      setDatos((d) => ({
        ...d,
        items: d.items.map((x) => (x.id === u.id ? { ...x, le_sigo: !x.le_sigo } : x)),
      }));
    } catch (e) { avisoError(e); } finally { setOcupado(0); }
  }

  const items = datos?.items || [];

  return (
    <section className="panel-gente" aria-label="Lista de personas">
      <div className="panel-gente-cabecera">
        <div className="cambiar-lista" role="tablist" aria-label="Qué lista ver">
          {[
            ['followers', 'Seguidores'],
            ['following', 'Siguiendo'],
          ].map(([clave, etiqueta]) => (
            <button
              key={clave}
              role="tab"
              aria-selected={vista === clave}
              className={vista === clave ? 'active' : ''}
              onClick={() => setVista(clave)}
            >
              {etiqueta}
            </button>
          ))}
        </div>
        <span className="muted small">{datos ? plural(datos.total, 'persona') : '…'}</span>
        {onCerrar ? (
          <button type="button" className="icon-btn" aria-label="Cerrar la lista" onClick={onCerrar}>✕</button>
        ) : null}
      </div>

      {!datos ? <div className="spinner" /> : null}

      {datos && items.length === 0 ? (
        <p className="muted small" style={{ margin: '10px 2px 4px' }}>
          {vista === 'followers' ? 'Todavía no tiene seguidores.' : 'Todavía no sigue a nadie.'}
        </p>
      ) : null}

      {items.map((u) => (
        <div className="fila-gente" key={u.id}>
          <a className="quien" href={`#/user/${u.username}`}>
            <Avatar user={u} size="sm" />
            <span className="texto">
              <b>{u.display_name || u.username} <VerifiedBadge show={u.is_verified} /></b>
              <small>@{u.username}{u.is_private ? ' · privada' : ''}</small>
            </span>
          </a>
          {u.soy_yo ? (
            <span className="muted small">Eres tú</span>
          ) : (
            <button
              type="button"
              className={u.le_sigo ? 'btn btn-outline btn-sm' : 'btn btn-sm'}
              disabled={ocupado === u.id}
              onClick={() => seguir(u)}
            >
              {u.le_sigo ? 'Siguiendo' : 'Seguir'}
            </button>
          )}
        </div>
      ))}

      {items.length > 0 ? (
        <button
          type="button"
          className="btn-ghost btn-sm enlace-suave"
          onClick={async () => {
            try {
              const enlace = `${window.location.origin}/#/user/${id}`;
              await navigator.clipboard.writeText(enlace);
              toast.ok('Enlace copiado');
            } catch { toast.info('Comparte el perfil desde su pantalla'); }
          }}
        >
          Compartir este perfil
        </button>
      ) : null}
    </section>
  );
}
