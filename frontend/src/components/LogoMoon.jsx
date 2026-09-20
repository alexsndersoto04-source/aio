// Moon — el logo
// ============================================================
// Diseño en estilo lineal (outline / solo de líneas y subrayado):
// 1. Media luna: dibujada solo con trazos de líneas, sin relleno sólido, del mismo color del orbe.
// 2. Aro: solo de líneas, rodeando e inclinando la luna, con animación de giro continuo.
// 3. Subrayado: línea estilizada en la base.

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
        <ellipse
          cx="60"
          cy="52"
          rx="46"
          ry="32"
          {...trazo}
          strokeWidth="2.4"
          strokeDasharray="65 30 50 30"
          transform="rotate(-20 60 52)"
        />
      </g>

      {/* 2. Media luna: diseño SOLO DE LÍNEAS (contorno visible, sin relleno) */}
      <path
        d="M 60 22 A 30 30 0 0 0 60 82 A 23 23 0 0 1 60 22 Z"
        {...trazo}
        strokeWidth="3.2"
      />

      {/* 3. Subrayado elegante en la base */}
      <line
        x1="36"
        y1="108"
        x2="84"
        y2="108"
        {...trazo}
        strokeWidth="3"
      />
    </svg>
  );
}
