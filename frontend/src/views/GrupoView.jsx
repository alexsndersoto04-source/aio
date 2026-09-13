// Moon — Un grupo
// ============================================================
// La portada del grupo, quién está dentro, el redactor para publicar (solo
// para miembros) y lo que se ha publicado, con los mismos me gusta,
// comentarios y guardados de siempre.

import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../api.js';
import { toast, avisoError, confirmar } from '../ui.js';
import Composer from '../components/Composer.jsx';
import PostCard from '../components/PostCard.jsx';
import Avatar, { VerifiedBadge } from '../components/Avatar.jsx';
import { PostSkeleton } from '../components/Skeleton.jsx';
import { IconUsers, IconPlus, IconCheck, IconSettings, IconLayers } from '../components/Icons.jsx';

export default function GrupoView({ id }) {
  const [grupo, setGrupo] = useState(null);
  const [posts, setPosts] = useState([]);
  const [cargando, setCargando] = useState(true);
  const [error, setError] = useState('');
  const [ocupado, setOcupado] = useState(false);

  const cargar = useCallback(async () => {
    setCargando(true);
    try {
      const g = await api.get(`/api/groups/${id}`);
      setGrupo(g);
      try {
        const feed = await api.get(`/api/groups/${id}/posts?page=1&limit=20`);
        setPosts(feed.items || []);
      } catch (e) {
        // Grupo privado al que todavía no pertenezco: no es un error, es un aviso.
        setPosts([]);
      }
    } catch (e) {
      setError(e.message || 'No se pudo abrir el grupo');
    } finally {
      setCargando(false);
    }
  }, [id]);

  useEffect(() => { cargar(); }, [cargar]);

  async function entrarOSalir() {
    if (!grupo || ocupado) return;
    setOcupado(true);
    try {
      if (grupo.soy_miembro) {
        const ok = await confirmar({
          title: '¿Salir del grupo?',
          message: `Dejarás de ver lo que se publica en «${grupo.name}».`,
          confirmText: 'Salir',
          danger: true,
        });
        if (!ok) return;
        await api.del(`/api/groups/${grupo.id}/join`);
        toast.info('Has salido del grupo');
      } else {
        await api.post(`/api/groups/${grupo.id}/join`, {});
        toast.ok('Ya estás dentro');
      }
      cargar();
    } catch (e) {
      avisoError(e);
    } finally {
      setOcupado(false);
    }
  }

  if (cargando) return <PostSkeleton etiqueta="Abriendo el grupo…" alto="40vh" />;

  if (error || !grupo) {
    return (
      <div className="empty">
        <IconLayers />
        <h3>{error || 'Este grupo no existe'}</h3>
        <p>Puede que se haya eliminado o que el enlace esté mal escrito.</p>
        <a className="btn btn-aurora" href="#/grupos">Ver todos los grupos</a>
      </div>
    );
  }

  return (
    <>
      <header className="portada-grupo">
        <div className="cubierta-grupo">
          {grupo.cover_url ? <img src={grupo.cover_url} alt="" /> : <span className="trama" aria-hidden="true" />}
        </div>
        <div className="datos-grupo">
          <h1>{grupo.name}</h1>
          <p className="muted">
            <IconUsers /> {grupo.miembros} {grupo.miembros === 1 ? 'miembro' : 'miembros'}
            {grupo.privacy === 'private' ? ' · grupo privado' : ' · grupo abierto'}
            {' · '}
            <a href={`#/user/${grupo.owner_username}`}>creado por {grupo.owner_display_name || grupo.owner_username}</a>
          </p>
          {grupo.about ? <p className="sobre">{grupo.about}</p> : null}
          <div className="acciones-grupo">
            <button
              type="button"
              className={`btn ${grupo.soy_miembro ? 'btn-outline' : 'btn-aurora'}`}
              onClick={entrarOSalir}
              disabled={ocupado}
            >
              {grupo.soy_miembro ? (<><IconCheck /> Estás dentro</>) : (<><IconPlus /> Entrar al grupo</>)}
            </button>
            <a className="btn btn-ghost" href="#/grupos">Todos los grupos</a>
          </div>
        </div>
      </header>

      <div className="cuerpo-grupo">
        <div className="columna-grupo">
          {grupo.soy_miembro ? (
            <Composer
              destino={`/api/groups/${grupo.id}/posts`}
              placeholder={`Comparte algo con ${grupo.name}…`}
              etiquetaBoton="Publicar"
              onCreated={(post) => setPosts((prev) => [post, ...prev])}
            />
          ) : (
            <div className="aviso-grupo">
              <IconUsers />
              <p>
                {grupo.privacy === 'private'
                  ? 'Este grupo es privado: entra para ver y compartir lo que se publica.'
                  : 'Entra al grupo para publicar y participar.'}
              </p>
              <button type="button" className="btn btn-aurora btn-sm" onClick={entrarOSalir} disabled={ocupado}>
                Entrar al grupo
              </button>
            </div>
          )}

          {posts.length === 0 ? (
            <div className="empty">
              <IconLayers />
              <h3>Aquí todavía no ha publicado nadie</h3>
              <p>{grupo.soy_miembro ? 'Escribe la primera publicación del grupo.' : 'Cuando entres, podrás ser tú quien empiece.'}</p>
            </div>
          ) : (
            posts.map((p) => <PostCard key={p.id} post={p} onChanged={() => cargar()} />)
          )}
        </div>

        <aside className="aside-grupo">
          <section className="tarjeta-rail">
            <h3>Miembros · {grupo.miembros}</h3>
            <ul className="lista-miembros">
              {grupo.miembros_lista.map((m) => (
                <li key={m.id}>
                  <a href={`#/user/${m.id}`}>
                    <Avatar user={m} size="sm" />
                    <span className="quien">
                      <b>
                        {m.display_name}
                        <VerifiedBadge show={m.is_verified} />
                      </b>
                      <small>
                        @{m.username}
                        {m.papel === 'owner' ? ' · lo creó' : m.papel === 'admin' ? ' · administra' : ''}
                      </small>
                    </span>
                  </a>
                </li>
              ))}
            </ul>
          </section>

          {grupo.es_mio ? (
            <section className="tarjeta-rail">
              <h3>
                <IconSettings /> Lo que puedes hacer
              </h3>
              <p className="muted small">
                Eres quien creó este grupo. Puedes cambiar su descripción cuando quieras.
              </p>
              <button
                type="button"
                className="btn btn-outline btn-sm btn-block"
                onClick={async () => {
                  const { pedirTexto } = await import('../ui.js');
                  const texto = await pedirTexto({
                    title: 'Descripción del grupo',
                    label: 'Cuéntale a la gente de qué va',
                    value: grupo.about || '',
                    confirmText: 'Guardar',
                    requerido: false,
                  });
                  if (texto === null) return;
                  try {
                    await api.patch(`/api/groups/${grupo.id}`, { about: texto || '' });
                    toast.ok('Descripción guardada');
                    cargar();
                  } catch (e) {
                    avisoError(e);
                  }
                }}
              >
                Cambiar la descripción
              </button>
            </section>
          ) : null}
        </aside>
      </div>
    </>
  );
}
