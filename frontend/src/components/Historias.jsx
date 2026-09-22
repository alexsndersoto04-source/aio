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
import { reproducirMusica, detenerMusica } from '../musicaHistorias.js';

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
  const [sonido, setSonido] = useState(true);
  const [voladores, setVoladores] = useState([]); // [{ id, emoji, left, delay }]

  // Comentarios
  const [textoComentario, setTextoComentario] = useState('');
  const [enviandoComentario, setEnviandoComentario] = useState(false);
  const [verComentarios, setVerComentarios] = useState(false);
  const [listaComentarios, setListaComentarios] = useState([]);
  const [cargandoComentarios, setCargandoComentarios] = useState(false);

  const temporizador = useRef(null);
  const audioRef = useRef(null);

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

  // Manejo de música de fondo sincronizada con la historia
  useEffect(() => {
    if (!actual?.music_url || !sonido) {
      detenerMusica();
      if (audioRef.current) {
        audioRef.current.pause();
      }
      return;
    }

    if (actual.music_url.startsWith('synth:')) {
      const pistaId = actual.music_url.replace('synth:', '') || 'lofi';
      reproducirMusica(pistaId);
      if (audioRef.current) audioRef.current.pause();
    } else {
      detenerMusica();
      if (audioRef.current) {
        audioRef.current.currentTime = actual.music_start_sec || 0;
        audioRef.current.play().catch(() => {});
      }
    }

    return () => {
      detenerMusica();
      if (audioRef.current) audioRef.current.pause();
    };
  }, [actual, sonido]);

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

  // Cambia de historia sola a los 6 segundos (se pausa si escribes o mantienes presionado)
  useEffect(() => {
    if (!actual || pausado || verComentarios) return;
    clearTimeout(temporizador.current);
    const duracion = (actual.music_duration_sec || 6) * 1000;
    temporizador.current = setTimeout(siguiente, Math.min(15000, Math.max(5000, duracion)));
    return () => clearTimeout(temporizador.current);
  }, [actual, siguiente, pausado, verComentarios]);

  useEffect(() => {
    const tecla = (e) => {
      if (e.key === 'Escape') {
        if (verComentarios) setVerComentarios(false);
        else alCerrar();
      }
      if (e.key === 'ArrowRight' && !verComentarios) siguiente();
      if (e.key === 'ArrowLeft' && !verComentarios) anterior();
    };
    document.addEventListener('keydown', tecla);
    document.body.style.overflow = 'hidden';
    return () => {
      document.removeEventListener('keydown', tecla);
      document.body.style.overflow = '';
      detenerMusica();
      if (audioRef.current) audioRef.current.pause();
    };
  }, [alCerrar, siguiente, anterior, verComentarios]);

  // Reacciones persistentes
  async function reaccionar(emoji) {
    if (!actual) return;
    // Pausar temporalmente para que el usuario aprecie su reacción
    setPausado(true);
    setTimeout(() => setPausado(false), 3500);

    // Animación de emojis voladores
    const nuevos = Array.from({ length: 7 }).map((_, idx) => ({
      id: Date.now() + Math.random(),
      emoji,
      left: Math.random() * 75 + 12,
      delay: idx * 0.1,
    }));
    setVoladores((v) => [...v, ...nuevos]);
    setTimeout(() => {
      setVoladores((v) => v.filter((item) => !nuevos.includes(item)));
    }, 1800);

    // Persistencia inmediata en el estado
    setDatos((d) => {
      if (!d) return d;
      return {
        ...d,
        stories: d.stories.map((s) => {
          if (s.id === actual.id) {
            const eraMismo = s.mi_reaccion === emoji;
            return {
              ...s,
              mi_reaccion: emoji,
              reactions_count: eraMismo ? s.reactions_count : (s.reactions_count || 0) + 1,
            };
          }
          return s;
        }),
      };
    });

    try {
      await api.post(`/api/stories/${actual.id}/react`, { emoji });
    } catch (_) {}
  }

  // Enviar comentario a la historia
  async function enviarComentario(e) {
    if (e) e.preventDefault();
    if (!actual || !textoComentario.trim() || enviandoComentario) return;
    setEnviandoComentario(true);
    const texto = textoComentario.trim();
    try {
      await api.post(`/api/stories/${actual.id}/comments`, { content: texto });
      toast.ok('Comentario enviado al creador');
      setTextoComentario('');
      setDatos((d) => {
        if (!d) return d;
        return {
          ...d,
          stories: d.stories.map((s) => s.id === actual.id ? { ...s, comments_count: (s.comments_count || 0) + 1 } : s),
        };
      });
    } catch (err) {
      avisoError(err);
    } finally {
      setEnviandoComentario(false);
      setPausado(false);
    }
  }

  // Cargar comentarios para el autor
  async function abrirComentariosAutor() {
    if (!actual) return;
    setVerComentarios(true);
    setPausado(true);
    setCargandoComentarios(true);
    try {
      const res = await api.get(`/api/stories/${actual.id}/comments`);
      setListaComentarios(res.items || []);
    } catch (err) {
      avisoError(err);
    } finally {
      setCargandoComentarios(false);
    }
  }

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
        {/* Elemento de audio nativo para música de archivos subidos */}
        {actual?.music_url && !actual.music_url.startsWith('synth:') && (
          <audio
            ref={audioRef}
            src={imgUrl(actual.music_url)}
            preload="auto"
            loop
          />
        )}

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

          {/* Badge de música si tiene audio */}
          {actual?.music_title && (
            <div
              className="visor-badge-musica"
              onClick={() => setSonido(!sonido)}
              role="button"
              title="Activar o silenciar música"
            >
              <span>🎵 {actual.music_title}</span>
              <span style={{ fontSize: 13 }}>{sonido ? '🔊' : '🔇'}</span>
            </div>
          )}

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

          {/* Emojis flotantes animados */}
          {voladores.map((item) => (
            <span
              key={item.id}
              className="emoji-flotante"
              style={{
                left: `${item.left}%`,
                animationDelay: `${item.delay}s`,
              }}
            >
              {item.emoji}
            </span>
          ))}

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

        {/* Pie flotante con caption, reacciones 100% fijas y caja de comentarios permanente */}
        <div className="visor-pie-flotante">
          {actual?.caption ? (
            <p className="visor-pie-texto">{actual.caption}</p>
          ) : null}

          {/* Barra de reacciones fijas y permanentes */}
          <div className="visor-barra-reacciones">
            <div className="visor-emojis-reaccion">
              {['❤️', '🔥', '😂', '😮', '👏', '🌙', '💯'].map((em) => {
                const esActivo = actual?.mi_reaccion === em;
                return (
                  <button
                    key={em}
                    type="button"
                    className={`visor-btn-emoji${esActivo ? ' activa' : ''}`}
                    onClick={() => reaccionar(em)}
                    aria-label={`Reaccionar con ${em}`}
                    style={{
                      transform: esActivo ? 'scale(1.28)' : 'scale(1)',
                      filter: esActivo ? 'drop-shadow(0 0 8px #ffffff)' : 'none',
                    }}
                  >
                    {em}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Caja permanente para comentar la historia */}
          <div style={{ display: 'flex', gap: 8, alignItems: 'center', width: '100%' }}>
            <form onSubmit={enviarComentario} className="visor-fila-comentario" style={{ flex: 1 }}>
              <input
                className="visor-input-comentario"
                placeholder={mia ? 'Añade una nota o comentario…' : `Comentar a @${datos?.user?.username}…`}
                value={textoComentario}
                onChange={(e) => setTextoComentario(e.target.value)}
                onFocus={() => setPausado(true)}
                onBlur={() => setPausado(false)}
              />
              <button
                type="submit"
                className="visor-btn-enviar-comentario"
                disabled={!textoComentario.trim() || enviandoComentario}
              >
                {enviandoComentario ? '…' : 'Enviar'}
              </button>
            </form>

            <button
              type="button"
              className="visor-btn-ver-comentarios"
              onClick={abrirComentariosAutor}
              title="Ver comentarios de esta historia"
            >
              💬 {actual?.comments_count || 0}
            </button>
          </div>
        </div>

        {/* Modal / Panel de comentarios de la historia */}
        {verComentarios && (
          <div className="visor-modal-comentarios">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
              <b style={{ color: '#fff', fontSize: 16 }}>Comentarios de la historia</b>
              <button
                type="button"
                className="btn-ghost btn-sm"
                style={{ color: '#fff' }}
                onClick={() => {
                  setVerComentarios(false);
                  setPausado(false);
                }}
              >
                ✕ Cerrar
              </button>
            </div>

            {cargandoComentarios ? (
              <p className="muted">Cargando comentarios…</p>
            ) : listaComentarios.length === 0 ? (
              <p className="muted" style={{ margin: 'auto', textAlign: 'center' }}>
                Aún nadie ha comentado esta historia.
              </p>
            ) : (
              <div className="visor-comentarios-lista">
                {listaComentarios.map((c) => (
                  <div key={c.id} className="visor-comentario-item">
                    <Avatar user={c.user} size="sm" />
                    <div>
                      <div style={{ color: '#fff', fontSize: 12, fontWeight: 700 }}>
                        {c.user.display_name || c.user.username}{' '}
                        <span className="muted" style={{ fontWeight: 400, marginLeft: 4 }}>
                          {tiempoRelativo(c.created_at)}
                        </span>
                      </div>
                      <div className="visor-comentario-burbuja" style={{ color: '#fff', marginTop: 2 }}>
                        {c.content}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
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

  async function publicarDesdeEditor(file, texto, musicTitle = '', musicUrl = '', musicStart = 0, musicDuration = 15) {
    setSubiendo(true);
    try {
      const subida = await uploadMedia('story', file);
      await api.post('/api/stories', {
        image_url: subida.url,
        caption: texto || '',
        music_title: musicTitle || '',
        music_url: musicUrl || '',
        music_start_sec: musicStart || 0,
        music_duration_sec: musicDuration || 15,
      });
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

              {/* Información abajo: Nombre limpio */}
              <div className="historia-info-abajo">
                <p className="historia-nombre">
                  {g.mine ? 'Tu historia' : (g.display_name || g.username).split(' ')[0]}
                </p>
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
