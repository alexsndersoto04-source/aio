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
import { IconImage, IconX, IconSend } from './Icons.jsx';

const MAX_CHARS = 2000;
const MAX_IMAGENES = 4;

export default function Composer({ onCreated }) {
  const { user } = useAuth();
  const [contenido, setContenido] = useState('');
  const [imagenes, setImagenes] = useState([]); // { id, url }
  const [enviando, setEnviando] = useState(false);
  const [subiendo, setSubiendo] = useState(0);
  const [arrastrando, setArrastrando] = useState(false);
  const area = useRef(null);
  const archivos = useRef(null);

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
    if (!file.type.startsWith('image/')) {
      toast.err('Solo se pueden adjuntar imágenes (JPEG, PNG o WebP)');
      return;
    }
    setSubiendo((n) => n + 1);
    try {
      const res = await uploadMedia('post', file);
      setImagenes((prev) => (prev.length >= MAX_IMAGENES ? prev : [...prev, { id: res.id, url: res.url }]));
    } catch (e) {
      avisoError(e);
    } finally {
      setSubiendo((n) => n - 1);
    }
  }

  function elegir(e) {
    const lista = Array.from(e.target.files || []);
    e.target.value = '';
    const hueco = MAX_IMAGENES - imagenes.length;
    if (hueco <= 0) { toast.info(`Máximo ${MAX_IMAGENES} imágenes por publicación`); return; }
    lista.slice(0, hueco).forEach(subir);
  }

  function soltar(e) {
    e.preventDefault();
    setArrastrando(false);
    const lista = Array.from(e.dataTransfer?.files || []).filter((f) => f.type.startsWith('image/'));
    const hueco = MAX_IMAGENES - imagenes.length;
    lista.slice(0, hueco).forEach(subir);
  }

  async function publicar() {
    const texto = contenido.trim();
    if ((!texto && imagenes.length === 0) || enviando) return;
    setEnviando(true);
    try {
      const creado = await api.post('/api/posts', {
        content: texto,
        images: imagenes.map((i) => i.id),
      });
      setContenido('');
      setImagenes([]);
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
  const puede = (contenido.trim().length > 0 || imagenes.length > 0) && !enviando && usados <= MAX_CHARS;

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
          placeholder="¿Qué está pasando en tu órbita?"
          value={contenido}
          maxLength={MAX_CHARS + 200}
          rows={2}
          aria-label="Texto de la publicación"
          onChange={(e) => {
            setContenido(e.target.value);
            const el = e.target;
            el.style.height = 'auto';
            el.style.height = `${Math.min(el.scrollHeight, 320)}px`;
          }}
        />

        {imagenes.length > 0 ? (
          <div className="composer-preview">
            {imagenes.map((img, i) => (
              <div className="thumb" key={img.id}>
                <img src={imgUrl(img.url)} alt={`Adjunto ${i + 1}`} />
                <button
                  type="button"
                  onClick={() => setImagenes((prev) => prev.filter((x) => x.id !== img.id))}
                  aria-label={`Quitar adjunto ${i + 1}`}
                >
                  <IconX />
                </button>
              </div>
            ))}
          </div>
        ) : null}

        <div className="composer-tools">
          <input
            ref={archivos}
            type="file"
            accept="image/jpeg,image/png,image/webp"
            multiple
            hidden
            onChange={elegir}
          />
          <button
            type="button"
            className="icon-btn"
            onClick={() => archivos.current && archivos.current.click()}
            disabled={subiendo > 0}
            title="Añadir imágenes"
            aria-label="Añadir imágenes"
          >
            <IconImage />
          </button>
          {subiendo > 0 ? <span className="muted" style={{ fontSize: 13 }}>Subiendo {subiendo}…</span> : null}
          {arrastrando ? <span className="muted" style={{ fontSize: 13 }}>Suelta las imágenes aquí</span> : null}

          <span className="spacer" />

          {usados > MAX_CHARS - 300 ? (
            <span className={`counter ${nivel}`} title={`${usados} de ${MAX_CHARS} caracteres`}>
              <span>{MAX_CHARS - usados}</span>
            </span>
          ) : null}

          <span className="pill" title="Longitud máxima">
            {imagenes.length}/{MAX_IMAGENES} fotos
          </span>

          <button className="btn btn-primary btn-sm" onClick={publicar} disabled={!puede}>
            {enviando ? 'Publicando…' : (<><IconSend /> Publicar</>)}
          </button>
        </div>
      </div>
    </div>
  );
}
