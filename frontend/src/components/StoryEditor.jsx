// Moon — Editor de historias
// ============================================================
// Editor visual, todo en el navegador (sin tocar el backend): sobre la foto
// elegida puedes escribir texto, cambiar su color, tamaño, alineación y
// posición arrastrando. Al publicar, el texto se "dibuja" sobre la imagen con
// un canvas y se sube la foto ya compuesta, igual que hacen las redes grandes.

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { IconX } from './Icons.jsx';

const COLORES = ['#ffffff', '#000000', '#ffd400', '#ff3b6f', '#4f46e5', '#22c55e', '#ff7a00'];
const TAMAÑOS = [{ id: 's', label: 'S', rel: 0.045 }, { id: 'm', label: 'M', rel: 0.065 }, { id: 'l', label: 'L', rel: 0.09 }];

/** Parte el texto en líneas que quepan en `maxAncho` px. */
function partirLineas(ctx, texto, maxAncho) {
  const palabras = String(texto).split(/\s+/).filter(Boolean);
  const lineas = [];
  let actual = '';
  for (const p of palabras) {
    const prueba = actual ? `${actual} ${p}` : p;
    if (ctx.measureText(prueba).width > maxAncho && actual) {
      lineas.push(actual);
      actual = p;
    } else {
      actual = prueba;
    }
  }
  if (actual) lineas.push(actual);
  return lineas;
}

export default function StoryEditor({ archivo, onCancelar, onListo }) {
  const [texto, setTexto] = useState('');
  const [color, setColor] = useState('#ffffff');
  const [tam, setTam] = useState('m');
  const [fondo, setFondo] = useState(true);         // pastilla semitransparente tras el texto
  const [pos, setPos] = useState({ x: 0.5, y: 0.82 }); // posición normalizada del bloque
  const [url, setUrl] = useState('');
  const [natural, setNatural] = useState({ w: 1080, h: 1920 });
  const zonaRef = useRef(null);
  const arrastrando = useRef(false);

  useEffect(() => {
    const u = URL.createObjectURL(archivo);
    setUrl(u);
    const img = new Image();
    img.onload = () => setNatural({ w: img.naturalWidth, h: img.naturalHeight });
    img.src = u;
    return () => URL.revokeObjectURL(u);
  }, [archivo]);

  const rel = useMemo(() => (TAMAÑOS.find((t) => t.id === tam) || TAMAÑOS[1]).rel, [tam]);
  const tamPx = Math.round(natural.w * rel);

  // Mueve el bloque de texto arrastrando sobre la vista previa.
  function alMover(e) {
    if (!arrastrando.current || !zonaRef.current) return;
    const r = zonaRef.current.getBoundingClientRect();
    const px = (e.touches ? e.touches[0].clientX : e.clientX) - r.left;
    const py = (e.touches ? e.touches[0].clientY : e.clientY) - r.top;
    setPos({ x: Math.min(0.96, Math.max(0.04, px / r.width)), y: Math.min(0.96, Math.max(0.06, py / r.height)) });
  }

  function publicar() {
    const img = new Image();
    img.onload = () => {
      const W = img.naturalWidth;
      const H = img.naturalHeight;
      const canvas = document.createElement('canvas');
      canvas.width = W;
      canvas.height = H;
      const ctx = canvas.getContext('2d');
      ctx.drawImage(img, 0, 0, W, H);

      if (texto.trim()) {
        const fs = Math.round(W * rel);
        ctx.font = `700 ${fs}px -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        const lineas = partirLineas(ctx, texto.trim(), W * 0.82);
        const altoLinea = fs * 1.2;
        const cx = pos.x * W;
        const cy = pos.y * H;
        const startY = cy - ((lineas.length - 1) * altoLinea) / 2;

        if (fondo) {
          let anchoMax = 0;
          lineas.forEach((l) => { anchoMax = Math.max(anchoMax, ctx.measureText(l).width); });
          const padX = fs * 0.4;
          const padY = fs * 0.28;
          const bw = anchoMax + padX * 2;
          const bh = lineas.length * altoLinea + padY * 2;
          ctx.fillStyle = 'rgba(0,0,0,0.42)';
          const r = fs * 0.35;
          const x0 = cx - bw / 2;
          const y0 = cy - bh / 2;
          ctx.beginPath();
          if (ctx.roundRect) ctx.roundRect(x0, y0, bw, bh, r);
          else ctx.rect(x0, y0, bw, bh);
          ctx.fill();
        }

        ctx.fillStyle = color;
        ctx.shadowColor = 'rgba(0,0,0,0.35)';
        ctx.shadowBlur = fs * 0.12;
        lineas.forEach((l, i) => ctx.fillText(l, cx, startY + i * altoLinea));
      }

      canvas.toBlob((blob) => {
        const file = new File([blob], 'historia.jpg', { type: 'image/jpeg' });
        onListo(file, texto.trim());
      }, 'image/jpeg', 0.9);
    };
    img.src = url;
  }

  const alinear = { transform: `translate(-50%, -50%) left ${pos.x * 100}% top ${pos.y * 100}%` };

  return (
    <div className="editor-historia" role="dialog" aria-modal="true" aria-label="Editar historia">
      <div className="editor-caja">
        <header className="editor-cabecera">
          <b>Editar historia</b>
          <button type="button" onClick={onCancelar} aria-label="Cancelar"><IconX /></button>
        </header>

        <div
          ref={zonaRef}
          className="editor-lienzo"
          onPointerDown={() => { arrastrando.current = true; }}
          onPointerUp={() => { arrastrando.current = false; }}
          onPointerLeave={() => { arrastrando.current = false; }}
          onPointerMove={alMover}
        >
          {url ? <img src={url} alt="Tu historia" draggable={false} /> : null}
          {texto ? (
            <div
              className="editor-texto"
              style={{
                left: `${pos.x * 100}%`,
                top: `${pos.y * 100}%`,
                transform: 'translate(-50%, -50%)',
                color,
                fontSize: `clamp(14px, ${rel * 100}cqw, 40px)`,
                background: fondo ? 'rgba(0,0,0,0.42)' : 'transparent',
              }}
            >
              {texto}
            </div>
          ) : null}
          {!texto ? <span className="editor-ayuda">Escribe abajo y arrastra el texto donde quieras</span> : null}
        </div>

        <div className="editor-controles">
          <textarea
            className="editor-input"
            rows={2}
            maxLength={220}
            placeholder="Escribe algo…"
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
          />

          <div className="editor-fila">
            <div className="editor-colores" role="group" aria-label="Color del texto">
              {COLORES.map((c) => (
                <button
                  key={c}
                  type="button"
                  className={`editor-color${color === c ? ' activa' : ''}`}
                  style={{ background: c }}
                  onClick={() => setColor(c)}
                  aria-label={`Color ${c}`}
                />
              ))}
            </div>
            <div className="editor-tams" role="group" aria-label="Tamaño">
              {TAMAÑOS.map((t) => (
                <button key={t.id} type="button" className={`editor-tam${tam === t.id ? ' activa' : ''}`} onClick={() => setTam(t.id)}>{t.label}</button>
              ))}
            </div>
            <label className="editor-fondo">
              <input type="checkbox" checked={fondo} onChange={(e) => setFondo(e.target.checked)} /> Fondo
            </label>
          </div>

          <div className="editor-acciones">
            <button type="button" className="btn" onClick={onCancelar}>Cancelar</button>
            <button type="button" className="btn-aurora" onClick={publicar}>Publicar historia</button>
          </div>
        </div>
      </div>
    </div>
  );
}
