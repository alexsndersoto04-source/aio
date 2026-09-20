// Moon — el logo
// ============================================================
// Diseño en estilo lineal (outline / solo de líneas y subrayado):
// 1. La media luna dibujada SOLO con línea de contorno (sin relleno sólido).
// 2. Un aro alrededor en trazo lineal segmentado que gira en movimiento continuo.
// 3. Una línea de subrayado elegante en la base.
// Todo con el color característico de la marca Moon.

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
      {/* 1. Aro exterior en movimiento continuo, solo trazo de líneas segmentado */}
      <g className="logo-moon__aro">
        <circle
          cx="60"
          cy="54"
          r="44"
          {...trazo}
          strokeWidth="2.5"
          strokeDasharray="55 35 45 35 40 30"
        />
      </g>

      {/* 2. Media luna: diseño SOLO DE LÍNEAS (contorno hueco, sin relleno sólido) */}
      <path
        d="M 60 25 A 29 29 0 0 0 60 83 A 24 24 0 0 1 60 25 Z"
        {...trazo}
        strokeWidth="3.2"
      />

      {/* 3. Subrayado de líneas en la base */}
      <line
        x1="38"
        y1="110"
        x2="82"
        y2="110"
        {...trazo}
        strokeWidth="3"
      />
      <line
        x1="48"
        y1="115"
        x2="72"
        y2="115"
        {...trazo}
        strokeWidth="1.8"
        opacity="0.6"
      />
    </svg>
  );
}
