// Moon — el logo
// ============================================================
// Diseño definitivo inspirado en el vector aprobado:
// 1. Media luna: fija (NO se mueve), trazo fino de contorno neón morado/índigo, sin relleno sólido.
// 2. Aro orbital: en perspectiva 3D inclinada cruzando la luna, con trazo punteado/segmentado
//    y animación de rotación continua y fluida que roba la atención.
// 3. Subrayado: línea horizontal minimalista y centrada en la base.

import React from 'react';

export default function LogoMoon({ tamano = 110, titulo = 'Moon' }) {
  return (
    <svg
      className="logo-moon"
      width={tamano}
      height={tamano}
      viewBox="0 0 160 160"
      role="img"
      aria-label={titulo}
    >
      <defs>
        {/* Gradiente neón de marca para los trazos */}
        <linearGradient id="neonMoonGrad" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#c084fc" />
          <stop offset="50%" stopColor="#818cf8" />
          <stop offset="100%" stopColor="#4f46e5" />
        </linearGradient>

        <linearGradient id="neonRingGrad" x1="100%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stopColor="#e879f9" />
          <stop offset="60%" stopColor="#818cf8" />
          <stop offset="100%" stopColor="#38bdf8" />
        </linearGradient>

        {/* Resplandor sutil estilo neón */}
        <filter id="neonGlow" x="-20%" y="-20%" width="140%" height="140%">
          <feDropShadow dx="0" dy="0" stdDeviation="2.5" floodColor="#818cf8" floodOpacity="0.45" />
        </filter>
      </defs>

      {/* 1. MEDIA LUNA (COMPLETAMENTE FIJA, NO SE MUEVE) */}
      <g className="logo-moon__cuerpo" filter="url(#neonGlow)">
        <path
          d="M 86 32 C 60 32 46 50 46 72 C 46 94 62 112 90 112 C 68 106 58 90 58 72 C 58 54 68 38 86 32 Z"
          fill="none"
          stroke="url(#neonMoonGrad)"
          strokeWidth="3.2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </g>

      {/* 2. ARO ORBITAL EN MOVIMIENTO (GIRA CONTINUO ALREDEDOR DE LA LUNA) */}
      <g className="logo-moon__aro-wrap">
        <g className="logo-moon__aro" filter="url(#neonGlow)">
          <ellipse
            cx="80"
            cy="72"
            rx="66"
            ry="24"
            fill="none"
            stroke="url(#neonRingGrad)"
            strokeWidth="2.8"
            strokeLinecap="round"
            strokeDasharray="140 24 35 24 20 20"
            transform="rotate(-24 80 72)"
          />
        </g>
      </g>

      {/* 3. SUBRAYADO MINIMALISTA EN LA BASE (FIJO) */}
      <g filter="url(#neonGlow)">
        <line
          x1="54"
          y1="138"
          x2="106"
          y2="138"
          stroke="url(#neonMoonGrad)"
          strokeWidth="3"
          strokeLinecap="round"
        />
      </g>
    </svg>
  );
}
