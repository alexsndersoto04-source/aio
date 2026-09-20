// Moon — el logo
// ============================================================
// Una media luna del color de la marca y, alrededor, un aro dibujado solo con
// líneas (no es un círculo completo: tiene sus huecos) que gira despacio, más
// un subrayado fino debajo. Sin rellenos de más ni brillos: la luna y las
// líneas, nada más.
//
// Las tres variantes (a, b, c) existen para poder compararlas; la que se usa
// en la pantalla de inicio es la «a».

import React, { useId } from 'react';

export default function LogoMoon({ variante = 'a', tamano = 96, titulo = 'Moon' }) {
  // Cada logo necesita su propio nombre de máscara: si no, dos logos en la
  // misma pantalla se pisarían el recorte.
  const id = `luna-${String(useId()).replace(/[^a-zA-Z0-9]/g, '')}`;
  const linea = {
    fill: 'none',
    stroke: 'currentColor',
    strokeLinecap: 'round',
  };

  return (
    <svg
      className={`logo-moon logo-moon--${variante}`}
      width={tamano}
      height={tamano}
      viewBox="0 0 120 120"
      role="img"
      aria-label={titulo}
    >
      <defs>
        <mask id={id}>
          <rect width="120" height="120" fill="#000" />
          <circle cx="60" cy="56" r="29" fill="#fff" />
          <circle cx="75" cy="43" r="26.5" fill="#000" />
        </mask>
      </defs>

      {/* El aro: solo líneas, con huecos, girando sin parar. */}
      <g className="logo-moon__aro">
        {variante === 'b' ? (
          <>
            <circle cx="60" cy="56" r="43" {...linea} strokeWidth="2.6" strokeDasharray="206 64" />
            <circle cx="60" cy="56" r="43" {...linea} strokeWidth="2.6" strokeDasharray="7 263" strokeDashoffset="-238" />
          </>
        ) : variante === 'c' ? (
          <>
            <circle cx="60" cy="56" r="40" {...linea} strokeWidth="2.2" strokeDasharray="52 32 44 32 40 32" />
            <circle cx="60" cy="56" r="48" {...linea} strokeWidth="1.6" strokeDasharray="90 40 70 40" opacity="0.7" className="logo-moon__aro-exterior" />
          </>
        ) : (
          <circle cx="60" cy="56" r="43" {...linea} strokeWidth="2.6" strokeDasharray="60 37 52 37 48 36.2" />
        )}
      </g>

      {/* La media luna, del color de la marca. */}
      <circle cx="60" cy="56" r="29" fill="currentColor" mask={`url(#${id})`} />

      {/* El subrayado. */}
      {variante === 'b' ? (
        <>
          <line x1="30" y1="111" x2="90" y2="111" {...linea} strokeWidth="1.6" opacity="0.75" />
          <line x1="47" y1="116" x2="73" y2="116" {...linea} strokeWidth="3" />
        </>
      ) : (
        <line x1="40" y1="112" x2="80" y2="112" {...linea} strokeWidth={variante === 'c' ? 3 : 4} />
      )}
    </svg>
  );
}
