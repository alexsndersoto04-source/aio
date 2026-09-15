// Moon — Notas de voz
// ============================================================
// Dos piezas pequeñas:
//   · Grabador    → botón de micrófono que graba con el teléfono, muestra el
//                   tiempo y las barras, y devuelve el audio al terminar.
//   · AudioMensaje → reproductor de una nota ya enviada (play, avance, duración).
//
// Nada de librerías: se usa MediaRecorder, que ya viene en el navegador, y el
// elemento <audio>. Si el navegador no sabe grabar, el botón simplemente no
// aparece y todo lo demás sigue funcionando.

import React, { useEffect, useRef, useState } from 'react';
import { IconMic, IconPlay, IconPause, IconTrash, IconSend, IconWarning } from './Icons.jsx';

/** Tope por nota de voz: 5 minutos (el servidor admite hasta 10). */
export const MAX_NOTA_MS = 5 * 60 * 1000;
const AVISO_TOPE_MS = MAX_NOTA_MS - 30 * 1000;

const CANDIDATOS = [
  'audio/webm;codecs=opus',
  'audio/webm',
  'audio/mp4',
  'audio/mp4;codecs=mp4a.40.2',
  'audio/ogg;codecs=opus',
];

/** ¿Se puede grabar en este navegador? */
export function puedeGrabar() {
  return typeof navigator !== 'undefined'
    && !!navigator.mediaDevices
    && typeof navigator.mediaDevices.getUserMedia === 'function'
    && typeof window.MediaRecorder === 'function';
}

function mejorFormato() {
  if (typeof window === 'undefined' || typeof window.MediaRecorder !== 'function') return '';
  for (const tipo of CANDIDATOS) {
    try { if (MediaRecorder.isTypeSupported(tipo)) return tipo; } catch { /* siguiente */ }
  }
  return '';
}

