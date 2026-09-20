// Moon — el logo
// ============================================================
// 1. Media luna: fija en el centro, trazo neón morado/índigo de líneas finas, sin relleno sólido.
// 2. Aro orbital tipo Saturno:
//    - Proporción reducida y ajustada (no gigante).
//    - Perspectiva 3D inclinada cruzando la luna.
//    - El giro se realiza en el plano 3D (rotación horizontal sobre su propio eje orbital),
//      dando la sensación de partículas orbitando alrededor de la luna en 3D.
// 3. Subrayado: línea minimalista y limpia en la base.

import React from 'react';

export default function LogoMoon({ tamano = 100, titulo = 'Moon' }) {
  return (
    <div className="logo-moon-wrap" style={{ width: tamano, height: tamano }}>
      <svg
        className="logo-moon"
        width={tamano}
        height={tamano}
        viewBox="0 0 140 140"
        role="img"
        aria-label={titulo}
      >
        <defs>
          <linearGradient id="lunaGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#c084fc" />
            <stop offset="50%" stopColor="#818cf8" />
            <stop offset="100%" stopColor="#4f46e5" />
          </linearGradient>

          <linearGradient id="aroGrad" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#e879f9" />
            <stop offset="50%" stopColor="#818cf8" />
            <stop offset="100%" stopColor="#38bdf8" />
          </linearGradient>

          <filter id="brilloNeon" x="-20%" y="-20%" width="140%" height="140%">
            <feDropShadow dx="0" dy="0" stdDeviation="2" floodColor="#818cf8" floodOpacity="0.4" />
          </filter>
        </defs>

        {/* 1. MEDIA LUNA (FIJA) */}
        <g className="logo-moon__luna" filter="url(#brilloNeon)">
          <path
            d="M 76 26 C 54 26 42 42 42 62 C 42 82 56 98 80 98 C 60 92 52 78 52 62 C 52 46 60 32 76 26 Z"
            fill="none"
            stroke="url(#lunaGrad)"
            strokeWidth="3.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </g>

        {/* 2. SUBRAYADO (FIJO) */}
        <line
          x1="48"
          y1="120"
          x2="92"
          y2="120"
          stroke="url(#lunaGrad)"
          strokeWidth="2.8"
          strokeLinecap="round"
          filter="url(#brilloNeon)"
        />

        {/* 3. ARO ORBITAL 3D TIPO SATURNO (MÁS PEQUEÑO Y AJUSTADO A LA LUNA) */}
        {/* El contenedor orbital tiene perspectiva y rota sus segmentos orbitales continuamente */}
        <g className="logo-moon__saturno-escena">
          <ellipse
            className="logo-moon__saturno-aro"
            cx="70"
            cy="62"
            rx="46"
            ry="16"
            fill="none"
            stroke="url(#aroGrad)"
            strokeWidth="2.4"
            strokeLinecap="round"
            strokeDasharray="40 18 25 18 15 18"
            transform="rotate(-22 70 62)"
            filter="url(#brilloNeon)"
          />
        </g>
      </svg>
    </div>
  );
}
