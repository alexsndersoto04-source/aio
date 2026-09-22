// Moon — Historias (Tarjetas visuales con vista previa de contenido y visor pantalla completa)
// ============================================================
// Historias reales de 24 horas: a simple vista en el feed de Inicio se ve
// el contenido visual real de la historia (foto o diseño con texto) sin tener
// que seleccionarla, y al abrirla se despliega a pantalla completa real inmersiva.

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, uploadMedia, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError, confirmar } from '../ui.js';
import StoryEditor from './StoryEditor.jsx';
import Avatar from './Avatar.jsx';
import { IconPlus, IconX, IconChevronLeft, IconTrash, IconEye } from './Icons.jsx';

function tiempoRelativo(fecha) {
  const t = new Date(fecha).getTime();
  if (!t) return '';
  const min = Math.round((Date.now() - t) / 60000);
  if (min < 1) return 'ahora';
  if (min < 60) return `hace ${min} min`;
  const h = Math.round(min / 60);
  if (h < 24) return `hace ${h} h`;
  return `${Math.round(h / 24)} d`;
}

function Visor({ grupo, alCerrar, alCambiarContador }) {
  const { user } = useAuth();
  const [datos, setDatos] = useState(null);
  const [indice, setIndice] = useState(0);
  const [cargando, setCargando] = useState(true);
  const [pausado, setPausado] = useState(false);
  const temporizador = useRef(null);

  const cargar = useCallback(async () => {
    setCargando(true);
    try {
      const d = await api.get(`/api/stories/${grupo.user_id}`);
      const inicio = Math.max(0, d.stories.findIndex((s) => !s.vista));
      setDatos(d);
      setIndice(inicio);
    } catch (e) {
      avisoError(e);
      alCerrar();
    } finally {
      setCargando(false);
    }
  }, [grupo.user_id, alCerrar]);

  useEffect(() => { cargar(); }, [cargar]);

  const actual = datos?.stories?.[indice];

  // Al mostrar una historia se marca como vista en la base de datos
  useEffect(() => {
    if (!actual || actual.vista) return;
    api.post(`/api/stories/${actual.id}/view`, {})
      .then(() => {
        setDatos((d) => d ? {
          ...d,
          stories: d.stories.map((s) => (s.id === actual.id ? { ...s, vista: true, views_count: s.views_count + 1 } : s))
        } : d);
        if (alCambiarContador) alCambiarContador();
      })
      .catch(() => {});
  }, [actual, alCambiarContador]);

  const siguiente = useCallback(() => {
    if (!datos) return;
    setIndice((i) => {
      if (i < datos.stories.length - 1) return i + 1;
      alCerrar();
      return i;
    });
  }, [datos, alCerrar]);

  const anterior = useCallback(() => {
    setIndice((i) => Math.max(0, i - 1));
  }, []);

  // Cambia de historia sola a los 5 segundos (se pausa si mantienes presionado)
  useEffect(() => {
    if (!actual || pausado) return;
    clearTimeout(temporizador.current);
    temporizador.current = setTimeout(siguiente, 5000);
    return () => clearTimeout(temporizador.current);
  }, [actual, siguiente, pausado]);

  useEffect(() => {
    const tecla = (e) => {
      if (e.key === 'Escape') alCerrar();
      if (e.key === 'ArrowRight') siguiente();
      if (e.key === 'ArrowLeft') anterior();
    };
    document.addEventListener('keydown', tecla);
    document.body.style.overflow = 'hidden';
    return () => {
      document.removeEventListener('keydown', tecla);
      document.body.style.overflow = '';
    };
  }, [alCerrar, siguiente, anterior]);

  async function borrar() {
    if (!actual) return;
    const ok = await confirmar({
      title: '¿Eliminar esta historia?',
      message: 'Dejará de verse en tu perfil y en el feed de tus seguidores.',
      confirmText: 'Eliminar',
      danger: true,
    });
    if (!ok) return;
    try {
      await api.del(`/api/stories/${actual.id}`);
      toast.ok('Historia eliminada');
      if (alCambiarContador) alCambiarContador();
      if (datos.stories.length <= 1) alCerrar();
      else {
        setDatos((d) => ({ ...d, stories: d.stories.filter((s) => s.id !== actual.id) }));
        setIndice((i) => Math.max(0, i - 1));
      }
    } catch (e) {
      avisoError(e);
    }
  }

  const mia = datos?.user && Number(datos.user.id) === Number(user?.id);

  return (
    <div className="visor-historias" role="dialog" aria-modal="true" aria-label="Visor de historias pantalla completa">
      <div className="visor-caja">
        {/* Barras de progreso superiores */}
        <div className="visor-progreso">
          {datos?.stories?.map((s, i) => (
            <span
              key={s.id}
              className={`tramo${i < indice ? ' hecho' : i === indice ? ' actual' : ''}`}
            />
          ))}
        </div>

        {/* Cabecera flotante */}
        <header className="visor-cabecera">
          <Avatar user={datos?.user} size="sm" />
          <div className="quien">
            <b>{datos?.user?.display_name || datos?.user?.username}</b>
            <small>{actual ? tiempoRelativo(actual.created_at) : ''}</small>
          </div>
          <div className="acciones">
            {mia && actual ? (
              <span className="vistas" title="Visitas">
                <IconEye /> {actual.views_count}
              </span>
            ) : null}
            {mia ? (
              <button type="button" onClick={borrar} aria-label="Eliminar la historia">
                <IconTrash />
              </button>
            ) : null}
            <button type="button" onClick={alCerrar} aria-label="Cerrar visor">
              <IconX />
            </button>
          </div>
        </header>

        {/* Contenido visual central a pantalla completa */}
        <div className="visor-contenido">
          {cargando ? (
            <div className="cargando">
              <span className="giro" aria-hidden="true" />
              <span>Cargando historia…</span>
            </div>
          ) : actual ? (
            <img
              src={imgUrl(actual.image_url)}
              alt={actual.caption || 'Historia'}
            />
          ) : (
            <p className="muted">Esta historia ya no está disponible.</p>
          )}

          {/* Zonas táctiles para navegar y mantener presionado para pausar */}
          <div
            className="visor-zonas-tactiles"
            onPointerDown={() => setPausado(true)}
            onPointerUp={() => setPausado(false)}
            onPointerLeave={() => setPausado(false)}
          >
            <div
              className="visor-zona-izq"
              onClick={anterior}
              role="button"
              aria-label="Historia anterior"
            />
            <div
              className="visor-zona-der"
              onClick={siguiente}
              role="button"
              aria-label="Historia siguiente"
            />
          </div>
        </div>

        {/* Pie flotante con caption */}
        {actual?.caption ? (
          <div className="visor-pie-flotante">
            <p className="visor-pie-texto">{actual.caption}</p>
            {mia && (
              <div className="visor-pie-meta">
                {datos?.stories?.length} historia(s) publicadas · expiran a las 24 horas
              </div>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}

/** Una historia recién publicada (menos de 2 horas) se marca como nueva. */
function esNueva(grupo) {
  const creada = grupo?.created_at || grupo?.ultima;
  if (!creada) return false;
  const t = new Date(creada).getTime();
  if (!t) return false;
  return Date.now() - t < 2 * 60 * 60 * 1000;
}

export default function Historias({ onNovedad }) {
  const { user } = useAuth();
  const [grupos, setGrupos] = useState([]);
  const [abierto, setAbierto] = useState(null);
  const [subiendo, setSubiendo] = useState(false);
  const [editando, setEditando] = useState(false);
  const [archivoEditor, setArchivoEditor] = useState(null);
  const [menuCrear, setMenuCrear] = useState(false);
  const archivoRef = useRef(null);

  const cargar = useCallback(() => {
    api.get('/api/stories').then(setGrupos).catch(() => setGrupos([]));
  }, []);

  useEffect(() => { cargar(); }, [cargar]);

  function elegirArchivo(e) {
    const archivo = e.target.files?.[0];
    e.target.value = '';
    if (!archivo) return;
    setArchivoEditor(archivo);
    setEditando(true);
  }

  function abrirEditorEnBlanco() {
    setArchivoEditor(null);
    setEditando(true);
  }

  async function publicarDesdeEditor(file, texto) {
    setSubiendo(true);
    try {
      const subida = await uploadMedia('story', file);
      await api.post('/api/stories', { image_url: subida.url, caption: texto || '' });
      toast.ok('¡Historia publicada! Estará activa 24 horas.');
      setEditando(false);
      setArchivoEditor(null);
      const nuevos = await api.get('/api/stories');
      setGrupos(nuevos);
      const mia = nuevos.find((g) => g.mine);
      if (mia) setAbierto(mia);
      if (onNovedad) onNovedad();
    } catch (err) {
      avisoError(err);
    } finally {
      setSubiendo(false);
    }
  }

  if (!user) return null;
  const mia = grupos.find((g) => g.mine);

  return (
    <>
      <section className="historias" aria-label="Historias en órbita">
        {/* Tarjeta 1: Crear historia */}
        <div
          className="historia-card crear-card"
          onClick={() => setMenuCrear(true)}
          role="button"
          tabIndex={0}
          aria-label="Crear o añadir historia"
        >
          <div className="crear-card-top">
            {user.avatar_url ? (
              <img
                src={imgUrl(user.avatar_url)}
                alt="Tu perfil"
                className="avatar-fondo"
              />
            ) : (
              <div
                style={{
                  width: '100%',
                  height: '100%',
                  background: 'var(--aurora-grad, linear-gradient(135deg, #4f46e5, #ec4899))',
                  opacity: 0.8,
                }}
              />
            )}
            <div className="crear-btn-flotante">
              {subiendo ? (
                <span className="giro mini" aria-hidden="true" style={{ width: 14, height: 14, borderWidth: 2 }} />
              ) : (
                <IconPlus />
              )}
            </div>
          </div>
          <div className="crear-card-bottom">
            <b>{subiendo ? 'Publicando…' : mia ? 'Añadir más' : 'Crear historia'}</b>
          </div>
        </div>

        <input
          ref={archivoRef}
          type="file"
          accept="image/*"
          hidden
          onChange={elegirArchivo}
          aria-label="Elegir una imagen para tu historia"
        />

        {/* Modal de selección para crear historia */}
        {menuCrear && (
          <div
            style={{
              position: 'fixed',
              inset: 0,
              backgroundColor: 'rgba(0,0,0,0.7)',
              backdropFilter: 'blur(4px)',
              zIndex: 99999,
              display: 'flex',
              alignItems: 'flex-end',
              justifyContent: 'center',
              padding: '0 0 calc(env(safe-area-inset-bottom, 0px) + 16px)',
            }}
            onClick={() => setMenuCrear(false)}
          >
            <div
              className="card"
              style={{
                width: 'min(420px, calc(100% - 32px))',
                padding: '20px 16px',
                display: 'flex',
                flexDirection: 'column',
                gap: 12,
                borderRadius: 20,
                background: 'var(--surface)',
                boxShadow: '0 10px 30px rgba(0,0,0,0.5)',
              }}
              onClick={(e) => e.stopPropagation()}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <b style={{ fontSize: 16 }}>Crear historia</b>
                <button
                  className="btn-ghost btn-sm"
                  onClick={() => setMenuCrear(false)}
                  style={{ borderRadius: '50%', width: 32, height: 32, padding: 0 }}
                >
                  ✕
                </button>
              </div>
              <p className="muted" style={{ margin: 0, fontSize: 13 }}>
                Elige cómo quieres expresarte hoy:
              </p>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginTop: 4 }}>
                <button
                  type="button"
                  className="btn-aurora"
                  style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 10, padding: '12px' }}
                  onClick={() => {
                    setMenuCrear(false);
                    archivoRef.current?.click();
                  }}
                >
                  📷 Subir Foto desde Galería
                </button>
                <button
                  type="button"
                  className="btn-ghost"
                  style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 10, padding: '12px', border: '1px solid var(--line-strong)' }}
                  onClick={() => {
                    setMenuCrear(false);
                    abrirEditorEnBlanco();
                  }}
                >
                  🎨 Crear con Texto, Stickers y Fondos Moon
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Tarjetas de historias reales con vista previa del contenido visual */}
        {grupos.map((g) => {
          const tieneImagen = Boolean(g.image_url);
          return (
            <div
              key={g.user_id}
              className="historia-card"
              onClick={() => setAbierto(g)}
              role="button"
              tabIndex={0}
              aria-label={`Ver historia de ${g.display_name || g.username}`}
            >
              {/* Imagen real de fondo de la historia para verla sin abrirla */}
              {tieneImagen ? (
                <img
                  src={imgUrl(g.image_url)}
                  alt="Vista previa de la historia"
                  className="historia-bg"
                  loading="lazy"
                />
              ) : (
                <div className="historia-bg-placeholder" />
              )}

              {/* Degradado para garantizar contraste visual */}
              <div className="historia-mascara" />

              {/* Si hay texto o pie corto, se muestra como extracto legible */}
              {g.caption ? (
                <div className="historia-extracto-texto">
                  {g.caption}
                </div>
              ) : null}

              {/* Avatar flotante arriba con anillo aurora si está sin ver */}
              <div className={`historia-avatar-aro${g.sin_ver > 0 ? ' sin-ver' : ''}`}>
                <Avatar
                  user={{ username: g.username, display_name: g.display_name, avatar_url: g.avatar_url }}
                  size="sm"
                />
              </div>

              {/* Información abajo: Nombre y etiqueta */}
              <div className="historia-info-abajo">
                <p className="historia-nombre">
                  {g.mine ? 'Tu historia' : (g.display_name || g.username).split(' ')[0]}
                </p>
                {esNueva(g) ? (
                  <span className="historia-badge-nueva">NUEVA</span>
                ) : null}
              </div>
            </div>
          );
        })}
      </section>

      {/* Editor visual potente de historias */}
      {editando ? (
        <StoryEditor
          archivo={archivoEditor}
          onCancelar={() => {
            setEditando(false);
            setArchivoEditor(null);
          }}
          onListo={publicarDesdeEditor}
        />
      ) : null}

      {/* Visor a pantalla completa real */}
      {abierto ? (
        <Visor
          grupo={abierto}
          alCerrar={() => {
            setAbierto(null);
            cargar();
          }}
          alCambiarContador={onNovedad}
        />
      ) : null}
    </>
  );
}
