// Moon — Grupos
// ============================================================
// Espacios con nombre donde la gente se reúne: los míos arriba, los que
// puedo descubrir debajo, buscador por nombre y creación en un diálogo.
// Todo real: cada grupo y cada miembro viven en la base de datos.

import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../api.js';
import { toast, avisoError, pedirTexto } from '../ui.js';
import { IconUsers, IconPlus, IconSearch, IconLayers } from '../components/Icons.jsx';
import { ListSkeleton } from '../components/Skeleton.jsx';

function Tarjeta({ grupo, alEntrar }) {
  return (
    <article className="tarjeta-grupo">
      <a className="cubierta" href={`#/grupo/${grupo.id}`} aria-label={grupo.name}>
        {grupo.cover_url ? (
          <img src={grupo.cover_url} alt="" />
        ) : (
          <span className="cubierta-vacia" aria-hidden="true"><IconUsers /></span>
        )}
      </a>
      <div className="cuerpo">
        <a className="nombre" href={`#/grupo/${grupo.id}`}>{grupo.name}</a>
        <p className="resumen">{grupo.about || 'Sin descripción todavía.'}</p>
        <div className="pie">
          <span className="dato">
            <b>{grupo.miembros}</b> {grupo.miembros === 1 ? 'miembro' : 'miembros'}
          </span>
          <span className="dato">
            <b>{grupo.publicaciones}</b> {grupo.publicaciones === 1 ? 'publicación' : 'publicaciones'}
          </span>
          {grupo.privacy === 'private' ? <span className="etiqueta">Privado</span> : null}
        </div>
        <div className="acciones">
          {grupo.soy_miembro ? (
            <a className="btn btn-outline btn-sm" href={`#/grupo/${grupo.id}`}>Ver el grupo</a>
          ) : (
            <button type="button" className="btn btn-aurora btn-sm" onClick={() => alEntrar(grupo)}>
              Entrar al grupo
            </button>
          )}
        </div>
      </div>
    </article>
  );
}

export default function GruposView() {
  const [datos, setDatos] = useState(null);
  const [buscar, setBuscar] = useState('');
  const [creando, setCreando] = useState(false);

  const cargar = useCallback((q = '') => {
    api.get(`/api/groups${q ? `?q=${encodeURIComponent(q)}` : ''}`)
      .then((r) => setDatos({
        mios: Array.isArray(r?.mios) ? r.mios : [],
        // Si algún día el servidor devolviera otra forma, tampoco se rompe la pantalla.
        descubrir: Array.isArray(r?.descubrir) ? r.descubrir : (Array.isArray(r?.items) ? r.items.filter((g) => !g.soy_miembro) : []),
      }))
      .catch(() => setDatos({ mios: [], descubrir: [] }));
  }, []);

  useEffect(() => { cargar(); }, [cargar]);

  // El buscador espera a que dejes de escribir para no molestar al servidor.
  useEffect(() => {
    const t = setTimeout(() => cargar(buscar.trim()), 320);
    return () => clearTimeout(t);
  }, [buscar, cargar]);

  async function entrar(grupo) {
    try {
      await api.post(`/api/groups/${grupo.id}/join`, {});
      toast.ok(`Ya estás en «${grupo.name}»`);
      cargar(buscar.trim());
    } catch (e) {
      avisoError(e);
    }
  }

  async function crear() {
    const nombre = await pedirTexto({
      title: 'Crear un grupo',
      label: '¿Cómo se va a llamar?',
      placeholder: 'Por ejemplo: Astronomía del Sur',
      confirmText: 'Siguiente',
    });
    if (!nombre) return;
    const about = await pedirTexto({
      title: 'Cuéntale a la gente de qué va',
      label: 'Descripción del grupo (opcional)',
      placeholder: 'De qué se habla aquí…',
      confirmText: 'Crear grupo',
      requerido: false,
    });
    if (about === null) return;
    setCreando(true);
    try {
      const g = await api.post('/api/groups', { name: nombre, about: about || '' });
      toast.ok('Grupo creado');
      window.location.hash = `#/grupo/${g.id}`;
    } catch (e) {
      avisoError(e);
    } finally {
      setCreando(false);
    }
  }

  return (
    <>
      <div className="topbar">
        <h1>Grupos</h1>
        <div className="topbar-acciones">
          <label className="buscador-en-linea">
            <IconSearch />
            <input
              className="input"
              type="search"
              value={buscar}
              onChange={(e) => setBuscar(e.target.value)}
              placeholder="Buscar grupos"
              aria-label="Buscar grupos"
            />
          </label>
          <button type="button" className="btn btn-aurora btn-sm" onClick={crear} disabled={creando}>
            <IconPlus /> {creando ? 'Creando…' : 'Crear grupo'}
          </button>
        </div>
      </div>

      {datos === null ? (
        <ListSkeleton etiqueta="Cargando tus grupos…" />
      ) : (
        <>
          <section className="bloque">
            <h2 className="titulo-bloque">
              {datos.mios.length > 0 ? 'Tus grupos' : 'Todavía no estás en ningún grupo'}
            </h2>
            {datos.mios.length > 0 ? (
              <div className="rejilla-grupos">
                {datos.mios.map((g) => <Tarjeta key={g.id} grupo={g} alEntrar={entrar} />)}
              </div>
            ) : (
              <div className="empty">
                <IconLayers />
                <h3>Los grupos son para compartir con quien quieras</h3>
                <p>Crea el primero o entra en uno de los que aparecen abajo.</p>
                <button type="button" className="btn btn-aurora" onClick={crear}>
                  <IconPlus /> Crear mi primer grupo
                </button>
              </div>
            )}
          </section>

          {datos.descubrir.length > 0 ? (
            <section className="bloque">
              <h2 className="titulo-bloque">Grupos para descubrir</h2>
              <div className="rejilla-grupos">
                {datos.descubrir.map((g) => <Tarjeta key={g.id} grupo={g} alEntrar={entrar} />)}
              </div>
            </section>
          ) : null}

          {buscar && datos.mios.length === 0 && datos.descubrir.length === 0 ? (
            <div className="empty">
              <IconSearch />
              <h3>Ningún grupo se llama así</h3>
              <p>Prueba con otra palabra o crea tú ese grupo.</p>
              <button type="button" className="btn btn-aurora" onClick={crear}>
                <IconPlus /> Crear «{buscar}»
              </button>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
