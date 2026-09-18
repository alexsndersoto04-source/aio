// Moon — Imagen con carga progresiva
// ============================================================
// El problema que resuelve: cuando la foto llega tarde, la pantalla «salta»
// porque nadie sabía cuánto iba a ocupar, y la imagen aparece de golpe.
//
// Aquí la caja ya nace con su proporción definitiva (nada se mueve al cargar),
// se pinta con un fondo suave propio de cada foto y la imagen entra desenfocada
// y un poco más grande, para aterrizar nítida con un fundido corto. Es el mismo
// truco que usan las apps grandes; el usuario no lo nombra, pero lo nota.

import React, { useEffect, useRef, useState } from 'react';
import { imgUrl } from '../api.js';

/** Paleta de fondos suaves: cada foto toma el suyo según su dirección. */
const TONOS = [
  'linear-gradient(135deg, #e8e8ee 0%, #d6d6de 100%)',
  'linear-gradient(135deg, #eae6f3 0%, #d8d2e8 100%)',
  'linear-gradient(135deg, #e3edf2 0%, #cfe1ea 100%)',
  'linear-gradient(135deg, #f0e9e4 0%, #e0d5cc 100%)',
  'linear-gradient(135deg, #e6efe8 0%, #d2e2d7 100%)',
  'linear-gradient(135deg, #f2e8ec 0%, #e3d3da 100%)',
];

/** Número estable a partir de una cadena (para repartir tonos sin aleatoriedad). */
function semilla(texto = '') {
  let n = 0;
  for (let i = 0; i < texto.length; i += 1) n = (n * 31 + texto.charCodeAt(i)) % 100000;
  return n;
}

/**
 * @param {string} src    dirección de la imagen (relativa o absoluta)
 * @param {string} ratio  proporción de la caja («4 / 5», «1 / 1», «auto»)
 * @param {string} alt    texto alternativo
 */
export default function Imagen({ src, alt = '', ratio = '4 / 5', className = '', onClick }) {
  const url = imgUrl(src);
  const [estado, setEstado] = useState('cargando');
  const ref = useRef(null);

  // Si la foto ya estaba en la caché, el evento onLoad no vuelve a dispararse:
  // se comprueba al montar para no quedarse en el estado «cargando».
  useEffect(() => {
    setEstado('cargando');
    const nodo = ref.current;
    if (nodo?.complete && nodo.naturalWidth > 0) setEstado('lista');
  }, [url]);

  return (
    <div
      className={`foto ${estado === 'lista' ? 'foto-lista' : ''} ${estado === 'fallo' ? 'foto-fallo' : ''} ${className}`.trim()}
      style={{ '--ratio': ratio, '--tono': TONOS[semilla(url) % TONOS.length] }}
      onClick={onClick}
    >
      {estado === 'fallo' ? (
        <span className="foto-aviso" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <rect x="3" y="4" width="18" height="16" rx="2.5" />
            <path d="m4 16 4.5-4.5 4 4L16 12l4 4" />
            <circle cx="9" cy="9" r="1.4" />
          </svg>
        </span>
      ) : null}
      <img
        ref={ref}
        src={url}
        alt={alt}
        loading="lazy"
        decoding="async"
        onLoad={() => setEstado('lista')}
        onError={() => setEstado('fallo')}
      />
    </div>
  );
}
