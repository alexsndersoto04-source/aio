// Moon — Redactor de publicaciones
// ============================================================
// Escribe texto, adjunta hasta 4 imágenes (clic, arrastrar o pegar),
// muestra la vista previa y publica contra el backend real. El anillo
// del contador avisa cuando se acerca al límite de caracteres.

import React, { useEffect, useRef, useState } from 'react';
import { api, uploadMedia, imgUrl } from '../api.js';
import { useAuth } from '../auth.jsx';
import { toast, avisoError } from '../ui.js';
import Avatar from './Avatar.jsx';
import { IconImage, IconVideo, IconX, IconSend, IconTrend } from './Icons.jsx';

const MAX_CHARS = 2000;
const MAX_IMAGENES = 4;

export default function Composer({ onCreated, destino = '/api/posts', placeholder, etiquetaBoton }) {
  const { user } = useAuth();
  const [contenido, setContenido] = useState('');
  const [imagenes, setImagenes] = useState([]); // { id, url, kind }
  const [enviando, setEnviando] = useState(false);
  const [subiendo, setSubiendo] = useState(0);
  const [progresoVideo, setProgresoVideo] = useState(0);
  const [arrastrando, setArrastrando] = useState(false);
  // Encuesta opcional: una pregunta con dos a cuatro respuestas.
  const [encuesta, setEncuesta] = useState(null); // null = sin encuesta
  // Menciones: al escribir «@» se busca gente y se ofrece la lista.
  const [mencion, setMencion] = useState(null); // { desde, consulta }
  const [gente, setGente] = useState([]);
  const area = useRef(null);
  const archivos = useRef(null);
  const archivosVideo = useRef(null);

  // El menú y el botón flotante piden el foco con este evento.
  useEffect(() => {
    function enfocar() {
      if (area.current) {
        area.current.focus();
        area.current.scrollIntoView({ block: 'center', behavior: 'smooth' });
      }
    }
    window.addEventListener('moon:componer', enfocar);
    return () => window.removeEventListener('moon:componer', enfocar);
  }, []);

  async function subir(file) {
    const esVid = file.type.startsWith('video/') || /\.(mp4|webm|mov|mkv|3gp)$/i.test(file.name || '');
    const esImg = file.type.startsWith('image/');

    if (!esVid && !esImg) {
      toast.err('Solo se permiten imágenes (JPEG, PNG, WebP) o videos (MP4, WebM, MOV)');
      return;
    }

    if (esVid && file.size > 120 * 1024 * 1024) {
      toast.err('El video supera el límite de 120 MB');
      return;
    }
    if (esImg && file.size > 15 * 1024 * 1024) {
      toast.err('La imagen supera los 15 MB');
      return;
    }

    setSubiendo((n) => n + 1);
    if (esVid) setProgresoVideo(1);

    try {
      const res = await uploadMedia(
        esVid ? 'video' : 'post',
        file,
        esVid ? (pct) => setProgresoVideo(pct) : undefined
      );
      setImagenes((prev) => (prev.length >= MAX_IMAGENES ? prev : [...prev, {
        id: res.id,
        url: res.url,
        kind: esVid ? 'video' : (res.kind || 'image'),
      }]));
    } catch (e) {
      avisoError(e);
    } finally {
      setSubiendo((n) => Math.max(0, n - 1));
      if (esVid) setProgresoVideo(0);
    }
  }

  function elegir(e) {
    const lista = Array.from(e.target.files || []);
    e.target.value = '';
    const hueco = MAX_IMAGENES - imagenes.length;
    if (hueco <= 0) { toast.info(`Máximo ${MAX_IMAGENES} adjuntos por publicación`); return; }
    lista.slice(0, hueco).forEach(subir);
  }

  function soltar(e) {
    e.preventDefault();
    setArrastrando(false);
    const lista = Array.from(e.dataTransfer?.files || []).filter((f) => f.type.startsWith('image/') || f.type.startsWith('video/'));
    const hueco = MAX_IMAGENES - imagenes.length;
    lista.slice(0, hueco).forEach(subir);
  }

  async function publicar() {
    const texto = contenido.trim();
    const opciones = encuesta ? encuesta.opciones.map((o) => o.trim()).filter(Boolean) : [];
    const hayEncuesta = !!encuesta && opciones.length >= 2;
    if ((!texto && imagenes.length === 0 && !hayEncuesta) || enviando) return;
    setEnviando(true);
    try {
      const creado = await api.post(destino, {
        content: texto,
        images: imagenes.map((i) => ({ id: i.id, url: i.url })),
        poll: hayEncuesta
          ? { pregunta: (encuesta.pregunta || '').trim(), opciones, horas: encuesta.horas }
          : undefined,
      });
      setContenido('');
      setImagenes([]);
      setEncuesta(null);
      toast.ok('Publicación creada');
      if (onCreated) onCreated(creado);
    } catch (e) {
      avisoError(e);
    } finally {
      setEnviando(false);
    }
  }

  const usados = contenido.length;
  const pct = Math.min(100, Math.round((usados / MAX_CHARS) * 100));
  const nivel = pct >= 100 ? 'danger' : pct >= 85 ? 'warn' : '';
  // Busca personas mientras se escribe una mención (con un respiro de 220 ms).
  useEffect(() => {
    if (!mencion) { setGente([]); return undefined; }
    let vivo = true;
    const t = setTimeout(() => {
      api.get(`/api/search?q=${encodeURIComponent(mencion.consulta)}&type=users`)
        .then((res) => { if (vivo) setGente((res || []).slice(0, 5)); })
        .catch(() => {});
    }, 220);
    return () => { vivo = false; clearTimeout(t); };
  }, [mencion?.consulta, mencion?.desde]);

  /** Mete el @usuario en el texto, en el sitio donde se escribió. */
  function elegirMencion(usuario) {
    const desde = mencion?.desde ?? contenido.length;
    const hasta = desde + 1 + (mencion?.consulta || '').length;
    const next = `${contenido.slice(0, desde)}@${usuario} ${contenido.slice(hasta)}`;
    setContenido(next.slice(0, MAX_CHARS + 200));
    setMencion(null);
    setGente([]);
    if (area.current) area.current.focus();
  }

  const opcionesOk = encuesta ? encuesta.opciones.map((o) => o.trim()).filter(Boolean).length >= 2 : false;
  const puede = (contenido.trim().length > 0 || imagenes.length > 0 || opcionesOk) && !enviando && usados <= MAX_CHARS;

  return (
    <div
      className="composer"
      onDragOver={(e) => { e.preventDefault(); setArrastrando(true); }}
      onDragLeave={() => setArrastrando(false)}
      onDrop={soltar}
      style={arrastrando ? { boxShadow: 'inset 0 0 0 2px var(--accent)' } : undefined}
    >
      <Avatar user={user} />

      <div className="body">
        <textarea
          ref={area}
          className="textarea"
          placeholder={placeholder || '¿Qué está pasando en tu órbita?'}
          value={contenido}
          maxLength={MAX_CHARS + 200}
          rows={2}
          aria-label="Texto de la publicación"
          onChange={(e) => {
            const valor = e.target.value;
            setContenido(valor);
            const el = e.target;
            el.style.height = 'auto';
            el.style.height = `${Math.min(el.scrollHeight, 320)}px`;

            // ¿Estoy escribiendo una mención? Se mira la palabra antes del cursor.
            const cursor = el.selectionStart ?? valor.length;
            const antes = valor.slice(0, cursor);
            const m = antes.match(/(^|\s)@([a-zA-Z0-9_]{0,24})$/);
            if (m) setMencion({ desde: cursor - m[2].length - 1, consulta: m[2] });
            else setMencion(null);
          }}
          onKeyDown={(e) => {
            if (mencion && gente.length > 0 && e.key === 'Enter') {
              e.preventDefault();
              elegirMencion(gente[0].username);
            }
            if (e.key === 'Escape') setMencion(null);
          }}
        />

        {mencion && gente.length > 0 ? (
          <div className="menciones" role="listbox" aria-label="Personas para mencionar">
            {gente.map((u) => (
              <button
                type="button"
                key={u.id}
                role="option"
                aria-selected="false"
                onClick={() => elegirMencion(u.username)}
              >
                <Avatar user={u} size="sm" />
                <span className="quien">
                  <b>{u.display_name || u.username}</b>
                  <span className="muted small">@{u.username}</span>
                </span>
              </button>
            ))}
          </div>
        ) : null}

        {imagenes.length > 0 ? (
          <div className="composer-preview">
            {imagenes.map((img, i) => {
              const esVid = img.kind === 'video' || /\.(mp4|webm|mov|mkv|3gp)(\?.*)?$/i.test(img.url);
              return (
                <div className={`thumb ${esVid ? 'thumb-video' : ''}`} key={img.id}>
                  {esVid ? (
                    <div className="video-thumb-preview">
                      <video src={imgUrl(img.url)} muted playsInline preload="metadata" />
                      <span className="badge-video">📹 Video</span>
                    </div>
                  ) : (
                    <img src={imgUrl(img.url)} alt={`Adjunto ${i + 1}`} />
                  )}
                  <button
                    type="button"
                    onClick={() => setImagenes((prev) => prev.filter((x) => x.id !== img.id))}
                    aria-label={`Quitar adjunto ${i + 1}`}
                  >
                    <IconX />
                  </button>
                </div>
              );
            })}
          </div>
        ) : null}

        {encuesta ? (
          <div className="composer-encuesta">
            <div className="cabecera-encuesta">
              <b>Encuesta</b>
              <button type="button" className="icon-btn" onClick={() => setEncuesta(null)} aria-label="Quitar la encuesta">
                <IconX />
              </button>
            </div>
            <input
              className="input"
              placeholder="Escribe la pregunta"
              value={encuesta.pregunta}
              maxLength={160}
              aria-label="Pregunta de la encuesta"
              onChange={(e) => setEncuesta({ ...encuesta, pregunta: e.target.value })}
            />
            {encuesta.opciones.map((o, i) => (
              <div className="fila-opcion" key={i}>
                <input
                  className="input"
                  placeholder={`Respuesta ${i + 1}`}
                  value={o}
                  maxLength={80}
                  aria-label={`Respuesta ${i + 1}`}
                  onChange={(e) => {
                    const copia = [...encuesta.opciones];
                    copia[i] = e.target.value;
                    setEncuesta({ ...encuesta, opciones: copia });
                  }}
                />
                {encuesta.opciones.length > 2 ? (
                  <button
                    type="button"
                    className="icon-btn"
                    aria-label={`Quitar la respuesta ${i + 1}`}
                    onClick={() => setEncuesta({ ...encuesta, opciones: encuesta.opciones.filter((_, j) => j !== i) })}
                  >
                    <IconX />
                  </button>
                ) : null}
              </div>
            ))}
            <div className="pie-encuesta-redactor">
              {encuesta.opciones.length < 4 ? (
                <button
                  type="button"
                  className="btn-ghost btn-sm"
                  onClick={() => setEncuesta({ ...encuesta, opciones: [...encuesta.opciones, ''] })}
                >
                  + Añadir respuesta
                </button>
              ) : null}
              <span className="spacer" />
              <label className="duracion">
                Dura
                <select
                  className="select"
                  value={encuesta.horas}
                  aria-label="Duración de la encuesta"
                  onChange={(e) => setEncuesta({ ...encuesta, horas: Number(e.target.value) })}
                >
                  <option value={6}>6 horas</option>
                  <option value={24}>1 día</option>
                  <option value={72}>3 días</option>
                  <option value={168}>7 días</option>
                </select>
              </label>
            </div>
          </div>
        ) : null}

        <div className="composer-tools">
          <input
            ref={archivos}
            type="file"
            accept="image/jpeg,image/png,image/webp,image/gif"
            multiple
            hidden
            onChange={elegir}
          />
          <input
            ref={archivosVideo}
            type="file"
            accept="video/mp4,video/webm,video/quicktime,video/3gpp"
            hidden
            onChange={elegir}
          />
          <button
            type="button"
            className="icon-btn"
            onClick={() => archivos.current && archivos.current.click()}
            disabled={subiendo > 0}
            title="Añadir fotos"
            aria-label="Añadir fotos"
          >
            <IconImage />
          </button>
          <button
            type="button"
            className="icon-btn"
            onClick={() => archivosVideo.current && archivosVideo.current.click()}
            disabled={subiendo > 0}
            title="Añadir video"
            aria-label="Añadir video"
          >
            <IconVideo />
          </button>
          <button
            type="button"
            className={`icon-btn${encuesta ? ' activo' : ''}`}
            onClick={() => setEncuesta(encuesta
              ? null
              : { pregunta: '', opciones: ['', ''], horas: 24 })}
            title={encuesta ? 'Quitar la encuesta' : 'Añadir una encuesta'}
            aria-label={encuesta ? 'Quitar la encuesta' : 'Añadir una encuesta'}
            aria-pressed={!!encuesta}
          >
            <IconTrend />
          </button>
          {subiendo > 0 ? (
            <span className="muted" style={{ fontSize: 13, fontWeight: 500 }}>
              {progresoVideo > 0 ? `Subiendo video ${progresoVideo}%…` : `Subiendo…`}
            </span>
          ) : null}
          {arrastrando ? <span className="muted" style={{ fontSize: 13 }}>Suelta los archivos aquí</span> : null}

          <span className="spacer" />

          {usados > MAX_CHARS - 300 ? (
            <span className={`counter ${nivel}`} title={`${usados} de ${MAX_CHARS} caracteres`}>
              <span>{MAX_CHARS - usados}</span>
            </span>
          ) : null}

          <span className="pill" title="Adjuntos">
            {imagenes.length}/{MAX_IMAGENES} adjuntos
          </span>

          <button className="btn btn-aurora btn-sm" onClick={publicar} disabled={!puede}>
            {enviando ? 'Publicando…' : (<><IconSend /> {etiquetaBoton || 'Publicar'}</>)}
          </button>
        </div>
      </div>
    </div>
  );
}
