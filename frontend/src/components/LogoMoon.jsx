// Moon — el logo
// ============================================================
// 1. Media luna: dibujada SOLO de líneas (contorno hueco, sin relleno sólido),
//    con el color morado de la marca (#4f46e5).
// 2. Aro: una elipse/anillo de líneas que rodea la luna y GIRA en movimiento continuo.
// 3. Subrayado: línea de trazo recto en la base.

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
      {/* Aro en movimiento continuo de rotación */}
      <g className="logo-moon__aro">
        <ellipse
          cx="60"
          cy="50"
          rx="48"
          ry="32"
          {...trazo}
          strokeWidth="2.8"
          strokeDasharray="65 30 50 30"
          transform="rotate(-22 60 50)"
        />
      </g>

      {/* Media luna en cuarto creciente, SOLO DE LÍNEAS (sin relleno sólido) */}
      <path
        d="M 68 18 A 32 32 0 1 0 68 82 A 25 25 0 0 1 68 18 Z"
        {...trazo}
        strokeWidth="3.4"
      />

      {/* Subrayado de líneas en la base */}
      <line
        x1="32"
        y1="106"
        x2="88"
        y2="106"
        {...trazo}
        strokeWidth="3.4"
      />
      <line
        x1="45"
        y1="112"
        x2="75"
        y2="112"
        {...trazo}
        strokeWidth="2.2"
        opacity="0.6"
      />
    </svg>
  );
}
