// Moon — Ilustraciones de los estados vacíos
// ============================================================
// Un hueco vacío es el momento en que una app parece pequeña. Aquí cada hueco
// tiene su dibujo: trazo fino, geometría de órbitas (el lenguaje de la casa) y
// un solo toque de acento. Sustituyen al emoji suelto, que se ve barato.
//
// Todas heredan el color del texto con `currentColor` y se adaptan al tema.

import React from 'react';

function Lienzo({ children, titulo }) {
  return (
    <span className="ilustra" role="img" aria-label={titulo}>
      <svg viewBox="0 0 120 96" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
        {children}
      </svg>
    </span>
  );
}

/** Inicio sin publicaciones. */
export function IlustraInicio() {
  return (
    <Lienzo titulo="Todavía no hay publicaciones">
      <ellipse cx="60" cy="50" rx="44" ry="17" className="ilustra-orbita" />
      <circle cx="60" cy="50" r="15" className="ilustra-acento" />
      <path d="M52 45h16M52 51h11" strokeWidth="1.3" className="ilustra-suave" />
      <circle cx="104" cy="50" r="3.4" className="ilustra-acento" />
      <path d="M18 78h84" className="ilustra-suave" />
    </Lienzo>
  );
}

/** Sin conversaciones. */
export function IlustraMensajes() {
  return (
    <Lienzo titulo="Todavía no hay conversaciones">
      <rect x="20" y="26" width="52" height="34" rx="9" className="ilustra-acento" />
      <path d="M34 60v9l10-9" className="ilustra-acento" />
      <path d="M34 40h24M34 48h15" strokeWidth="1.3" className="ilustra-suave" />
      <rect x="62" y="42" width="38" height="26" rx="8" />
      <path d="M88 68v7l-8-7" />
    </Lienzo>
  );
}

/** Sin avisos. */
export function IlustraAvisos() {
  return (
    <Lienzo titulo="No hay avisos nuevos">
      <path d="M60 20a18 18 0 0 0-18 18v12l-6 9h48l-6-9V38a18 18 0 0 0-18-18Z" className="ilustra-acento" />
      <path d="M53 63a7 7 0 0 0 14 0" />
      <path d="M60 12v6" className="ilustra-suave" />
      <path d="M28 34 20 30M92 34l8-4" className="ilustra-suave" />
      <path d="M30 80h60" className="ilustra-suave" />
    </Lienzo>
  );
}

/** Sin fotos. */
export function IlustraFotos() {
  return (
    <Lienzo titulo="Todavía no hay fotos">
      <rect x="18" y="24" width="60" height="46" rx="8" className="ilustra-acento" />
      <path d="m24 62 15-15 12 12 10-9 13 12" className="ilustra-suave" />
      <circle cx="38" cy="38" r="4.5" className="ilustra-suave" />
      <rect x="52" y="40" width="48" height="34" rx="7" />
      <path d="M60 66l11-11 9 9" className="ilustra-suave" />
    </Lienzo>
  );
}

/** Sin gente (contactos, seguidores, solicitudes). */
export function IlustraGente() {
  return (
    <Lienzo titulo="Todavía no hay nadie aquí">
      <circle cx="46" cy="36" r="13" className="ilustra-acento" />
      <path d="M24 72a22 22 0 0 1 44 0" className="ilustra-acento" />
      <circle cx="82" cy="42" r="9" />
      <path d="M68 70a17 17 0 0 1 28 0" />
      <path d="M20 82h80" className="ilustra-suave" />
    </Lienzo>
  );
}

/** Sin resultados de búsqueda. */
export function IlustraBusqueda() {
  return (
    <Lienzo titulo="No se encontró nada">
      <circle cx="52" cy="44" r="20" className="ilustra-acento" />
      <path d="m67 59 16 16" strokeWidth="2.4" />
      <path d="M44 44h16M52 36v16" className="ilustra-suave" />
    </Lienzo>
  );
}

/** Sin grupos. */
export function IlustraGrupos() {
  return (
    <Lienzo titulo="Todavía no hay grupos">
      <circle cx="60" cy="30" r="10" className="ilustra-acento" />
      <circle cx="34" cy="46" r="8" />
      <circle cx="86" cy="46" r="8" />
      <path d="M42 70a18 18 0 0 1 36 0" className="ilustra-acento" />
      <path d="M20 68a14 14 0 0 1 24-6M100 68a14 14 0 0 0-24-6" className="ilustra-suave" />
    </Lienzo>
  );
}