function segundos(ms) {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/**
 * Grabador de notas de voz.
 * `onListo(blob, duracionMs)` se llama al terminar; `onCancelar()` al descartar.
 */
export function Grabador({ onListo, onCancelar, disabled = false }) {
  const [grabando, setGrabando] = useState(false);
  const [ms, setMs] = useState(0);
  const [error, setError] = useState('');
  const grabadora = useRef(null);
  const trozos = useRef([]);
  const inicio = useRef(0);
  const reloj = useRef(null);
  const flujo = useRef(null);

  useEffect(() => () => {
    if (reloj.current) clearInterval(reloj.current);
    if (flujo.current) flujo.current.getTracks().forEach((t) => t.stop());
  }, []);

  async function empezar() {
    setError('');
    if (!puedeGrabar()) {
      setError('Este navegador no puede grabar audio. Prueba a abrir Moon en Chrome o Safari.');
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      flujo.current = stream;
      trozos.current = [];
      const formato = mejorFormato();
      const rec = new MediaRecorder(stream, formato ? { mimeType: formato } : undefined);
      rec.ondataavailable = (e) => { if (e.data && e.data.size > 0) trozos.current.push(e.data); };
      rec.onstop = () => {
        const dur = Date.now() - inicio.current;
        stream.getTracks().forEach((t) => t.stop());
        flujo.current = null;
        if (reloj.current) clearInterval(reloj.current);
        setGrabando(false);
        setMs(0);
        const blob = new Blob(trozos.current, { type: rec.mimeType || 'audio/webm' });
        trozos.current = [];
        if (dur < 700 || blob.size === 0) {
          setError('La nota quedó muy corta: mantén pulsado y habla un momento.');
          return;
        }
        onListo(blob, dur);
      };
      rec.onerror = () => {
        setError('No se pudo grabar. Revisa el permiso del micrófono.');
        try { rec.stop(); } catch { /* ya estaba parada */ }
      };
      grabadora.current = rec;
      inicio.current = Date.now();
      rec.start();
      setGrabando(true);
      setMs(0);
      reloj.current = setInterval(() => {
        const llevo = Date.now() - inicio.current;
        setMs(llevo);
        // A los 5 minutos se corta sola: así nunca se pierde la nota por ser larga.
        if (llevo >= MAX_NOTA_MS) terminar();
      }, 120);
    } catch (e) {
      const permiso = e && (e.name === 'NotAllowedError' || e.name === 'SecurityError');
      setError(permiso ? 'Para grabar hay que dar permiso al micrófono.' : 'No se pudo abrir el micrófono.');
    }
  }

  function terminar() {
    try { grabadora.current && grabadora.current.stop(); } catch { /* nada */ }
  }

  function cancelar() {
    if (reloj.current) clearInterval(reloj.current);
    if (flujo.current) flujo.current.getTracks().forEach((t) => t.stop());
    flujo.current = null;
    setGrabando(false);
    setMs(0);
    trozos.current = [];
    if (grabadora.current) {
      grabadora.current.onstop = null;
      try { grabadora.current.stop(); } catch { /* nada */ }
    }
    if (onCancelar) onCancelar();
  }

  if (grabando) {
    return (
      <div className="grabador" role="status" aria-live="polite">
        <span className="punto-rojo" aria-hidden="true" />
        <span className="tiempo">{segundos(ms)}</span>
        {ms >= AVISO_TOPE_MS ? (
          <span className="aviso-tope" role="status">termina en {segundos(MAX_NOTA_MS - ms)}</span>
        ) : null}
        <span className="ondas" aria-hidden="true">
          {Array.from({ length: 14 }).map((_, i) => (
            <i key={i} style={{ animationDelay: `${i * 70}ms` }} />
          ))}
        </span>
        <button type="button" className="btn-grabar cancelar" onClick={cancelar} aria-label="Descartar la nota de voz" title="Descartar">
          <IconTrash />
        </button>
        <button type="button" className="btn-grabar enviar" onClick={terminar} aria-label="Enviar la nota de voz" title="Enviar">
          <IconSend />
        </button>
      </div>
    );
  }

  return (
    <div className="grabador-caja">
      <button
        type="button"
        className="btn-grabar"
        onClick={empezar}
        disabled={disabled}
        aria-label="Grabar una nota de voz"
        title="Grabar nota de voz"
      >
        <IconMic />
      </button>
      {error ? (
        <span className="error-nota">
          <IconWarning /> {error}
        </span>
      ) : null}
    </div>
  );
}

/** Reproductor de una nota de voz ya enviada. */
export function AudioMensaje({ url, duracionMs = 0, mio = false }) {
  const audio = useRef(null);
  const [sonando, setSonando] = useState(false);
  const [avance, setAvance] = useState(0);
  const [total, setTotal] = useState(Number(duracionMs) || 0);
  const [fallo, setFallo] = useState(false);

  useEffect(() => {
    setFallo(false);
    setAvance(0);
    setSonando(false);
  }, [url]);

  function alternar() {
    const el = audio.current;
    if (!el) return;
    if (sonando) {
      el.pause();
      return;
    }
    el.play().catch(() => setFallo(true));
  }

  const duracion = total || Number(duracionMs) || 0;
  const porcentaje = duracion > 0 ? Math.min(100, (avance / duracion) * 100) : 0;

  return (
    <div className={`nota-voz${mio ? ' mia' : ''}${fallo ? ' fallo' : ''}`}>
      <audio
        ref={audio}
        src={url}
        preload="metadata"
        onPlay={() => setSonando(true)}
        onPause={() => setSonando(false)}
        onEnded={() => { setSonando(false); setAvance(0); }}
        onTimeUpdate={(e) => setAvance(e.currentTarget.currentTime * 1000)}
        onLoadedMetadata={(e) => {
          const d = e.currentTarget.duration;
          if (Number.isFinite(d) && d > 0) setTotal(d * 1000);
        }}
        onError={() => setFallo(true)}
      />
      <button type="button" className="btn-play" onClick={alternar} aria-label={sonando ? 'Pausar la nota de voz' : 'Reproducir la nota de voz'}>
        {sonando ? <IconPause /> : <IconPlay />}
      </button>
      <span className="barra-audio" aria-hidden="true">
        <i style={{ width: `${porcentaje}%` }} />
      </span>
      <span className="tiempo-audio">{fallo ? 'No se pudo cargar' : segundos(sonando || avance > 0 ? avance : duracion)}</span>
    </div>
  );
}
