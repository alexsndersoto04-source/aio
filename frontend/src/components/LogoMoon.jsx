// Moon — el logo
// ============================================================
// Diseño profesional de alta gama para Moon:
// - Media luna estilizada, geométrica y precisa, solo de líneas finas.
// - Anillo orbital elíptico en perspectiva 3D que la rodea limpiamente.
// - El aro gira suave y continuo.
// - Subrayado de base sobrio, simétrico y elegante.

import React from 'react';

export default function LogoMoon({ tamano = 96, titulo = 'Moon' }) {
  const trazo = {
    fill: 'none',
    stroke: 'currentColor',
    strokeLinecap: 'round',
    strokeLinejoin: 'round',
  };

  return (
    <svg
      className="logo-moon"
      width={tamano}
      height={tamano}
      viewBox="0 0 120 120"
      role="img"
      aria-label={titulo}
    >
      <defs>
        {/* Degradado sutil para el brillo orbital */}
        <linearGradient id="lunaGrad" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="currentColor" stopOpacity="1" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0.75" />
        </linearGradient>
      </defs>

      {/* 1. Aro orbital en perspectiva astronómica, con giro suave */}
      <g className="logo-moon__aro">
        <ellipse
          cx="60"
          cy="52"
          rx="47"
          ry="19"
          {...trazo}
          strokeWidth="2"
          strokeDasharray="95 18 25 18"
          transform="rotate(-26 60 52)"
        />
      </g>

      {/* 2. Media luna con geometría pura y curva suave (cuarto creciente pro) */}
      <path
        d="M 64 24 C 44 24 33 38 33 52 C 33 66 44 80 64 80 C 49 76 43 65 43 52 C 43 39 49 28 64 24 Z"
        fill="none"
        stroke="url(#lunaGrad)"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />

      {/* 3. Subrayado arquitectónico minimalista */}
      <line
        x1="44"
        y1="98"
        x2="76"
        y2="98"
        {...trazo}
        strokeWidth="2.2"
      />
      <circle
        cx="60"
        cy="104"
        r="1.2"
        fill="currentColor"
        opacity="0.8"
      />
    </svg>
  );
}
