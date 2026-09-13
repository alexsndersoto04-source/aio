// Moon — Historias (24 horas)
// ============================================================
// Historias reales, guardadas en la base de datos: se ven las propias y las
// de a quienes sigues, caducan a las 24 horas y cada visita queda registrada.
//
// La fila va arriba del inicio, como en las redes grandes. El anillo con el
// degradado de la casa marca lo que aún no has visto.

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, uploadMedia, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError, pedirTexto, confirmar } from '../ui.js';
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

  // Al mostrar una historia se marca como vista (cuenta real en la base).
  useEffect(() => {
    if (!actual || actual.vista) return;
    api.post(`/api/stories/${actual.id}/view`, {})
      .then(() => {
        setDatos((d) => d ? { ...d, stories: d.stories.map((s) => (s.id === actual.id ? { ...s, vista: true, views_count: s.views_count + 1 } : s)) } : d);
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

  const anterior = () => setIndice((i) => Math.max(0, i - 1));

  // Cambia de historia sola a los 5 segundos, como es costumbre.
  useEffect(() => {
    if (!actual) return;
    clearTimeout(temporizador.current);
    temporizador.current = setTimeout(siguiente, 5000);
    return () => clearTimeout(temporizador.current);
  }, [actual, siguiente]);

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
  }, [alCerrar, siguiente]);

  async function borrar() {
    if (!actual) return;
    const ok = await confirmar({
      title: '¿Eliminar esta historia?',
      message: 'Dejará de verse en tu perfil.',
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
    <div className="visor-historias" role="dialog" aria-modal="true" aria-label="Historias">
      <button type="button" className="visor-velo" onClick={alCerrar} aria-label="Cerrar las historias" />

      <div className="visor-caja">
        <div className="visor-progreso">
          {datos?.stories?.map((s, i) => (
            <span key={s.id} className={`tramo${i < indice ? ' hecho' : i === indice ? ' actual' : ''}`} />
          ))}
        </div>

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
              <button type="button" onClick={borrar} aria-label="Eliminar la historia"><IconTrash /></button>
            ) : null}
            <button type="button" onClick={alCerrar} aria-label="Cerrar"><IconX /></button>
          </div>
        </header>

        <div className="visor-contenido">
          {cargando ? (
            <div className="cargando"><span className="giro" aria-hidden="true" /><span>Abriendo la historia…</span></div>
          ) : actual ? (
            <img src={imgUrl(actual.image_url)} alt={actual.caption || 'Historia'} />
          ) : (
            <p className="muted">Esta historia ya no está.</p>
          )}

          {indice > 0 ? (
            <button type="button" className="flecha izq" onClick={anterior} aria-label="Anterior"><IconChevronLeft /></button>
          ) : null}
          {datos && indice < datos.stories.length - 1 ? (
            <button type="button" className="flecha der" onClick={siguiente} aria-label="Siguiente"><IconChevronLeft /></button>
          ) : null}
        </div>

        {actual?.caption ? <p className="visor-pie">{actual.caption}</p> : null}

        {mia ? (
          <p className="visor-quien">
            {datos?.stories?.length} historia(s) · se borran solas a las 24 horas
          </p>
        ) : null}
      </div>
    </div>
  );
}

export default function Historias({ onNovedad }) {
  const { user } = useAuth();
  const [grupos, setGrupos] = useState([]);
  const [abierto, setAbierto] = useState(null);
  const [subiendo, setSubiendo] = useState(false);
  const archivoRef = useRef(null);

  const cargar = useCallback(() => {
    api.get('/api/stories').then(setGrupos).catch(() => setGrupos([]));
  }, []);

  useEffect(() => { cargar(); }, [cargar]);

  async function elegirArchivo(e) {
    const archivo = e.target.files?.[0];
    e.target.value = '';
    if (!archivo) return;
    setSubiendo(true);
    try {
      const subida = await uploadMedia('story', archivo);
      const pie = await pedirTexto({
        title: 'Tu historia',
        label: '¿Quieres escribir algo? (opcional)',
        placeholder: 'Lo que quieras contar…',
        confirmText: 'Publicar historia',
        requerido: false,
      });
      if (pie === null) { toast.info('Historia cancelada'); return; }
      await api.post('/api/stories', { image_url: subida.url, caption: pie || '' });
      toast.ok('Historia publicada. Durará 24 horas.');
      cargar();
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
      <section className="historias" aria-label="Historias">
        <button
          type="button"
          className="historia crear"
          onClick={() => archivoRef.current?.click()}
          disabled={subiendo}
        >
          <span className="aro">
            <span className="relleno">
              {subiendo ? <span className="giro mini" aria-hidden="true" /> : <IconPlus />}
            </span>
          </span>
          <b>{subiendo ? 'Subiendo…' : mia ? 'Añadir historia' : 'Crear historia'}</b>
        </button>
        <input
          ref={archivoRef}
          type="file"
          accept="image/*"
          hidden
          onChange={elegirArchivo}
          aria-label="Elegir una imagen para tu historia"
        />

        {grupos.map((g) => (
          <button type="button" key={g.user_id} className="historia" onClick={() => setAbierto(g)}>
            <span className={`aro${g.sin_ver > 0 ? ' sin-ver' : ''}${g.mine ? ' mia' : ''}`}>
              <span className="relleno">
                <Avatar user={{ username: g.username, display_name: g.display_name, avatar_url: g.avatar_url }} size="md" />
              </span>
            </span>
            <b>{g.mine ? 'Tu historia' : (g.display_name || g.username).split(' ')[0]}</b>
            <small>{g.total > 1 ? `${g.total} historias` : '1 historia'}</small>
          </button>
        ))}
      </section>

      {abierto ? (
        <Visor
          grupo={abierto}
          alCerrar={() => { setAbierto(null); cargar(); }}
          alCambiarContador={onNovedad}
        />
      ) : null}
    </>
  );
}
