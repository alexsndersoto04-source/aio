// Moon — Redactor de publicaciones con subida real de imágenes
// ============================================================

import React, { useRef, useState } from 'react';
import { api, uploadMedia } from '../api.js';
import { useAuth } from '../auth.jsx';
import Avatar from './Avatar.jsx';
import { IconImage, IconX } from './Icons.jsx';

export default function Composer({ onCreated }) {
  const { user } = useAuth();
  const [content, setContent] = useState('');
  const [images, setImages] = useState([]); // {id, url}
  const [busy, setBusy] = useState(false);
  const [uploading, setUploading] = useState(false);
  const fileRef = useRef(null);

  function pick(e) {
    const files = Array.from(e.target.files || []);
    e.target.value = '';
    for (const f of files.slice(0, 4)) {
      upload(f);
    }
  }

  async function upload(file) {
    setUploading(true);
    try {
      const res = await uploadMedia('post', file);
      setImages((prev) => [...prev, { id: res.id, url: res.url }]);
    } catch (err) {
      alert(err.message);
    } finally {
      setUploading(false);
    }
  }

  async function submit() {
    const text = content.trim();
    if (!text && images.length === 0) return;
    setBusy(true);
    try {
      const created = await api.post('/api/posts', {
        content: text,
        images: images.map((i) => i.id),
      });
      setContent('');
      setImages([]);
      if (onCreated) onCreated(created);
    } catch (e) {
      alert(e.message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="composer card">
      <div className="row" style={{ alignItems: 'flex-start', gap: 12 }}>
        <Avatar user={user} />
        <textarea
          placeholder="¿Qué está pasando en tu mundo?"
          value={content}
          maxLength={2000}
          onChange={(e) => setContent(e.target.value)}
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') submit();
          }}
        />
      </div>
      {images.length > 0 ? (
        <div className="composer-preview">
          {images.map((img) => (
            <div key={img.id} style={{ position: 'relative' }}>
              <img src={img.url} alt="" />
              <button
                onClick={() => setImages(images.filter((i) => i.id !== img.id))}
                style={{ position: 'absolute', top: 4, right: 4, background: 'rgba(23,23,28,.7)', border: 'none', borderRadius: '50%', color: '#fff', width: 24, height: 24, display: 'grid', placeItems: 'center' }}
                aria-label="Quitar imagen"
              >
                <IconX />
              </button>
            </div>
          ))}
        </div>
      ) : null}
      <div className="composer-foot">
        <div className="composer-tools">
          <button onClick={() => fileRef.current && fileRef.current.click()} disabled={uploading} title="Añadir imágenes">
            <IconImage />
          </button>
          <input ref={fileRef} type="file" accept="image/jpeg,image/png,image/webp" hidden multiple onChange={pick} />
          {uploading ? <span className="muted" style={{ alignSelf: 'center' }}>Subiendo…</span> : null}
        </div>
        <button className="btn btn-sm" disabled={busy || (!content.trim() && images.length === 0) || uploading} onClick={submit}>
          Publicar
        </button>
      </div>
    </div>
  );
}
