// Moon — Editor de historias profesional (suite creativa avanzada)
// ============================================================
// Permite componer historias con fotos de la galería o fondos degradados,
// múltiples tipografías de impacto, filtros visuales para fotos, stickers/badges,
// estilos de resaltado de texto y arrastre táctil libre en pantalla completa.

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { IconX } from './Icons.jsx';
import { PISTAS_MUSICA, reproducirMusica, detenerMusica } from '../musicaHistorias.js';

const TIPOGRAFIAS = [
  { id: 'moderna', label: 'Moderna', font: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif', weight: '700' },
  { id: 'titular', label: 'Titular', font: 'Impact, "Arial Black", sans-serif', weight: '900', uppercase: true },
  { id: 'elegante', label: 'Elegante', font: 'Georgia, "Times New Roman", serif', weight: '700' },
  { id: 'neon', label: 'Neón', font: '"Courier New", Consolas, monospace', weight: '700', neon: true },
  { id: 'manuscrita', label: 'Manuscrita', font: '"Brush Script MT", "Segoe Script", cursive, sans-serif', weight: '700' },
  { id: 'cyber', label: 'Cyber', font: '"SFMono-Regular", Consolas, "Liberation Mono", monospace', weight: '600' },
];

const COLORES = [
  '#ffffff', '#000000', '#facc15', '#fb923c', '#f43f5e',
  '#a855f7', '#38bdf8', '#4ade80', '#ec4899', '#22d3ee',
];

const ESTILOS_TEXTO = [
  { id: 'transparente', label: 'Limpio' },
  { id: 'glass', label: 'Vidrio' },
  { id: 'solido', label: 'Sólido' },
  { id: 'neon', label: 'Glow' },
];

const TAMAÑOS = [
  { id: 's', label: 'S', rel: 0.045 },
  { id: 'm', label: 'M', rel: 0.065 },
  { id: 'l', label: 'L', rel: 0.088 },
  { id: 'xl', label: 'XL', rel: 0.115 },
];

const FONDOS_DEGRADADOS = [
  { id: 'aurora', label: 'Aurora Moon', css: 'linear-gradient(135deg, #4f46e5 0%, #7c3aed 50%, #ec4899 100%)', colors: ['#4f46e5', '#7c3aed', '#ec4899'] },
  { id: 'cosmic', label: 'Cósmico', css: 'linear-gradient(135deg, #0f172a 0%, #312e81 50%, #4c1d95 100%)', colors: ['#0f172a', '#312e81', '#4c1d95'] },
  { id: 'sunset', label: 'Atardecer', css: 'linear-gradient(135deg, #ea580c 0%, #dc2626 50%, #be123c 100%)', colors: ['#ea580c', '#dc2626', '#be123c'] },
  { id: 'emerald', label: 'Esmeralda', css: 'linear-gradient(135deg, #064e3b 0%, #047857 50%, #10b981 100%)', colors: ['#064e3b', '#047857', '#10b981'] },
  { id: 'violet', label: 'Púrpura', css: 'linear-gradient(135deg, #581c87 0%, #7e22ce 50%, #c026d3 100%)', colors: ['#581c87', '#7e22ce', '#c026d3'] },
  { id: 'ocean', label: 'Océano', css: 'linear-gradient(135deg, #0c4a6e 0%, #0284c7 50%, #38bdf8 100%)', colors: ['#0c4a6e', '#0284c7', '#38bdf8'] },
  { id: 'dark', label: 'Titanio', css: 'linear-gradient(135deg, #18181b 0%, #27272a 50%, #3f3f46 100%)', colors: ['#18181b', '#27272a', '#3f3f46'] },
  { id: 'solar', label: 'Oro Solar', css: 'linear-gradient(135deg, #78350f 0%, #b45309 50%, #f59e0b 100%)', colors: ['#78350f', '#b45309', '#f59e0b'] },
];

const FILTROS = [
  { id: 'normal', label: 'Normal', filter: 'none' },
  { id: 'vivido', label: 'Vívido', filter: 'saturate(1.4) contrast(1.15)' },
  { id: 'bw', label: 'B & N', filter: 'grayscale(1) contrast(1.2)' },
  { id: 'calido', label: 'Cálido', filter: 'sepia(0.35) saturate(1.25)' },
  { id: 'lunar', label: 'Lunar', filter: 'hue-rotate(185deg) saturate(1.15)' },
  { id: 'cyber', label: 'Cyberpunk', filter: 'contrast(1.35) saturate(1.5) brightness(1.05)' },
];

const STICKERS_EMOJIS = ['🔥', '❤️', '✨', '🚀', '🌙', '💯', '👏', '😍', '⚡', '🎉'];

function obtenerHoraActual() {
  const ahora = new Date();
  return ahora.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

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
  const [tipoFuente, setTipoFuente] = useState('moderna');
  const [color, setColor] = useState('#ffffff');
  const [estiloTexto, setEstiloTexto] = useState('glass');
  const [tam, setTam] = useState('m');
  const [alineacion, setAlineacion] = useState('center');
  const [posTexto, setPosTexto] = useState({ x: 0.5, y: 0.45 });

  const [fondoIndex, setFondoIndex] = useState(0);
  const [filtroId, setFiltroId] = useState('normal');
  const [urlImg, setUrlImg] = useState('');
  const [natural, setNatural] = useState({ w: 1080, h: 1920 });

  const [stickerActivo, setStickerActivo] = useState(null); // { tipo: 'emoji' | 'badge', valor: string }
  const [posSticker, setPosSticker] = useState({ x: 0.5, y: 0.72 });

  const [musica, setMusica] = useState(null); // { id, titulo, autor, emoji }
  const [posMusica, setPosMusica] = useState({ x: 0.5, y: 0.18 });

  const [pestana, setPestana] = useState('texto'); // 'texto' | 'estilo' | 'fondo' | 'stickers' | 'musica'
  const [elementoArrastrado, setElementoArrastrado] = useState(null); // 'texto' | 'sticker' | 'musica' | null

  const zonaRef = useRef(null);
  const fileInputRef = useRef(null);

  useEffect(() => {
    if (archivo) {
      const u = URL.createObjectURL(archivo);
      setUrlImg(u);
      const img = new Image();
      img.onload = () => setNatural({ w: img.naturalWidth, h: img.naturalHeight });
      img.src = u;
      return () => URL.revokeObjectURL(u);
    }
  }, [archivo]);

  function cambiarFoto(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    const u = URL.createObjectURL(file);
    setUrlImg(u);
    const img = new Image();
    img.onload = () => setNatural({ w: img.naturalWidth, h: img.naturalHeight });
    img.src = u;
  }

  const fuenteActual = useMemo(() => TIPOGRAFIAS.find((f) => f.id === tipoFuente) || TIPOGRAFIAS[0], [tipoFuente]);
  const rel = useMemo(() => (TAMAÑOS.find((t) => t.id === tam) || TAMAÑOS[1]).rel, [tam]);
  const filtroActual = useMemo(() => FILTROS.find((f) => f.id === filtroId) || FILTROS[0], [filtroId]);
  const fondoActual = useMemo(() => FONDOS_DEGRADADOS[fondoIndex] || FONDOS_DEGRADADOS[0], [fondoIndex]);

  useEffect(() => {
    return () => detenerMusica();
  }, []);

  function alArrastrar(e) {
    if (!elementoArrastrado || !zonaRef.current) return;
    const rect = zonaRef.current.getBoundingClientRect();
    const clientX = e.touches ? e.touches[0].clientX : e.clientX;
    const clientY = e.touches ? e.touches[0].clientY : e.clientY;
    const px = Math.min(0.95, Math.max(0.05, (clientX - rect.left) / rect.width));
    const py = Math.min(0.95, Math.max(0.08, (clientY - rect.top) / rect.height));

    if (elementoArrastrado === 'texto') {
      setPosTexto({ x: px, y: py });
    } else if (elementoArrastrado === 'sticker') {
      setPosSticker({ x: px, y: py });
    } else if (elementoArrastrado === 'musica') {
      setPosMusica({ x: px, y: py });
    }
  }

  function publicar() {
    const W = 1080;
    const H = 1920;
    const canvas = document.createElement('canvas');
    canvas.width = W;
    canvas.height = H;
    const ctx = canvas.getContext('2d');

    const renderContenidoFinal = (imgFondo) => {
      // 1. Dibujar fondo
      if (imgFondo) {
        if (filtroActual.filter !== 'none') {
          ctx.filter = filtroActual.filter;
        }
        // Escalar imagen para cubrir canvas (cover)
        const scale = Math.max(W / imgFondo.naturalWidth, H / imgFondo.naturalHeight);
        const x = (W - imgFondo.naturalWidth * scale) / 2;
        const y = (H - imgFondo.naturalHeight * scale) / 2;
        ctx.drawImage(imgFondo, x, y, imgFondo.naturalWidth * scale, imgFondo.naturalHeight * scale);
        ctx.filter = 'none';

        // Viñeta sutil para mejorar contraste
        const gradVignette = ctx.createLinearGradient(0, 0, 0, H);
        gradVignette.addColorStop(0, 'rgba(0,0,0,0.3)');
        gradVignette.addColorStop(0.3, 'rgba(0,0,0,0)');
        gradVignette.addColorStop(0.7, 'rgba(0,0,0,0)');
        gradVignette.addColorStop(1, 'rgba(0,0,0,0.4)');
        ctx.fillStyle = gradVignette;
        ctx.fillRect(0, 0, W, H);
      } else {
        // Fondo degradado elegido
        const grad = ctx.createLinearGradient(0, 0, W, H);
        grad.addColorStop(0, fondoActual.colors[0]);
        grad.addColorStop(0.5, fondoActual.colors[1]);
        grad.addColorStop(1, fondoActual.colors[2]);
        ctx.fillStyle = grad;
        ctx.fillRect(0, 0, W, H);
      }

      // 2. Dibujar Sticker o Badge si existe
      if (stickerActivo) {
        const sx = posSticker.x * W;
        const sy = posSticker.y * H;
        if (stickerActivo.tipo === 'emoji') {
          ctx.font = '120px sans-serif';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          ctx.shadowColor = 'rgba(0,0,0,0.4)';
          ctx.shadowBlur = 18;
          ctx.fillText(stickerActivo.valor, sx, sy);
          ctx.shadowBlur = 0;
        } else if (stickerActivo.tipo === 'badge') {
          ctx.font = '700 36px -apple-system, sans-serif';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          const txt = stickerActivo.valor;
          const tw = ctx.measureText(txt).width;
          const bw = tw + 60;
          const bh = 70;
          const x0 = sx - bw / 2;
          const y0 = sy - bh / 2;

          ctx.fillStyle = 'rgba(255, 255, 255, 0.95)';
          ctx.shadowColor = 'rgba(0,0,0,0.35)';
          ctx.shadowBlur = 20;
          ctx.beginPath();
          if (ctx.roundRect) ctx.roundRect(x0, y0, bw, bh, 35);
          else ctx.rect(x0, y0, bw, bh);
          ctx.fill();

          ctx.shadowBlur = 0;
          ctx.fillStyle = '#0a0a0c';
          ctx.fillText(txt, sx, sy);
        }
      }

      // 2.5 Dibujar Sticker de Música si existe
      if (musica) {
        const mx = posMusica.x * W;
        const my = posMusica.y * H;
        ctx.font = '700 32px -apple-system, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        const txt = `🎵 ${musica.titulo} · ${musica.autor}`;
        const tw = ctx.measureText(txt).width;
        const bw = tw + 56;
        const bh = 64;
        const x0 = mx - bw / 2;
        const y0 = my - bh / 2;

        ctx.fillStyle = 'rgba(10, 10, 14, 0.78)';
        ctx.shadowColor = 'rgba(0,0,0,0.5)';
        ctx.shadowBlur = 18;
        ctx.beginPath();
        if (ctx.roundRect) ctx.roundRect(x0, y0, bw, bh, 32);
        else ctx.rect(x0, y0, bw, bh);
        ctx.fill();

        ctx.strokeStyle = 'rgba(255, 255, 255, 0.25)';
        ctx.lineWidth = 2;
        ctx.stroke();

        ctx.shadowBlur = 0;
        ctx.fillStyle = '#ffffff';
        ctx.fillText(txt, mx, my);
      }

      // 3. Dibujar Texto si existe
      if (texto.trim()) {
        const fs = Math.round(W * rel);
        const fontName = fuenteActual.font;
        const weight = fuenteActual.weight;
        ctx.font = `${weight} ${fs}px ${fontName}`;
        ctx.textAlign = alineacion;
        ctx.textBaseline = 'middle';

        const lineas = partirLineas(ctx, texto.trim(), W * 0.85);
        const altoLinea = fs * 1.28;
        const cx = posTexto.x * W;
        const cy = posTexto.y * H;
        const startY = cy - ((lineas.length - 1) * altoLinea) / 2;

        if (estiloTexto === 'glass' || estiloTexto === 'solido') {
          let maxW = 0;
          lineas.forEach((l) => { maxW = Math.max(maxW, ctx.measureText(l).width); });
          const padX = fs * 0.45;
          const padY = fs * 0.35;
          const bw = maxW + padX * 2;
          const bh = lineas.length * altoLinea + padY * 2;
          const x0 = alineacion === 'center' ? cx - bw / 2 : alineacion === 'left' ? cx - padX : cx - bw + padX;
          const y0 = cy - bh / 2;

          ctx.fillStyle = estiloTexto === 'solido' ? '#ffffff' : 'rgba(0, 0, 0, 0.55)';
          ctx.beginPath();
          if (ctx.roundRect) ctx.roundRect(x0, y0, bw, bh, 20);
          else ctx.rect(x0, y0, bw, bh);
          ctx.fill();
        }

        if (fuenteActual.neon || estiloTexto === 'neon') {
          ctx.shadowColor = color;
          ctx.shadowBlur = 24;
        } else {
          ctx.shadowColor = 'rgba(0, 0, 0, 0.65)';
          ctx.shadowBlur = 12;
        }

        ctx.fillStyle = estiloTexto === 'solido' ? '#000000' : color;
        lineas.forEach((l, i) => {
          ctx.fillText(l, cx, startY + i * altoLinea);
        });
        ctx.shadowBlur = 0;
      }

      // 4. Exportar a Blob y emitir
      canvas.toBlob((blob) => {
        detenerMusica();
        const file = new File([blob], 'historia.jpg', { type: 'image/jpeg' });
        onListo(file, texto.trim(), musica?.titulo || '', musica ? `synth:${musica.id}` : '');
      }, 'image/jpeg', 0.92);
    };

    if (urlImg) {
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload = () => renderContenidoFinal(img);
      img.src = urlImg;
    } else {
      renderContenidoFinal(null);
    }
  }

  return (
    <div className="editor-historia" role="dialog" aria-modal="true" aria-label="Editor de historias de Moon">
      <div className="editor-caja">
        {/* Cabecera superior flotante */}
        <header className="editor-cabecera">
          <b>Crear historia</b>
          <button type="button" onClick={onCancelar} aria-label="Cerrar editor">
            <IconX />
          </button>
        </header>

        {/* Lienzo interactivo central a pantalla completa */}
        <div
          ref={zonaRef}
          className="editor-lienzo"
          onPointerMove={alArrastrar}
          onPointerUp={() => setElementoArrastrado(null)}
          onPointerCancel={() => setElementoArrastrado(null)}
        >
          {/* Fondo: Imagen con filtro O Degradado */}
          {urlImg ? (
            <img
              src={urlImg}
              alt="Lienzo de historia"
              style={{ filter: filtroActual.filter }}
              draggable={false}
            />
          ) : (
            <div className="fondo-degradado" style={{ background: fondoActual.css }} />
          )}

          {/* Texto arrastrable */}
          {texto.trim() ? (
            <div
              className="editor-texto-caja"
              onPointerDown={(e) => { e.stopPropagation(); setElementoArrastrado('texto'); }}
              style={{
                left: `${posTexto.x * 100}%`,
                top: `${posTexto.y * 100}%`,
                transform: alineacion === 'center' ? 'translate(-50%, -50%)' : alineacion === 'left' ? 'translate(0, -50%)' : 'translate(-100%, -50%)',
                color: estiloTexto === 'solido' ? '#000000' : color,
                fontFamily: fuenteActual.font,
                fontWeight: fuenteActual.weight,
                textTransform: fuenteActual.uppercase ? 'uppercase' : 'none',
                textAlign: alineacion,
                fontSize: `clamp(16px, ${rel * 100}cqw, 48px)`,
                background: estiloTexto === 'glass' ? 'rgba(0,0,0,0.55)' : estiloTexto === 'solido' ? '#ffffff' : 'transparent',
                boxShadow: estiloTexto === 'neon' ? `0 0 16px ${color}` : estiloTexto === 'glass' ? '0 4px 16px rgba(0,0,0,0.3)' : 'none',
                textShadow: estiloTexto === 'transparente' ? '0 2px 8px rgba(0,0,0,0.85)' : 'none',
                border: estiloTexto === 'neon' ? `2px solid ${color}` : 'none',
                pointerEvents: 'auto',
                cursor: 'grab',
              }}
            >
              {texto}
            </div>
          ) : null}

          {/* Sticker o Badge arrastrable */}
          {stickerActivo ? (
            <div
              className="editor-sticker-caja"
              onPointerDown={(e) => { e.stopPropagation(); setElementoArrastrado('sticker'); }}
              style={{
                left: `${posSticker.x * 100}%`,
                top: `${posSticker.y * 100}%`,
                pointerEvents: 'auto',
                cursor: 'grab',
              }}
            >
              {stickerActivo.tipo === 'emoji' ? (
                <span style={{ fontSize: 64, filter: 'drop-shadow(0 4px 10px rgba(0,0,0,0.4))' }}>
                  {stickerActivo.valor}
                </span>
              ) : (
                <div
                  style={{
                    padding: '8px 18px',
                    borderRadius: 999,
                    background: 'rgba(255,255,255,0.95)',
                    color: '#0a0a0c',
                    fontWeight: 700,
                    fontSize: 14,
                    boxShadow: '0 4px 14px rgba(0,0,0,0.35)',
                  }}
                >
                  {stickerActivo.valor}
                </div>
              )}
            </div>
          ) : null}

          {/* Sticker de Música arrastrable */}
          {musica ? (
            <div
              className="editor-sticker-caja"
              onPointerDown={(e) => { e.stopPropagation(); setElementoArrastrado('musica'); }}
              style={{
                left: `${posMusica.x * 100}%`,
                top: `${posMusica.y * 100}%`,
                pointerEvents: 'auto',
                cursor: 'grab',
              }}
            >
              <div
                style={{
                  padding: '7px 16px',
                  borderRadius: 999,
                  background: 'rgba(10, 10, 14, 0.82)',
                  backdropFilter: 'blur(10px)',
                  color: '#ffffff',
                  fontWeight: 700,
                  fontSize: 13,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  border: '1.5px solid rgba(255,255,255,0.25)',
                  boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
                }}
              >
                <span>🎵 {musica.titulo}</span>
                <span className="ecualizador-mini">
                  <span className="barra b1" />
                  <span className="barra b2" />
                  <span className="barra b3" />
                </span>
              </div>
            </div>
          ) : null}

          {!texto && !stickerActivo && !musica ? (
            <span className="editor-ayuda">Escribe abajo o añade música, stickers y arrástralos</span>
          ) : null}
        </div>

        {/* Panel inferior de herramientas */}
        <div className="editor-panel-inferior">
          {/* Pestañas de herramientas */}
          <div className="editor-pestanas-herramientas">
            <button
              type="button"
              className={`editor-pestana-btn${pestana === 'texto' ? ' activa' : ''}`}
              onClick={() => setPestana('texto')}
            >
              ✏️ Texto
            </button>
            <button
              type="button"
              className={`editor-pestana-btn${pestana === 'musica' ? ' activa' : ''}`}
              onClick={() => setPestana('musica')}
            >
              🎵 Música
            </button>
            <button
              type="button"
              className={`editor-pestana-btn${pestana === 'estilo' ? ' activa' : ''}`}
              onClick={() => setPestana('estilo')}
            >
              🔤 Tipografía & Estilo
            </button>
            <button
              type="button"
              className={`editor-pestana-btn${pestana === 'fondo' ? ' activa' : ''}`}
              onClick={() => setPestana('fondo')}
            >
              🎨 Fondo / Filtro
            </button>
            <button
              type="button"
              className={`editor-pestana-btn${pestana === 'stickers' ? ' activa' : ''}`}
              onClick={() => setPestana('stickers')}
            >
              ✨ Stickers & Badges
            </button>
          </div>

          {/* Subpanel 1: Texto */}
          {pestana === 'texto' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <input
                className="editor-input"
                maxLength={220}
                placeholder="Escribe el texto de tu historia…"
                value={texto}
                onChange={(e) => setTexto(e.target.value)}
                autoFocus
              />
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div className="editor-subpanel">
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
                <div style={{ display: 'flex', gap: 4 }}>
                  {['left', 'center', 'right'].map((al) => (
                    <button
                      key={al}
                      type="button"
                      className={`editor-chip${alineacion === al ? ' activo' : ''}`}
                      onClick={() => setAlineacion(al)}
                    >
                      {al === 'left' ? '⇤' : al === 'center' ? '≡' : '⇥'}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Subpanel Música */}
          {pestana === 'musica' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span className="muted" style={{ fontSize: 11 }}>Selecciona una banda sonora para tu historia:</span>
                {musica ? (
                  <button
                    type="button"
                    className="btn-ghost btn-sm"
                    style={{ fontSize: 11, padding: '2px 8px', color: '#ef4444' }}
                    onClick={() => {
                      detenerMusica();
                      setMusica(null);
                    }}
                  >
                    ✕ Quitar música
                  </button>
                ) : null}
              </div>
              <div className="editor-subpanel">
                {PISTAS_MUSICA.map((p) => {
                  const seleccionada = musica?.id === p.id;
                  return (
                    <button
                      key={p.id}
                      type="button"
                      className={`editor-chip${seleccionada ? ' activo' : ''}`}
                      style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '6px 12px' }}
                      onClick={() => {
                        if (seleccionada) {
                          detenerMusica();
                          setMusica(null);
                        } else {
                          setMusica(p);
                          reproducirMusica(p.id);
                        }
                      }}
                    >
                      <span>{p.emoji}</span>
                      <div style={{ textAlign: 'left', lineHeight: 1.1 }}>
                        <div style={{ fontSize: 12, fontWeight: 700 }}>{p.titulo}</div>
                        <div style={{ fontSize: 10, opacity: 0.8 }}>{p.autor}</div>
                      </div>
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Subpanel 2: Tipografía & Estilo */}
          {pestana === 'estilo' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div className="editor-subpanel">
                {TIPOGRAFIAS.map((f) => (
                  <button
                    key={f.id}
                    type="button"
                    className={`editor-chip${tipoFuente === f.id ? ' activo' : ''}`}
                    onClick={() => setTipoFuente(f.id)}
                  >
                    {f.label}
                  </button>
                ))}
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8 }}>
                <div className="editor-subpanel">
                  {ESTILOS_TEXTO.map((est) => (
                    <button
                      key={est.id}
                      type="button"
                      className={`editor-chip${estiloTexto === est.id ? ' activo' : ''}`}
                      onClick={() => setEstiloTexto(est.id)}
                    >
                      {est.label}
                    </button>
                  ))}
                </div>
                <div className="editor-subpanel">
                  {TAMAÑOS.map((t) => (
                    <button
                      key={t.id}
                      type="button"
                      className={`editor-chip${tam === t.id ? ' activo' : ''}`}
                      onClick={() => setTam(t.id)}
                    >
                      {t.label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Subpanel 3: Fondo o Filtros */}
          {pestana === 'fondo' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <button
                  type="button"
                  className="editor-chip activo"
                  onClick={() => fileInputRef.current?.click()}
                >
                  📷 {urlImg ? 'Cambiar Foto' : 'Subir Foto'}
                </button>
                {urlImg ? (
                  <button
                    type="button"
                    className="editor-chip"
                    onClick={() => setUrlImg('')}
                  >
                    🎨 Usar Fondos Moon
                  </button>
                ) : null}
                <input
                  ref={fileInputRef}
                  type="file"
                  accept="image/*"
                  hidden
                  onChange={cambiarFoto}
                />
              </div>

              {urlImg ? (
                <div>
                  <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>Filtros para la imagen:</div>
                  <div className="editor-subpanel">
                    {FILTROS.map((fil) => (
                      <button
                        key={fil.id}
                        type="button"
                        className={`editor-chip${filtroId === fil.id ? ' activo' : ''}`}
                        onClick={() => setFiltroId(fil.id)}
                      >
                        {fil.label}
                      </button>
                    ))}
                  </div>
                </div>
              ) : (
                <div>
                  <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>Fondos degradados exclusivos:</div>
                  <div className="editor-subpanel">
                    {FONDOS_DEGRADADOS.map((fd, idx) => (
                      <button
                        key={fd.id}
                        type="button"
                        className={`editor-chip${fondoIndex === idx ? ' activo' : ''}`}
                        style={{
                          background: fd.css,
                          borderColor: fondoIndex === idx ? '#fff' : 'transparent',
                        }}
                        onClick={() => setFondoIndex(idx)}
                      >
                        {fd.label}
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Subpanel 4: Stickers & Badges */}
          {pestana === 'stickers' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {/* Emojis reactivos */}
              <div className="editor-subpanel">
                {STICKERS_EMOJIS.map((em) => (
                  <button
                    key={em}
                    type="button"
                    style={{
                      background: 'rgba(255,255,255,0.12)',
                      border: 0,
                      borderRadius: 10,
                      fontSize: 22,
                      padding: '4px 8px',
                      cursor: 'pointer',
                    }}
                    onClick={() => setStickerActivo({ tipo: 'emoji', valor: em })}
                  >
                    {em}
                  </button>
                ))}
              </div>

              {/* Badges interactivos */}
              <div className="editor-subpanel">
                <button
                  type="button"
                  className="editor-chip"
                  onClick={() => setStickerActivo({ tipo: 'badge', valor: `⏰ ${obtenerHoraActual()}` })}
                >
                  ⏰ Hora actual
                </button>
                <button
                  type="button"
                  className="editor-chip"
                  onClick={() => setStickerActivo({ tipo: 'badge', valor: '📍 En directo' })}
                >
                  📍 En directo
                </button>
                <button
                  type="button"
                  className="editor-chip"
                  onClick={() => setStickerActivo({ tipo: 'badge', valor: '💬 Pregúntame' })}
                >
                  💬 Pregúntame
                </button>
                <button
                  type="button"
                  className="editor-chip"
                  onClick={() => setStickerActivo({ tipo: 'badge', valor: '🌙 En órbita' })}
                >
                  🌙 En órbita
                </button>
                <button
                  type="button"
                  className="editor-chip"
                  onClick={() => setStickerActivo({ tipo: 'badge', valor: '🎵 Vibra Moon' })}
                >
                  🎵 Vibra Moon
                </button>
                {stickerActivo && (
                  <button
                    type="button"
                    className="editor-chip"
                    style={{ background: 'rgba(239, 68, 68, 0.3)', borderColor: '#ef4444' }}
                    onClick={() => setStickerActivo(null)}
                  >
                    ✕ Quitar
                  </button>
                )}
              </div>
            </div>
          )}

          {/* Acciones principales de publicación */}
          <div className="editor-acciones">
            <button type="button" className="btn" onClick={onCancelar} style={{ flex: '0 0 auto' }}>
              Cancelar
            </button>
            <button
              type="button"
              className="btn-aurora"
              onClick={publicar}
              style={{ flex: 1, padding: '10px 18px', fontWeight: 700 }}
            >
              Publicar historia ✨
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
