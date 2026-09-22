// Moon — Editor de historias profesional (suite creativa avanzada)
// ============================================================
// Barra de herramientas vertical en el lateral derecho (estilo Instagram/Facebook),
// música personalizada desde el teléfono con recorte de segundos y selector de duración,
// stickers, tipografías y composición Full HD.

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { IconX } from './Icons.jsx';
import { PISTAS_MUSICA, reproducirMusica, detenerMusica } from '../musicaHistorias.js';
import { uploadMedia } from '../api.js';
import { toast, avisoError } from '../ui.js';

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

function formatearSegundos(seg) {
  const m = Math.floor(seg / 60);
  const s = Math.floor(seg % 60);
  return `${m}:${s < 10 ? '0' : ''}${s}`;
}

export default function StoryEditor({ archivo, onCancelar, onListo }) {
  // Texto
  const [texto, setTexto] = useState('');
  const [tipoFuente, setTipoFuente] = useState('moderna');
  const [color, setColor] = useState('#ffffff');
  const [estiloTexto, setEstiloTexto] = useState('glass');
  const [tam, setTam] = useState('m');
  const [alineacion, setAlineacion] = useState('center');
  const [posTexto, setPosTexto] = useState({ x: 0.5, y: 0.45 });

  // Fondo / Imagen
  const [fondoIndex, setFondoIndex] = useState(0);
  const [filtroId, setFiltroId] = useState('normal');
  const [urlImg, setUrlImg] = useState('');

  // Sticker
  const [stickerActivo, setStickerActivo] = useState(null); // { tipo: 'emoji' | 'badge', valor: string }
  const [posSticker, setPosSticker] = useState({ x: 0.5, y: 0.72 });

  // Música personalizada
  const [musica, setMusica] = useState(null); // { tipo: 'archivo' | 'synth', file?: File, url?: string, titulo: string, inicio: number, duracion: number, totalDur: number }
  const [posMusica, setPosMusica] = useState({ x: 0.5, y: 0.18 });
  const [reproduciendoPrevia, setReproduciendoPrevia] = useState(false);

  // Herramienta activa en la barra lateral derecha (null = ninguna abierta)
  const [herramientaActiva, setHerramientaActiva] = useState(null); // 'texto' | 'musica' | 'fondo' | 'stickers' | 'estilo' | null
  const [elementoArrastrado, setElementoArrastrado] = useState(null); // 'texto' | 'sticker' | 'musica' | null
  const [publicando, setPublicando] = useState(false);

  const zonaRef = useRef(null);
  const fileInputRef = useRef(null);
  const audioInputRef = useRef(null);
  const audioPreviaRef = useRef(null);

  useEffect(() => {
    if (archivo) {
      const u = URL.createObjectURL(archivo);
      setUrlImg(u);
      return () => URL.revokeObjectURL(u);
    }
  }, [archivo]);

  useEffect(() => {
    return () => {
      detenerMusica();
      if (audioPreviaRef.current) {
        audioPreviaRef.current.pause();
        audioPreviaRef.current = null;
      }
    };
  }, []);

  function cambiarFoto(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    const u = URL.createObjectURL(file);
    setUrlImg(u);
  }

  function elegirAudioArchivo(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    const audioUrl = URL.createObjectURL(file);
    const audioTemp = new Audio(audioUrl);

    audioTemp.onloadedmetadata = () => {
      const duracionTotal = Math.floor(audioTemp.duration) || 60;
      const nombreLimpio = file.name.replace(/\.[^/.]+$/, '').slice(0, 50);

      detenerMusica();
      if (audioPreviaRef.current) audioPreviaRef.current.pause();

      setMusica({
        tipo: 'archivo',
        file,
        url: audioUrl,
        titulo: nombreLimpio,
        inicio: 0,
        duracion: 15,
        totalDur: duracionTotal,
      });
    };
  }

  function alternarPreviaAudio() {
    if (!musica) return;
    if (musica.tipo === 'synth') {
      if (reproduciendoPrevia) {
        detenerMusica();
        setReproduciendoPrevia(false);
      } else {
        reproducirMusica(musica.id);
        setReproduciendoPrevia(true);
      }
      return;
    }

    if (!audioPreviaRef.current) {
      audioPreviaRef.current = new Audio(musica.url);
    }
    const a = audioPreviaRef.current;
    if (reproduciendoPrevia) {
      a.pause();
      setReproduciendoPrevia(false);
    } else {
      a.currentTime = musica.inicio;
      a.play().catch(() => {});
      setReproduciendoPrevia(true);
      setTimeout(() => {
        if (audioPreviaRef.current) {
          audioPreviaRef.current.pause();
          setReproduciendoPrevia(false);
        }
      }, musica.duracion * 1000);
    }
  }

  const fuenteActual = useMemo(() => TIPOGRAFIAS.find((f) => f.id === tipoFuente) || TIPOGRAFIAS[0], [tipoFuente]);
  const rel = useMemo(() => (TAMAÑOS.find((t) => t.id === tam) || TAMAÑOS[1]).rel, [tam]);
  const filtroActual = useMemo(() => FILTROS.find((f) => f.id === filtroId) || FILTROS[0], [filtroId]);
  const fondoActual = useMemo(() => FONDOS_DEGRADADOS[fondoIndex] || FONDOS_DEGRADADOS[0], [fondoIndex]);

  function alArrastrar(e) {
    if (!elementoArrastrado || !zonaRef.current) return;
    const rect = zonaRef.current.getBoundingClientRect();
    const clientX = e.touches ? e.touches[0].clientX : e.clientX;
    const clientY = e.touches ? e.touches[0].clientY : e.clientY;
    const px = Math.min(0.95, Math.max(0.05, (clientX - rect.left) / rect.width));
    const py = Math.min(0.95, Math.max(0.08, (clientY - rect.top) / rect.height));

    if (elementoArrastrado === 'texto') setPosTexto({ x: px, y: py });
    else if (elementoArrastrado === 'sticker') setPosSticker({ x: px, y: py });
    else if (elementoArrastrado === 'musica') setPosMusica({ x: px, y: py });
  }

  async function publicar() {
    setPublicando(true);
    detenerMusica();
    if (audioPreviaRef.current) audioPreviaRef.current.pause();

    try {
      let finalMusicUrl = '';
      let finalMusicTitle = '';
      let finalMusicStart = 0;
      let finalMusicDuration = 15;

      if (musica) {
        finalMusicTitle = musica.titulo;
        finalMusicStart = musica.inicio || 0;
        finalMusicDuration = musica.duracion || 15;

        if (musica.tipo === 'archivo' && musica.file) {
          toast.info('Subiendo música de tu teléfono…');
          const subidaAudio = await uploadMedia('audio', musica.file);
          finalMusicUrl = subidaAudio.url;
        } else if (musica.tipo === 'synth') {
          finalMusicUrl = `synth:${musica.id}`;
        }
      }

      const W = 1080;
      const H = 1920;
      const canvas = document.createElement('canvas');
      canvas.width = W;
      canvas.height = H;
      const ctx = canvas.getContext('2d');

      const renderContenidoFinal = (imgFondo) => {
        // 1. Fondo
        if (imgFondo) {
          if (filtroActual.filter !== 'none') ctx.filter = filtroActual.filter;
          const scale = Math.max(W / imgFondo.naturalWidth, H / imgFondo.naturalHeight);
          const x = (W - imgFondo.naturalWidth * scale) / 2;
          const y = (H - imgFondo.naturalHeight * scale) / 2;
          ctx.drawImage(imgFondo, x, y, imgFondo.naturalWidth * scale, imgFondo.naturalHeight * scale);
          ctx.filter = 'none';

          const gradVignette = ctx.createLinearGradient(0, 0, 0, H);
          gradVignette.addColorStop(0, 'rgba(0,0,0,0.3)');
          gradVignette.addColorStop(0.3, 'rgba(0,0,0,0)');
          gradVignette.addColorStop(0.7, 'rgba(0,0,0,0)');
          gradVignette.addColorStop(1, 'rgba(0,0,0,0.4)');
          ctx.fillStyle = gradVignette;
          ctx.fillRect(0, 0, W, H);
        } else {
          const grad = ctx.createLinearGradient(0, 0, W, H);
          grad.addColorStop(0, fondoActual.colors[0]);
          grad.addColorStop(0.5, fondoActual.colors[1]);
          grad.addColorStop(1, fondoActual.colors[2]);
          ctx.fillStyle = grad;
          ctx.fillRect(0, 0, W, H);
        }

        // 2. Sticker o Badge
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

        // 2.5 Sticker de Música
        if (musica) {
          const mx = posMusica.x * W;
          const my = posMusica.y * H;
          ctx.font = '700 32px -apple-system, sans-serif';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          const txt = `🎵 ${musica.titulo}`;
          const tw = ctx.measureText(txt).width;
          const bw = tw + 60;
          const bh = 64;
          const x0 = mx - bw / 2;
          const y0 = my - bh / 2;

          ctx.fillStyle = 'rgba(10, 10, 14, 0.82)';
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

        // 3. Texto
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

        // 4. Exportar
        canvas.toBlob((blob) => {
          const file = new File([blob], 'historia.jpg', { type: 'image/jpeg' });
          onListo(file, texto.trim(), finalMusicTitle, finalMusicUrl, finalMusicStart, finalMusicDuration);
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
    } catch (e) {
      avisoError(e);
      setPublicando(false);
    }
  }

  return (
    <div className="editor-historia" role="dialog" aria-modal="true" aria-label="Editor de historias">
      <div className="editor-caja">
        {/* Cabecera superior */}
        <header className="editor-cabecera">
          <b>Crear historia</b>
          <button type="button" onClick={onCancelar} aria-label="Cerrar">
            <IconX />
          </button>
        </header>

        {/* BARRA DE HERRAMIENTAS VERTICAL A LA DERECHA */}
        <div className="editor-barra-vertical">
          <button
            type="button"
            className={`editor-btn-vertical${herramientaActiva === 'texto' ? ' activo' : ''}`}
            onClick={() => setHerramientaActiva(herramientaActiva === 'texto' ? null : 'texto')}
            title="Escribir texto"
          >
            <span>🔤</span>
            <span className="btn-label">Texto</span>
          </button>

          <button
            type="button"
            className={`editor-btn-vertical${herramientaActiva === 'musica' ? ' activo' : ''}`}
            onClick={() => setHerramientaActiva(herramientaActiva === 'musica' ? null : 'musica')}
            title="Añadir música"
          >
            <span>🎵</span>
            <span className="btn-label">Música</span>
          </button>

          <button
            type="button"
            className={`editor-btn-vertical${herramientaActiva === 'fondo' ? ' activo' : ''}`}
            onClick={() => setHerramientaActiva(herramientaActiva === 'fondo' ? null : 'fondo')}
            title="Fondo o foto"
          >
            <span>🎨</span>
            <span className="btn-label">Fondo</span>
          </button>

          <button
            type="button"
            className={`editor-btn-vertical${herramientaActiva === 'stickers' ? ' activo' : ''}`}
            onClick={() => setHerramientaActiva(herramientaActiva === 'stickers' ? null : 'stickers')}
            title="Stickers y Badges"
          >
            <span>✨</span>
            <span className="btn-label">Sticker</span>
          </button>

          <button
            type="button"
            className={`editor-btn-vertical${herramientaActiva === 'estilo' ? ' activo' : ''}`}
            onClick={() => setHerramientaActiva(herramientaActiva === 'estilo' ? null : 'estilo')}
            title="Tipografía y estilo"
          >
            <span>📐</span>
            <span className="btn-label">Estilo</span>
          </button>
        </div>

        {/* Lienzo central */}
        <div
          ref={zonaRef}
          className="editor-lienzo"
          onClick={() => {
            // Si hay un panel abierto y tocas el lienzo, lo cierra para dar vista limpia
            if (herramientaActiva) setHerramientaActiva(null);
          }}
          onPointerMove={alArrastrar}
          onPointerUp={() => setElementoArrastrado(null)}
          onPointerCancel={() => setElementoArrastrado(null)}
        >
          {urlImg ? (
            <img
              src={urlImg}
              alt="Lienzo"
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

          {/* Sticker arrastrable */}
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
            <span className="editor-ayuda">Toca las herramientas de la derecha para personalizar tu historia</span>
          ) : null}
        </div>

        {/* Barra inferior fija con el botón publicar */}
        <div className="editor-barra-inferior-fija">
          <button type="button" className="btn" onClick={onCancelar}>
            Cancelar
          </button>
          <button
            type="button"
            className="btn-aurora"
            onClick={publicar}
            disabled={publicando}
            style={{ padding: '10px 22px', fontWeight: 700 }}
          >
            {publicando ? 'Publicando…' : 'Publicar historia ✨'}
          </button>
        </div>

        {/* PANELES EMERGENTES (Se abren desde la barra vertical) */}

        {/* Panel 1: Texto */}
        {herramientaActiva === 'texto' && (
          <div className="editor-panel-emergente">
            <div className="editor-panel-header">
              <b>Escribir texto</b>
              <button type="button" onClick={() => setHerramientaActiva(null)}>✕</button>
            </div>
            <input
              className="editor-input"
              maxLength={220}
              placeholder="¿Qué estás pensando?"
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

        {/* Panel 2: Música (desde teléfono con recorte de segundos) */}
        {herramientaActiva === 'musica' && (
          <div className="editor-panel-emergente">
            <div className="editor-panel-header">
              <b>🎵 Música de la historia</b>
              <button type="button" onClick={() => setHerramientaActiva(null)}>✕</button>
            </div>

            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <button
                type="button"
                className="btn-aurora btn-sm"
                style={{ flex: 1, padding: '10px' }}
                onClick={() => audioInputRef.current?.click()}
              >
                📁 Elegir Canción de mi Teléfono
              </button>
              <input
                ref={audioInputRef}
                type="file"
                accept="audio/*"
                hidden
                onChange={elegirAudioArchivo}
              />
              {musica && (
                <button
                  type="button"
                  className="btn-ghost btn-sm"
                  style={{ color: '#ef4444' }}
                  onClick={() => {
                    detenerMusica();
                    if (audioPreviaRef.current) audioPreviaRef.current.pause();
                    setMusica(null);
                    setReproduciendoPrevia(false);
                  }}
                >
                  ✕ Quitar
                </button>
              )}
            </div>

            {/* Recorte personalizado si seleccionó música del teléfono */}
            {musica && musica.tipo === 'archivo' && (
              <div className="musica-recorte-card">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <b>{musica.titulo}</b>
                  <button
                    type="button"
                    className="btn-ghost btn-sm"
                    onClick={alternarPreviaAudio}
                  >
                    {reproduciendoPrevia ? '⏸️ Pausar' : '▶️ Escuchar'}
                  </button>
                </div>

                <div className="musica-slider-fila">
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }} className="muted">
                    <span>Inicio: {formatearSegundos(musica.inicio)}</span>
                    <span>Total: {formatearSegundos(musica.totalDur)}</span>
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={Math.max(0, musica.totalDur - musica.duracion)}
                    value={musica.inicio}
                    onChange={(e) => {
                      const nuevoInicio = parseInt(e.target.value, 10);
                      setMusica({ ...musica, inicio: nuevoInicio });
                      if (audioPreviaRef.current) audioPreviaRef.current.currentTime = nuevoInicio;
                    }}
                  />
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span className="muted" style={{ fontSize: 12 }}>Duración:</span>
                  {[5, 10, 15, 30].map((d) => (
                    <button
                      key={d}
                      type="button"
                      className={`editor-chip${musica.duracion === d ? ' activo' : ''}`}
                      onClick={() => setMusica({ ...musica, duracion: d })}
                    >
                      {d}s
                    </button>
                  ))}
                </div>
              </div>
            )}

            {/* Opciones sintetizadas Moon */}
            <div>
              <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>O vibras musicales Moon:</div>
              <div className="editor-subpanel">
                {PISTAS_MUSICA.map((p) => {
                  const seleccionada = musica?.id === p.id;
                  return (
                    <button
                      key={p.id}
                      type="button"
                      className={`editor-chip${seleccionada ? ' activo' : ''}`}
                      onClick={() => {
                        if (seleccionada) {
                          detenerMusica();
                          setMusica(null);
                        } else {
                          detenerMusica();
                          if (audioPreviaRef.current) audioPreviaRef.current.pause();
                          setMusica({
                            tipo: 'synth',
                            id: p.id,
                            titulo: p.titulo,
                            inicio: 0,
                            duracion: 15,
                          });
                          reproducirMusica(p.id);
                        }
                      }}
                    >
                      {p.emoji} {p.titulo}
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        )}

        {/* Panel 3: Fondo o Foto */}
        {herramientaActiva === 'fondo' && (
          <div className="editor-panel-emergente">
            <div className="editor-panel-header">
              <b>Fondo y Filtros</b>
              <button type="button" onClick={() => setHerramientaActiva(null)}>✕</button>
            </div>
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
                <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>Filtros para la foto:</div>
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
                <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>Fondos degradados:</div>
                <div className="editor-subpanel">
                  {FONDOS_DEGRADADOS.map((fd, idx) => (
                    <button
                      key={fd.id}
                      type="button"
                      className={`editor-chip${fondoIndex === idx ? ' activo' : ''}`}
                      style={{ background: fd.css, borderColor: fondoIndex === idx ? '#fff' : 'transparent' }}
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

        {/* Panel 4: Stickers */}
        {herramientaActiva === 'stickers' && (
          <div className="editor-panel-emergente">
            <div className="editor-panel-header">
              <b>Stickers & Emojis</b>
              <button type="button" onClick={() => setHerramientaActiva(null)}>✕</button>
            </div>
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

        {/* Panel 5: Estilo */}
        {herramientaActiva === 'estilo' && (
          <div className="editor-panel-emergente">
            <div className="editor-panel-header">
              <b>Tipografía y estilo</b>
              <button type="button" onClick={() => setHerramientaActiva(null)}>✕</button>
            </div>
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
      </div>
    </div>
  );
}
