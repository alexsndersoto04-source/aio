// Moon — el logo
// ============================================================
// Diseño 100% de líneas y trazos:
// 1. Media luna icónica (estilo cuarto creciente clásico) en trazo de líneas.
// 2. Aro continuo o segmentado alrededor en movimiento de giro.
// 3. Subrayado limpio debajo.

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
      {/* 1. Aro exterior en movimiento continuo, solo trazo de líneas */}
      <g className="logo-moon__aro">
        <ellipse
          cx="60"
          cy="52"
          rx="47"
          ry="33"
          {...trazo}
          strokeWidth="2.4"
          strokeDasharray="60 30 50 30"
          transform="rotate(-20 60 52)"
        />
      </g>

      {/* 2. Media luna clásica en forma de C / cuarto creciente, solo de líneas */}
      <path
        d="M 68 20 A 32 32 0 1 0 68 84 A 25 25 0 0 1 68 20 Z"
        {...trazo}
        strokeWidth="3.2"
      />

      {/* 3. Subrayado de líneas en la base */}
      <line
        x1="34"
        y1="106"
        x2="86"
        y2="106"
        {...trazo}
        strokeWidth="3.2"
      />
      <line
        x1="46"
        y1="112"
        x2="74"
        y2="112"
        {...trazo}
        strokeWidth="2"
        opacity="0.6"
      />
    </svg>
  );
}
